//! Loads an ELF file into an `Executable`. The `e_flags` header field picks the loader:
//! v0 (`v0.rs`) or v3 (`v3.rs`). Reference: `elf.rs` in anza-xyz/sbpf.

mod parse;
mod symbols;
mod v0;
mod v3;

pub use symbols::FunctionNames;

use crate::executable::Executable;
use parse::{ParseError, u32_at};
use thiserror::Error;

/// Names match the reference `ElfError`. Only the ones camshaft can produce are here.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum ElfError {
    #[error("failed to parse ELF file: {0}")]
    FailedToParse(String),
    #[error("entrypoint out of bounds")]
    EntrypointOutOfBounds,
    #[error("entrypoint is not on an instruction boundary")]
    InvalidEntrypoint,
    #[error("relative call out of bounds at instruction #{0}")]
    RelativeJumpOutOfBounds(usize),
    #[error("symbol hash collision {0:#x}")]
    SymbolHashCollision(u32),
    #[error("wrong ABI")]
    WrongAbi,
    #[error("wrong machine")]
    WrongMachine,
    #[error("wrong type, expected a shared object")]
    WrongType,
    #[error("expected exactly one .text section")]
    NotOneTextSection,
    #[error("writable section {0} is not supported")]
    WritableSectionNotSupported(String),
    #[error("relocation points at invalid address {0:#x}")]
    InvalidVirtualAddress(u64),
    #[error("unknown relocation type {0}")]
    UnknownRelocation(u32),
    #[error("unknown symbol with index {0}")]
    UnknownSymbol(usize),
    #[error("offset or value is out of bounds")]
    ValueOutOfBounds,
    #[error("Detected sbpf_version required by the executable which are not enabled")]
    UnsupportedSBPFVersion,
    #[error("invalid program header")]
    InvalidProgramHeader,
}

impl From<ParseError> for ElfError {
    fn from(e: ParseError) -> Self {
        match e {
            ParseError::InvalidProgramHeader => ElfError::InvalidProgramHeader,
            ParseError::OutOfBounds => ElfError::ValueOutOfBounds,
            e => ElfError::FailedToParse(e.to_string()),
        }
    }
}

impl Executable {
    /// Loads a v0 or v3 ELF. Does not run the verifier; call `verify` on the result.
    pub fn from_elf(bytes: &[u8]) -> Result<Self, ElfError> {
        let flags = bytes.get(48..52).ok_or(ElfError::ValueOutOfBounds)?;
        match u32_at(flags, 0) {
            0 => v0::load(bytes),
            3 => v3::load(bytes),
            // v1 and v2 never reached mainnet, and v4 is not out yet.
            _ => Err(ElfError::UnsupportedSBPFVersion),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_and_non_elf_files() {
        assert_eq!(
            Executable::from_elf(&[]).err(),
            Some(ElfError::ValueOutOfBounds)
        );
        let mut not_elf = vec![0; 64];
        assert_eq!(
            Executable::from_elf(&not_elf).err(),
            Some(ElfError::FailedToParse("invalid file header".into()))
        );
        not_elf[48] = 3;
        assert_eq!(
            Executable::from_elf(&not_elf).err(),
            Some(ElfError::FailedToParse("invalid file header".into()))
        );
        not_elf[48] = 2;
        assert_eq!(
            Executable::from_elf(&not_elf).err(),
            Some(ElfError::UnsupportedSBPFVersion)
        );
    }
}
