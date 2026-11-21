//! Functions a program calls by name, like `sol_log_`. Reference: Agave v3.0 `syscalls/src`.
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

/// Every supported syscall, keyed by the hash of its name. Keys are computed at compile time.
const SYSCALLS: [(u32, Syscall); 19] = [
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
    (hash_symbol_name(b"sol_sha256"), hash::sha256),
    (hash_symbol_name(b"sol_keccak256"), hash::keccak256),
    (hash_symbol_name(b"sol_create_program_address"), pda::create),
    (
        hash_symbol_name(b"sol_try_find_program_address"),
        pda::try_find,
    ),
    (hash_symbol_name(b"sol_get_clock_sysvar"), sysvar::clock),
    (hash_symbol_name(b"sol_get_rent_sysvar"), sysvar::rent),
    (hash_symbol_name(b"sol_set_return_data"), return_data::set),
    (hash_symbol_name(b"sol_get_return_data"), return_data::get),
];

/// The syscall a `call imm` key names, if camshaft has it.
pub fn lookup(key: u32) -> Option<Syscall> {
    SYSCALLS.iter().find(|(k, _)| *k == key).map(|&(_, f)| f)
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
        assert_eq!(SYSCALLS[2].0, 0x2075_59bd);
        let mut keys: Vec<_> = SYSCALLS.iter().map(|(k, _)| *k).collect();
        keys.sort();
        keys.dedup();
        assert_eq!(keys.len(), SYSCALLS.len());
    }
}
