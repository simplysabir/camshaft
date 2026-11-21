//! A loaded program: its read-only memory, where the code is, and its functions.

use crate::memory::PROGRAM_START;
use crate::verifier::{VerifierError, verify};
use std::collections::BTreeMap;
use std::ops::Range;

pub struct Executable {
    /// The read-only region: code plus read-only data.
    pub ro: Vec<u8>,
    /// Virtual address of `ro[0]`.
    pub ro_addr: u64,
    /// Where the code is inside `ro`.
    pub text: Range<usize>,
    /// Slot index where execution starts.
    pub entry_pc: usize,
    /// `call imm` key to the pc it jumps to.
    pub functions: BTreeMap<u32, usize>,
}

impl Executable {
    /// Raw bytecode at the start of the program region. `functions` maps call keys to pcs.
    /// Starts at the function named `entrypoint` if there is one, else at pc 0.
    pub fn from_text(text: &[u8], mut functions: BTreeMap<u32, usize>) -> Self {
        let entry_pc = *functions.entry(entrypoint_key()).or_insert(0);
        Self {
            ro: text.to_vec(),
            ro_addr: PROGRAM_START,
            text: 0..text.len(),
            entry_pc,
            functions,
        }
    }

    pub fn text_bytes(&self) -> &[u8] {
        &self.ro[self.text.clone()]
    }

    /// Virtual address of the first instruction.
    pub fn text_addr(&self) -> u64 {
        self.ro_addr + self.text.start as u64
    }

    pub fn verify(&self) -> Result<(), VerifierError> {
        verify(self.text_bytes())
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
pub fn hash_symbol_name(name: &[u8]) -> u32 {
    let mix = |k: u32| {
        k.wrapping_mul(0xcc9e_2d51)
            .rotate_left(15)
            .wrapping_mul(0x1b87_3593)
    };
    let (blocks, tail) = name.as_chunks::<4>();
    let mut h = 0u32;
    for block in blocks {
        h ^= mix(u32::from_le_bytes(*block));
        h = h.rotate_left(13).wrapping_mul(5).wrapping_add(0xe654_6b64);
    }
    if !tail.is_empty() {
        let mut last = [0; 4];
        last[..tail.len()].copy_from_slice(tail);
        h ^= mix(u32::from_le_bytes(last));
    }
    h ^= name.len() as u32;
    h ^= h >> 16;
    h = h.wrapping_mul(0x85eb_ca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2_ae35);
    h ^ (h >> 16)
}
