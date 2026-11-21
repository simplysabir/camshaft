//! Loads a v0 ELF: validate, relocate, then lay out the read-only sections.
//! Reference: the lenient loader in anza-xyz/sbpf `elf.rs`.

use super::ElfError;
use super::parse::{Elf, Shdr, u32_at};
use crate::executable::{Executable, entrypoint_key, function_key, hash_symbol_name};
use crate::insn::INSN_SIZE;
use crate::memory::{PROGRAM_START, STACK_START};
use crate::syscalls;
use crate::version::Version;
use std::collections::BTreeMap;
use std::collections::btree_map::Entry;

const SHF_WRITE: u64 = 1;
const SHF_ALLOC: u64 = 2;

const R_BPF_64_64: u32 = 1;
const R_BPF_64_RELATIVE: u32 = 8;
const R_BPF_64_32: u32 = 10;
/// Executables start at region 1, so a section at file address `a` lives at `REGION + a`.
const REGION: u64 = PROGRAM_START;

pub(super) fn load(bytes: &[u8]) -> Result<Executable, ElfError> {
    let elf = Elf::parse(bytes)?;
    validate(&elf)?;

    let text = elf
        .section_named(b".text")
        .expect("validate checked there is one");
    if text.addr.saturating_add(REGION) >= STACK_START {
        return Err(ElfError::ValueOutOfBounds);
    }

    let mut relocated = bytes.to_vec();
    let mut functions = BTreeMap::new();
    let mut syscall_names = BTreeMap::new();
    relocate(
        &elf,
        &text,
        &mut relocated,
        &mut functions,
        &mut syscall_names,
    )?;

    let entry_offset = elf.header.entry.saturating_sub(text.addr);
    if entry_offset % INSN_SIZE as u64 != 0 {
        return Err(ElfError::InvalidEntrypoint);
    }
    let entry_pc = (entry_offset / INSN_SIZE as u64) as usize;
    functions.remove(&entrypoint_key());
    register(&mut functions, entrypoint_key(), entry_pc)?;

    let (ro, lowest) = read_only_sections(&elf, &relocated)?;
    let ro_addr = if lowest >= REGION {
        lowest
    } else {
        lowest + REGION
    };
    if ro_addr >> 32 != 1 {
        return Err(ElfError::ValueOutOfBounds);
    }
    let text_start = (text.addr - lowest) as usize;
    let text = ro[text_start..text_start + text.file_range().len()].to_vec();
    Ok(Executable {
        version: Version::V0,
        text_addr: ro_addr + text_start as u64,
        ro,
        ro_addr,
        text,
        entry_pc,
        functions,
        syscall_names,
    })
}

fn validate(elf: &Elf) -> Result<(), ElfError> {
    let header = &elf.header;
    if header.ident[7] != 0 {
        return Err(ElfError::WrongAbi);
    }
    // EM_BPF or EM_SBPF
    if header.machine != 247 && header.machine != 263 {
        return Err(ElfError::WrongMachine);
    }
    // ET_DYN: a shared object
    if header.e_type != 3 {
        return Err(ElfError::WrongType);
    }
    if elf
        .sections
        .iter()
        .filter(|(_, name)| *name == b".text")
        .count()
        != 1
    {
        return Err(ElfError::NotOneTextSection);
    }
    for (s, name) in &elf.sections {
        let writable = s.flags & (SHF_ALLOC | SHF_WRITE) == SHF_ALLOC | SHF_WRITE;
        if name.starts_with(b".bss")
            || (writable && name.starts_with(b".data") && !name.starts_with(b".data.rel"))
        {
            return Err(ElfError::WritableSectionNotSupported(
                String::from_utf8_lossy(name).into(),
            ));
        }
    }
    for (s, _) in &elf.sections {
        let end = s
            .offset
            .checked_add(s.size)
            .ok_or(ElfError::ValueOutOfBounds)?;
        elf.bytes
            .get(s.offset as usize..end as usize)
            .ok_or(ElfError::ValueOutOfBounds)?;
    }
    let text = elf.section_named(b".text").expect("counted above");
    if !text.vm_range().contains(&header.entry) {
        return Err(ElfError::EntrypointOutOfBounds);
    }
    Ok(())
}

/// Adds `key -> pc`. A key that is already a syscall, or already a different pc, is an error.
fn register(functions: &mut BTreeMap<u32, usize>, key: u32, pc: usize) -> Result<u32, ElfError> {
    if syscalls::lookup(key).is_some() {
        return Err(ElfError::SymbolHashCollision(key));
    }
    match functions.entry(key) {
        Entry::Vacant(e) => {
            e.insert(pc);
        }
        Entry::Occupied(e) if *e.get() != pc => return Err(ElfError::SymbolHashCollision(key)),
        Entry::Occupied(_) => {}
    }
    Ok(key)
}

fn read_u32(bytes: &[u8], at: usize) -> Result<u32, ElfError> {
    let b = bytes
        .get(at..at.saturating_add(4))
        .ok_or(ElfError::ValueOutOfBounds)?;
    Ok(u32_at(b, 0))
}

fn write_bytes(bytes: &mut [u8], at: usize, value: &[u8]) -> Result<(), ElfError> {
    let dst = bytes
        .get_mut(at..at.saturating_add(value.len()))
        .ok_or(ElfError::ValueOutOfBounds)?;
    dst.copy_from_slice(value);
    Ok(())
}

/// Writes a 64-bit address into an lddw: low half in the first slot's imm, high in the second's.
fn write_lddw_imm(bytes: &mut [u8], imm_offset: usize, addr: u64) -> Result<(), ElfError> {
    write_bytes(bytes, imm_offset, &(addr as u32).to_le_bytes())?;
    write_bytes(
        bytes,
        imm_offset.saturating_add(INSN_SIZE),
        &((addr >> 32) as u32).to_le_bytes(),
    )
}

/// Patches call keys and addresses into `out`, a copy of the file.
fn relocate(
    elf: &Elf,
    text: &Shdr,
    out: &mut [u8],
    functions: &mut BTreeMap<u32, usize>,
    syscall_names: &mut BTreeMap<u32, String>,
) -> Result<(), ElfError> {
    let text_range = text.file_range();

    // Calls inside the program use a relative imm. Replace it with the target's key.
    let text_bytes = out
        .get_mut(text_range.clone())
        .ok_or(ElfError::ValueOutOfBounds)?;
    let count = text_bytes.len() / INSN_SIZE;
    for pc in 0..count {
        let slot = &mut text_bytes[pc * INSN_SIZE..(pc + 1) * INSN_SIZE];
        let imm = u32_at(slot, 4) as i32;
        if slot[0] != 0x85 || imm == -1 {
            continue;
        }
        let target = (pc as isize).saturating_add(1).saturating_add(imm as isize);
        if target < 0 || target >= count as isize {
            return Err(ElfError::RelativeJumpOutOfBounds(pc));
        }
        let key = register(functions, function_key(target as usize), target as usize)?;
        slot[4..].copy_from_slice(&key.to_le_bytes());
    }

    for rel in &elf.relocations {
        let r_offset = rel.offset as usize;
        let imm_offset = r_offset.saturating_add(4);
        let symbol_index = (rel.info >> 32) as usize;
        let symbol = || {
            elf.dynsym
                .get(symbol_index)
                .ok_or(ElfError::UnknownSymbol(symbol_index))
        };
        match rel.info as u32 {
            // lddw of a symbol's address plus the addend already in the imm.
            R_BPF_64_64 => {
                let addend = read_u32(out, imm_offset)? as u64;
                let mut addr = symbol()?.value.saturating_add(addend);
                if addr < REGION {
                    addr += REGION;
                }
                write_lddw_imm(out, imm_offset, addr)?;
            }
            // An address relative to the start of the file.
            R_BPF_64_RELATIVE if text_range.contains(&r_offset) => {
                let low = read_u32(out, imm_offset)? as u64;
                let high = read_u32(out, imm_offset.saturating_add(INSN_SIZE))? as u64;
                let mut addr = high << 32 | low;
                if addr == 0 {
                    return Err(ElfError::InvalidVirtualAddress(addr));
                }
                if addr < REGION {
                    addr += REGION;
                }
                write_lddw_imm(out, imm_offset, addr)?;
            }
            // Outside the code it is a plain 64-bit word. Old toolchains only wrote its low half.
            R_BPF_64_RELATIVE => {
                let addr = REGION + read_u32(out, imm_offset)? as u64;
                write_bytes(out, r_offset, &addr.to_le_bytes())?;
            }
            // A call by name: an internal function if defined here, otherwise a syscall.
            R_BPF_64_32 => {
                let symbol = symbol()?;
                let name = elf
                    .dynamic_symbol_name(symbol.name)
                    .map_err(|_| ElfError::UnknownSymbol(symbol.name as usize))?;
                let key = if symbol.is_function() && symbol.value != 0 {
                    if !text.vm_range().contains(&symbol.value) {
                        return Err(ElfError::ValueOutOfBounds);
                    }
                    let pc = ((symbol.value - text.addr) / INSN_SIZE as u64) as usize;
                    let key = if name == b"entrypoint" {
                        entrypoint_key()
                    } else {
                        function_key(pc)
                    };
                    register(functions, key, pc)?
                } else {
                    let key = hash_symbol_name(name);
                    syscall_names.insert(key, String::from_utf8_lossy(name).into());
                    key
                };
                write_bytes(out, imm_offset, &key.to_le_bytes())?;
            }
            other => return Err(ElfError::UnknownRelocation(other)),
        }
    }
    Ok(())
}

/// The read-only sections laid out by address, and the lowest address.
///
/// If they sit next to each other in the file at their own addresses, the file bytes between
/// them are copied as they are. Otherwise each section is copied into a zeroed buffer.
fn read_only_sections(elf: &Elf, relocated: &[u8]) -> Result<(Vec<u8>, u64), ElfError> {
    let mut lowest = usize::MAX;
    let mut highest = 0;
    let mut addr_matches_offset = true;
    let mut indices = Vec::new();
    let mut slices = Vec::new();
    for (i, (s, name)) in elf.sections.iter().enumerate() {
        if !matches!(
            *name,
            b".text" | b".rodata" | b".data.rel.ro" | b".eh_frame"
        ) {
            continue;
        }
        indices.push(i);
        addr_matches_offset &= s.addr == s.offset;
        if s.addr.saturating_add(REGION) > STACK_START {
            return Err(ElfError::ValueOutOfBounds);
        }
        let data = relocated
            .get(s.file_range())
            .ok_or(ElfError::ValueOutOfBounds)?;
        let addr = s.addr as usize;
        lowest = lowest.min(addr);
        highest = highest.max(addr.saturating_add(data.len()));
        if !data.is_empty() {
            slices.push((addr, data));
        }
    }

    let contiguous = indices.last().map(|last| last + 1 - indices[0]) == Some(indices.len());
    let ro = if addr_matches_offset && contiguous {
        relocated[lowest..highest].to_vec()
    } else {
        let len = highest - lowest;
        if len > relocated.len() {
            return Err(ElfError::ValueOutOfBounds);
        }
        let mut ro = vec![0; len];
        for (addr, data) in slices {
            let start = addr - lowest;
            let n = data.len().min(len - start);
            ro[start..start + n].copy_from_slice(&data[..n]);
        }
        ro
    };
    Ok((ro, lowest as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_keys_may_not_clash_with_syscalls() {
        let sol_log = hash_symbol_name(b"sol_log_");
        assert_eq!(
            register(&mut BTreeMap::new(), sol_log, 0),
            Err(ElfError::SymbolHashCollision(sol_log))
        );
    }
}
