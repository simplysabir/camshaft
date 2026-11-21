//! Loads a v3 ELF. SIMD-0189 fixes the layout, so this is a short list of checks with no
//! sections, symbols or relocations. Reference: `load_with_strict_parser` in anza-xyz/sbpf
//! `elf.rs`.

use super::ElfError;
use super::parse::{EHDR_SIZE, Header, PHDR_SIZE, PT_LOAD, ParseError, Phdr};
use crate::executable::Executable;
use crate::insn::INSN_SIZE;
use crate::memory::{PROGRAM_START, RODATA_START};
use crate::version::Version;
use std::collections::BTreeMap;

const PF_X: u32 = 1;
const PF_R: u32 = 4;
const EM_BPF: u16 = 247;

/// The file is: header, program headers, read-only data (optional), code.
pub(super) fn load(bytes: &[u8]) -> Result<Executable, ElfError> {
    let header = Header::read(bytes.get(..EHDR_SIZE).ok_or(ParseError::OutOfBounds)?);
    let headers_end = EHDR_SIZE + PHDR_SIZE * header.phnum as usize;
    let ident = header.ident;
    if ident[..4] != *b"\x7fELF"
        || ident[4] != 2 // 64-bit
        || ident[5] != 1 // little-endian
        || ident[6] != 1 // ELF version 1
        || ident[7..] != [0; 9] // no OS ABI, ABI version 0, zero padding
        || header.machine != EM_BPF
        || header.version != 1
        || header.phoff != EHDR_SIZE as u64
        || header.ehsize as usize != EHDR_SIZE
        || header.phentsize as usize != PHDR_SIZE
        || header.phnum == 0
        || headers_end > bytes.len()
    {
        return Err(ParseError::InvalidFileHeader.into());
    }

    let phdrs: Vec<Phdr> = bytes[EHDR_SIZE..headers_end]
        .chunks(PHDR_SIZE)
        .map(Phdr::read)
        .collect();
    // A first header that is not read-only means there is no read-only data.
    let has_rodata = phdrs[0].flags == PF_R;
    if has_rodata && phdrs.len() < 2 {
        return Err(ParseError::InvalidFileHeader.into());
    }
    let expected: &[(u32, u64)] = if has_rodata {
        &[(PF_R, RODATA_START), (PF_X, PROGRAM_START)]
    } else {
        &[(PF_X, PROGRAM_START)]
    };
    // Each segment starts where the one before it ends. Extra headers are ignored.
    let mut next_offset = headers_end as u64;
    for (p, &(flags, vaddr)) in phdrs.iter().zip(expected) {
        let len = bytes.len() as u64;
        if p.p_type != PT_LOAD
            || p.flags != flags
            || p.offset != next_offset
            || p.offset >= len
            || !p.offset.is_multiple_of(INSN_SIZE as u64)
            || p.vaddr != vaddr
            || p.paddr != vaddr
            || p.filesz != p.memsz
            || p.filesz > len - p.offset
            || !p.filesz.is_multiple_of(INSN_SIZE as u64)
            || p.memsz >= 1 << 32
        {
            return Err(ParseError::InvalidProgramHeader.into());
        }
        next_offset += p.filesz;
    }

    let file_bytes = |p: &Phdr| bytes[p.offset as usize..(p.offset + p.filesz) as usize].to_vec();
    let (ro, code) = if has_rodata {
        (file_bytes(&phdrs[0]), &phdrs[1])
    } else {
        (Vec::new(), &phdrs[0])
    };

    // The entry must be a whole instruction inside the code.
    let code_range = code.vaddr..code.vaddr.saturating_add(code.memsz);
    let entry = header.entry;
    if !code_range.contains(&entry.saturating_add(INSN_SIZE as u64 - 1))
        || !entry.is_multiple_of(INSN_SIZE as u64)
    {
        return Err(ParseError::InvalidFileHeader.into());
    }

    Ok(Executable {
        version: Version::V3,
        ro,
        ro_addr: RODATA_START,
        text: file_bytes(code),
        text_addr: code.vaddr,
        entry_pc: ((entry - code.vaddr) / INSN_SIZE as u64) as usize,
        functions: BTreeMap::new(),
        syscall_names: BTreeMap::new(),
    })
}
