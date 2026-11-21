//! A loaded program: its version, code, read-only memory and functions.

use crate::memory::{PROGRAM_START, RODATA_START};
use crate::verifier::{VerifierError, verify};
use crate::version::Version;
use std::collections::BTreeMap;

pub struct Executable {
    pub version: Version,
    /// The read-only region. In v0 it holds the code too; in v3 only data.
    pub ro: Vec<u8>,
    /// Virtual address of `ro[0]`.
    pub ro_addr: u64,
    /// The bytecode.
    pub text: Vec<u8>,
    /// Virtual address of `text[0]`.
    pub text_addr: u64,
    /// Slot index where execution starts.
    pub entry_pc: usize,
    /// v0 only: `call imm` key to the pc it jumps to. v3 calls are relative.
    pub functions: BTreeMap<u32, usize>,
    /// Names of the syscalls the program calls, by key. Used to name a missing syscall.
    pub syscall_names: BTreeMap<u32, String>,
}

impl Executable {
    /// Raw bytecode at the start of the program region. `functions` maps call keys to pcs.
    /// Starts at the function named `entrypoint` if there is one, else at pc 0.
    /// Like the reference, the code is also readable as data: at `0x1_0000_0000` in v0 and
    /// at 0 in v3.
    pub fn from_text(text: &[u8], mut functions: BTreeMap<u32, usize>, version: Version) -> Self {
        let entry_pc = *functions.entry(entrypoint_key()).or_insert(0);
        Self {
            version,
            ro: text.to_vec(),
            ro_addr: match version {
                Version::V0 => PROGRAM_START,
                Version::V3 => RODATA_START,
            },
            text: text.to_vec(),
            text_addr: PROGRAM_START,
            entry_pc,
            functions,
            syscall_names: BTreeMap::new(),
        }
    }

    pub fn verify(&self) -> Result<(), VerifierError> {
        verify(&self.text, self.version)
    }
}

pub fn entrypoint_key() -> u32 {
    hash_symbol_name(b"entrypoint")
}

/// The key a v0 loader gives a function found by a relative call: the hash of its pc.
pub fn function_key(pc: usize) -> u32 {
    hash_symbol_name(&(pc as u64).to_le_bytes())
}

/// 32-bit murmur3 with seed 0. v0 names syscalls and functions by this hash.
/// `const` so syscall keys are computed at compile time.
pub const fn hash_symbol_name(name: &[u8]) -> u32 {
    const fn mix(k: u32) -> u32 {
        k.wrapping_mul(0xcc9e_2d51)
            .rotate_left(15)
            .wrapping_mul(0x1b87_3593)
    }
    let (blocks, tail) = name.as_chunks::<4>();
    let mut h = 0u32;
    let mut i = 0;
    while i < blocks.len() {
        h ^= mix(u32::from_le_bytes(blocks[i]));
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
        i += 1;
    }
    if !tail.is_empty() {
        let mut last = 0u32;
        let mut j = 0;
        while j < tail.len() {
            last |= (tail[j] as u32) << (8 * j);
            j += 1;
        }
        h ^= mix(last);
    }
    h ^= name.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}
