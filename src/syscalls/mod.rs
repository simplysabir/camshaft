//! Functions a program calls by name, like `sol_log_`. Reference: Agave 4.4.1 `syscalls/src`.
//!
//! A v0 program calls a syscall with `call imm`, where imm is the murmur3 hash of its name.
//! Arguments are in r1 to r5 and the result goes to r0. Each syscall charges compute units
//! before doing its work, as Agave does.

mod alloc;
mod hash;
mod log;
mod mem;
mod pda;
mod return_data;
mod sysvar;

pub use pda::create_program_address;

use crate::error::SyscallError;
use crate::executable::hash_symbol_name;
use crate::interp::Vm;
use crate::memory::Memory;

type Syscall = fn(&mut Vm, [u64; 5]) -> Result<u64, SyscallError>;

/// Agave's default costs, in compute units.
const SYSCALL_BASE_COST: u64 = 100;
const MEM_OP_BASE_COST: u64 = 10;
const CPI_BYTES_PER_UNIT: u64 = 250;

/// Every supported syscall, by name.
const SYSCALLS: [(&str, Syscall); 19] = [
    ("abort", log::abort),
    ("sol_panic_", log::panic),
    ("sol_log_", log::log),
    ("sol_log_64_", log::log_64),
    ("sol_log_pubkey", log::log_pubkey),
    ("sol_log_compute_units_", log::log_compute_units),
    ("sol_memcpy_", mem::memcpy),
    ("sol_memmove_", mem::memmove),
    ("sol_memset_", mem::memset),
    ("sol_memcmp_", mem::memcmp),
    ("sol_alloc_free_", alloc::alloc_free),
    ("sol_sha256", hash::sha256),
    ("sol_keccak256", hash::keccak256),
    ("sol_create_program_address", pda::create),
    ("sol_try_find_program_address", pda::try_find),
    ("sol_get_clock_sysvar", sysvar::clock),
    ("sol_get_rent_sysvar", sysvar::rent),
    ("sol_set_return_data", return_data::set),
    ("sol_get_return_data", return_data::get),
];

/// The key of each entry in `SYSCALLS`: the hash of its name, computed at compile time.
const KEYS: [u32; SYSCALLS.len()] = {
    let mut keys = [0; SYSCALLS.len()];
    let mut i = 0;
    while i < keys.len() {
        keys[i] = hash_symbol_name(SYSCALLS[i].0.as_bytes());
        i += 1;
    }
    keys
};

fn index(key: u32) -> Option<usize> {
    KEYS.iter().position(|&k| k == key)
}

/// The syscall a `call imm` key names, if camshaft has it.
pub fn lookup(key: u32) -> Option<Syscall> {
    index(key).map(|i| SYSCALLS[i].1)
}

/// The name of the syscall a key names, if camshaft has it.
pub fn name(key: u32) -> Option<&'static str> {
    index(key).map(|i| SYSCALLS[i].0)
}

/// `len` bytes at `addr` for reading, like Agave's `translate_slice::<u8>`.
/// A length of 0 is fine whatever the address.
fn read<'m>(memory: &'m Memory<'_>, addr: u64, len: u64) -> Result<&'m [u8], SyscallError> {
    if len == 0 {
        return Ok(&[]);
    }
    Ok(memory.slice(addr, checked_len(len)?)?)
}

/// `len` bytes at `addr` for writing. A length of 0 is fine whatever the address.
fn write<'m>(
    memory: &'m mut Memory<'_>,
    addr: u64,
    len: u64,
) -> Result<&'m mut [u8], SyscallError> {
    if len == 0 {
        return Ok(&mut []);
    }
    Ok(memory.slice_mut(addr, checked_len(len)?)?)
}

/// Several outputs written by one syscall: each must be writable, in order, and none may
/// share memory with another (Agave's `translate_mut!`).
fn check_outputs(memory: &mut Memory<'_>, outputs: &[(u64, u64)]) -> Result<(), SyscallError> {
    for &(addr, len) in outputs {
        write(memory, addr, len)?;
    }
    for (i, &(a, a_len)) in outputs.iter().enumerate() {
        for &(b, b_len) in &outputs[i + 1..] {
            if memory.overlaps((a, a_len as usize), (b, b_len as usize)) {
                return Err(SyscallError::CopyOverlapping);
            }
        }
    }
    Ok(())
}

/// An array of `(ptr, len)` pairs, the way programs pass a list of byte slices.
/// Each pair is 16 bytes and must be 8-byte aligned.
fn read_slices(
    memory: &Memory<'_>,
    addr: u64,
    count: u64,
) -> Result<Vec<(u64, u64)>, SyscallError> {
    let bytes = read(memory, addr, count.saturating_mul(16))?;
    if !bytes.is_empty() && !addr.is_multiple_of(8) {
        return Err(SyscallError::UnalignedPointer);
    }
    let u64_at = |b: &[u8]| u64::from_le_bytes(b.try_into().expect("8 bytes"));
    Ok(bytes
        .chunks(16)
        .map(|pair| (u64_at(&pair[..8]), u64_at(&pair[8..])))
        .collect())
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
        assert_eq!(name(0x2075_59bd), Some("sol_log_"));
        let mut keys = KEYS.to_vec();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), SYSCALLS.len());
    }
}
