//! Reads the structure of an ELF64 file: headers, sections, dynamic symbols, relocations.
//! Reference: `elf_parser/mod.rs` in anza-xyz/sbpf.

use std::ops::Range;
use thiserror::Error;

/// Errors from reading the file's structure. Messages match the reference `ElfParserError`,
/// because most of them end up as text inside `ElfError::FailedToParse`.
#[derive(Debug, Error)]
pub(super) enum ParseError {
    #[error("invalid file header")]
    InvalidFileHeader,
    #[error("invalid program header")]
    InvalidProgramHeader,
    #[error("invalid section header")]
    InvalidSectionHeader,
    #[error("Section or symbol name `{0}` is longer than `{1}` bytes")]
    StringTooLong(String, usize),
    #[error("value out of bounds")]
    OutOfBounds,
    #[error("invalid size")]
    InvalidSize,
    #[error("values overlap")]
    Overlap,
    #[error("sections not in ascending order")]
    SectionNotInOrder,
    #[error("no section name string table found")]
    NoSectionNameStringTable,
    #[error("invalid dynamic section table")]
    InvalidDynamicSectionTable,
    #[error("invalid alignment")]
    InvalidAlignment,
    #[error("no dynamic string table")]
    NoDynamicStringTable,
}

pub(super) const EHDR_SIZE: usize = 64;
pub(super) const PHDR_SIZE: usize = 56;
pub(super) const SHDR_SIZE: usize = 64;
pub(super) const SYM_SIZE: usize = 24;
const REL_SIZE: usize = 16;
const DYN_SIZE: usize = 16;

pub(super) const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const SHT_NULL: u32 = 0;
const SHT_SYMTAB: u32 = 2;
const SHT_STRTAB: u32 = 3;
const SHT_DYNAMIC: u32 = 6;
const SHT_NOBITS: u32 = 8;
const SHT_DYNSYM: u32 = 11;
const DT_SYMTAB: usize = 6;
const DT_REL: usize = 17;
const DT_RELSZ: usize = 18;
const DT_RELENT: usize = 19;
const DT_NUM: usize = 35;

const SECTION_NAME_MAX: usize = 16;
const SYMBOL_NAME_MAX: usize = 64;

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}
pub(super) fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(std::array::from_fn(|i| b[at + i]))
}
fn u64_at(b: &[u8], at: usize) -> u64 {
    u64::from_le_bytes(std::array::from_fn(|i| b[at + i]))
}

pub(super) struct Header {
    pub(super) ident: [u8; 16],
    pub(super) e_type: u16,
    pub(super) machine: u16,
    pub(super) version: u32,
    pub(super) entry: u64,
    pub(super) phoff: u64,
    pub(super) shoff: u64,
    pub(super) ehsize: u16,
    pub(super) phentsize: u16,
    pub(super) phnum: u16,
    shentsize: u16,
    pub(super) shnum: u16,
    pub(super) shstrndx: u16,
}

impl Header {
    pub(super) fn read(b: &[u8]) -> Self {
        Self {
            ident: std::array::from_fn(|i| b[i]),
            e_type: u16_at(b, 16),
            machine: u16_at(b, 18),
            version: u32_at(b, 20),
            entry: u64_at(b, 24),
            phoff: u64_at(b, 32),
            shoff: u64_at(b, 40),
            ehsize: u16_at(b, 52),
            phentsize: u16_at(b, 54),
            phnum: u16_at(b, 56),
            shentsize: u16_at(b, 58),
            shnum: u16_at(b, 60),
            shstrndx: u16_at(b, 62),
        }
    }
}

pub(super) struct Phdr {
    pub(super) p_type: u32,
    pub(super) flags: u32,
    pub(super) offset: u64,
    pub(super) vaddr: u64,
    pub(super) paddr: u64,
    pub(super) filesz: u64,
    pub(super) memsz: u64,
}

impl Phdr {
    pub(super) fn read(b: &[u8]) -> Self {
        Self {
            p_type: u32_at(b, 0),
            flags: u32_at(b, 4),
            offset: u64_at(b, 8),
            vaddr: u64_at(b, 16),
            paddr: u64_at(b, 24),
            filesz: u64_at(b, 32),
            memsz: u64_at(b, 40),
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Shdr {
    pub(super) name: u32,
    pub(super) sh_type: u32,
    pub(super) flags: u64,
    pub(super) addr: u64,
    pub(super) offset: u64,
    pub(super) size: u64,
}

impl Shdr {
    pub(super) fn read(b: &[u8]) -> Self {
        Self {
            name: u32_at(b, 0),
            sh_type: u32_at(b, 4),
            flags: u64_at(b, 8),
            addr: u64_at(b, 16),
            offset: u64_at(b, 24),
            size: u64_at(b, 32),
        }
    }

    /// Bytes the section occupies in the file. Empty for `.bss`-like sections.
    pub(super) fn file_range(&self) -> Range<usize> {
        if self.sh_type == SHT_NOBITS {
            return 0..0;
        }
        let start = self.offset as usize;
        start..start.saturating_add(self.size as usize)
    }

    pub(super) fn vm_range(&self) -> Range<u64> {
        self.addr..self.addr.saturating_add(self.size)
    }
}

pub(super) struct Sym {
    pub(super) name: u32,
    pub(super) info: u8,
    pub(super) value: u64,
}

impl Sym {
    pub(super) fn read(b: &[u8]) -> Self {
        Self {
            name: u32_at(b, 0),
            info: b[4],
            value: u64_at(b, 8),
        }
    }

    pub(super) fn is_function(&self) -> bool {
        self.info & 0xf == 2
    }
}

pub(super) struct Rel {
    pub(super) offset: u64,
    pub(super) info: u64,
}

/// The parts of the file the loader needs.
pub(super) struct Elf<'a> {
    pub(super) bytes: &'a [u8],
    pub(super) header: Header,
    phdrs: Vec<Phdr>,
    /// Section headers with their names.
    pub(super) sections: Vec<(Shdr, &'a [u8])>,
    pub(super) dynsym: Vec<Sym>,
    dynstr: Option<Shdr>,
    pub(super) relocations: Vec<Rel>,
}

/// `range.start..range.start + count * size`, failing on overflow.
fn table_range(offset: u64, size: usize, count: u16) -> Result<Range<usize>, ParseError> {
    let start = offset as usize;
    let len = size
        .checked_mul(count.into())
        .ok_or(ParseError::OutOfBounds)?;
    Ok(start..len.checked_add(start).ok_or(ParseError::OutOfBounds)?)
}

fn sized_range(offset: u64, size: u64) -> Result<Range<usize>, ParseError> {
    let start = offset as usize;
    Ok(start
        ..start
            .checked_add(size as usize)
            .ok_or(ParseError::OutOfBounds)?)
}

/// A table of `entry_size` records. Every ELF record type is 8-byte aligned.
fn table(bytes: &[u8], range: Range<usize>, entry_size: usize) -> Result<&[u8], ParseError> {
    if !range.len().is_multiple_of(entry_size) {
        return Err(ParseError::InvalidSize);
    }
    let start = range.start;
    let table = bytes.get(range).ok_or(ParseError::OutOfBounds)?;
    if !start.is_multiple_of(8) {
        return Err(ParseError::InvalidAlignment);
    }
    Ok(table)
}

fn no_overlap(a: &Range<usize>, b: &Range<usize>) -> Result<(), ParseError> {
    if a.end <= b.start || b.end <= a.start {
        Ok(())
    } else {
        Err(ParseError::Overlap)
    }
}

/// A NUL-terminated string at `offset` inside a string table section.
pub(super) fn string_in<'a>(
    bytes: &'a [u8],
    table: &Shdr,
    offset: u32,
    max: usize,
) -> Result<&'a [u8], ParseError> {
    if table.sh_type != SHT_STRTAB {
        return Err(ParseError::InvalidSectionHeader);
    }
    let start = (table.offset as usize)
        .checked_add(offset as usize)
        .ok_or(ParseError::OutOfBounds)?;
    let table_end = (table.offset as usize)
        .checked_add(table.size as usize)
        .ok_or(ParseError::OutOfBounds)?;
    let end = table_end.min(start.checked_add(max).ok_or(ParseError::OutOfBounds)?);
    let unterminated = bytes.get(start..end).ok_or(ParseError::OutOfBounds)?;
    match unterminated.iter().position(|&b| b == 0) {
        Some(len) => Ok(&unterminated[..len]),
        None => Err(ParseError::StringTooLong(
            String::from_utf8_lossy(unterminated).into(),
            max,
        )),
    }
}

impl<'a> Elf<'a> {
    pub(super) fn parse(bytes: &'a [u8]) -> Result<Self, ParseError> {
        let header_range = 0..EHDR_SIZE;
        let header = Header::read(
            bytes
                .get(header_range.clone())
                .ok_or(ParseError::OutOfBounds)?,
        );
        if header.ident[..4] != *b"\x7fELF"
            || header.ident[4] != 2 // 64-bit
            || header.ident[5] != 1 // little-endian
            || header.ident[6] != 1
            || header.version != 1
            || header.ehsize as usize != EHDR_SIZE
            || header.phentsize as usize != PHDR_SIZE
            || header.shentsize as usize != SHDR_SIZE
            || header.shstrndx >= header.shnum
        {
            return Err(ParseError::InvalidFileHeader);
        }

        let ph_range = table_range(header.phoff, PHDR_SIZE, header.phnum)?;
        no_overlap(&header_range, &ph_range)?;
        let ph_table = table(bytes, ph_range.clone(), PHDR_SIZE)?;
        let sh_range = table_range(header.shoff, SHDR_SIZE, header.shnum)?;
        no_overlap(&header_range, &sh_range)?;
        no_overlap(&ph_range, &sh_range)?;
        let sh_table = table(bytes, sh_range.clone(), SHDR_SIZE)?;
        let phdrs: Vec<Phdr> = ph_table.chunks(PHDR_SIZE).map(Phdr::read).collect();
        let shdrs: Vec<Shdr> = sh_table.chunks(SHDR_SIZE).map(Shdr::read).collect();

        if shdrs.first().map(|s| s.sh_type) != Some(SHT_NULL) {
            return Err(ParseError::InvalidSectionHeader);
        }

        // Loadable segments must be sorted by address and fit in the file.
        let mut vaddr = 0;
        for p in phdrs.iter().filter(|p| p.p_type == PT_LOAD) {
            if p.vaddr < vaddr {
                return Err(ParseError::InvalidProgramHeader);
            }
            let end = p
                .offset
                .checked_add(p.filesz)
                .ok_or(ParseError::OutOfBounds)?;
            if end as usize > bytes.len() {
                return Err(ParseError::OutOfBounds);
            }
            vaddr = p.vaddr;
        }

        // Sections with file data must be in order, in the file, and clear of the headers.
        let mut offset = 0;
        for s in shdrs.iter().filter(|s| s.sh_type != SHT_NOBITS) {
            let range = sized_range(s.offset, s.size)?;
            no_overlap(&range, &header_range)?;
            no_overlap(&range, &ph_range)?;
            no_overlap(&range, &sh_range)?;
            if range.start < offset {
                return Err(ParseError::SectionNotInOrder);
            }
            offset = range.end;
            if offset > bytes.len() {
                return Err(ParseError::OutOfBounds);
            }
        }

        let names = (header.shstrndx != 0)
            .then(|| shdrs[header.shstrndx as usize])
            .ok_or(ParseError::NoSectionNameStringTable)?;
        let mut sections: Vec<(Shdr, &[u8])> = Vec::with_capacity(shdrs.len());
        for s in shdrs {
            let name = string_in(bytes, &names, s.name, SECTION_NAME_MAX)?;
            // These three may appear at most once.
            let unique = matches!(name, b".symtab" | b".strtab" | b".dynstr");
            if unique && sections.iter().any(|(_, seen)| *seen == name) {
                return Err(ParseError::InvalidSectionHeader);
            }
            sections.push((s, name));
        }
        let dynstr = sections
            .iter()
            .find(|(_, name)| *name == b".dynstr")
            .map(|(s, _)| *s);

        let mut elf = Self {
            bytes,
            header,
            phdrs,
            sections,
            dynsym: Vec::new(),
            dynstr,
            relocations: Vec::new(),
        };
        elf.parse_dynamic()?;
        Ok(elf)
    }

    fn parse_dynamic(&mut self) -> Result<(), ParseError> {
        // Prefer the PT_DYNAMIC segment; fall back to the SHT_DYNAMIC section.
        let mut table_bytes = self
            .phdrs
            .iter()
            .find(|p| p.p_type == PT_DYNAMIC)
            .and_then(|p| table(self.bytes, sized_range(p.offset, p.filesz).ok()?, DYN_SIZE).ok());
        if table_bytes.is_none()
            && let Some((s, _)) = self.sections.iter().find(|(s, _)| s.sh_type == SHT_DYNAMIC)
        {
            let range = sized_range(s.offset, s.size)
                .map_err(|_| ParseError::InvalidDynamicSectionTable)?;
            table_bytes = Some(
                table(self.bytes, range, DYN_SIZE)
                    .map_err(|_| ParseError::InvalidDynamicSectionTable)?,
            );
        }
        let Some(table_bytes) = table_bytes else {
            return Ok(());
        };

        let mut dynamic = [0u64; DT_NUM];
        for entry in table_bytes.chunks(DYN_SIZE) {
            let tag = u64_at(entry, 0);
            if tag == 0 {
                break;
            }
            if let Some(slot) = dynamic.get_mut(tag as usize) {
                *slot = u64_at(entry, 8);
            }
        }

        if dynamic[DT_REL] != 0 {
            let vaddr = dynamic[DT_REL];
            if dynamic[DT_RELENT] as usize != REL_SIZE || dynamic[DT_RELSZ] == 0 {
                return Err(ParseError::InvalidDynamicSectionTable);
            }
            let offset = match self.segment_containing(vaddr)? {
                Some(p) => {
                    let into = vaddr.checked_sub(p.vaddr).ok_or(ParseError::OutOfBounds)?;
                    into.checked_add(p.offset).ok_or(ParseError::OutOfBounds)?
                }
                // Older toolchains put DT_REL outside every segment; use the section instead.
                None => {
                    self.section_at(vaddr)
                        .ok_or(ParseError::InvalidDynamicSectionTable)?
                        .offset
                }
            };
            let range = sized_range(offset, dynamic[DT_RELSZ])?;
            let rels = table(self.bytes, range, REL_SIZE)
                .map_err(|_| ParseError::InvalidDynamicSectionTable)?;
            self.relocations = rels
                .chunks(REL_SIZE)
                .map(|r| Rel {
                    offset: u64_at(r, 0),
                    info: u64_at(r, 8),
                })
                .collect();
        }

        if dynamic[DT_SYMTAB] != 0 {
            let s = self
                .section_at(dynamic[DT_SYMTAB])
                .ok_or(ParseError::InvalidDynamicSectionTable)?;
            if s.sh_type != SHT_SYMTAB && s.sh_type != SHT_DYNSYM {
                return Err(ParseError::InvalidSectionHeader);
            }
            let syms = table(self.bytes, sized_range(s.offset, s.size)?, SYM_SIZE)?;
            self.dynsym = syms.chunks(SYM_SIZE).map(Sym::read).collect();
        }
        Ok(())
    }

    /// Any program header whose memory range holds `vaddr`.
    fn segment_containing(&self, vaddr: u64) -> Result<Option<&Phdr>, ParseError> {
        for p in &self.phdrs {
            let end = p
                .vaddr
                .checked_add(p.memsz)
                .ok_or(ParseError::OutOfBounds)?;
            if (p.vaddr..end).contains(&vaddr) {
                return Ok(Some(p));
            }
        }
        Ok(None)
    }

    fn section_at(&self, addr: u64) -> Option<Shdr> {
        self.sections
            .iter()
            .find(|(s, _)| s.addr == addr)
            .map(|(s, _)| *s)
    }

    pub(super) fn section_named(&self, name: &[u8]) -> Option<Shdr> {
        self.sections
            .iter()
            .find(|(_, n)| *n == name)
            .map(|(s, _)| *s)
    }

    pub(super) fn dynamic_symbol_name(&self, offset: u32) -> Result<&'a [u8], ParseError> {
        let dynstr = self
            .dynstr
            .as_ref()
            .ok_or(ParseError::NoDynamicStringTable)?;
        string_in(self.bytes, dynstr, offset, SYMBOL_NAME_MAX)
    }
}
