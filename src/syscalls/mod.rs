//! Functions a program calls by name, like `sol_log_`. Reference: Agave v3.0.8 `syscalls/src`.
//!
//! A v0 program calls a syscall with `call imm`, where imm is the murmur3 hash of its name.
//! Arguments are in r1 to r5 and the result goes to r0.

mod alloc;
mod log;
mod mem;

pub use log::Logs;

use crate::error::SyscallError;
use crate::executable::hash_symbol_name;
use crate::interp::Vm;

type Syscall = fn(&mut Vm, [u64; 5]) -> Result<u64, SyscallError>;

/// Every supported syscall, keyed by the hash of its name. Keys are computed at compile time.
const SYSCALLS: [(u32, Syscall); 11] = [
    (hash_symbol_name(b"abort"), log::abort),
    (hash_symbol_name(b"sol_panic_"), log::panic),
    (hash_symbol_name(b"sol_log_"), log::log),
    (hash_symbol_name(b"sol_log_64_"), log::log_64),
    (hash_symbol_name(b"sol_log_pubkey"), log::log_pubkey),
    (
        hash_symbol_name(b"sol_log_compute_units_"),
        log::log_compute_units,
    ),
    (hash_symbol_name(b"sol_memcpy_"), mem::memcpy),
    (hash_symbol_name(b"sol_memmove_"), mem::memmove),
    (hash_symbol_name(b"sol_memset_"), mem::memset),
    (hash_symbol_name(b"sol_memcmp_"), mem::memcmp),
    (hash_symbol_name(b"sol_alloc_free_"), alloc::alloc_free),
];

/// The syscall a `call imm` key names, if camshaft has it.
pub fn lookup(key: u32) -> Option<Syscall> {
    SYSCALLS.iter().find(|(k, _)| *k == key).map(|&(_, f)| f)
}

/// `len` bytes at `addr` for reading, like Agave's `translate_slice::<u8>`.
/// A length of 0 is fine whatever the address.
fn read<'v>(vm: &'v Vm<'_>, addr: u64, len: u64) -> Result<&'v [u8], SyscallError> {
    if len == 0 {
        return Ok(&[]);
    }
    Ok(vm.memory.slice(addr, checked_len(len)?)?)
}

/// `len` bytes at `addr` for writing. A length of 0 is fine whatever the address.
fn write<'v>(vm: &'v mut Vm<'_>, addr: u64, len: u64) -> Result<&'v mut [u8], SyscallError> {
    if len == 0 {
        return Ok(&mut []);
    }
    Ok(vm.memory.slice_mut(addr, checked_len(len)?)?)
}

/// Lengths must fit in an `isize`.
fn checked_len(len: u64) -> Result<usize, SyscallError> {
    isize::try_from(len)
        .map(|len| len as usize)
        .map_err(|_| SyscallError::InvalidLength)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_match_the_known_hashes() {
        // sol_log_ is 0x207559bd in every Solana program.
        assert_eq!(SYSCALLS[2].0, 0x2075_59bd);
        let mut keys: Vec<_> = SYSCALLS.iter().map(|(k, _)| *k).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), SYSCALLS.len());
    }
}
