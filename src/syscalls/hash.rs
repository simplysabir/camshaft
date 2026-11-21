//! `sol_sha256` and `sol_keccak256`. Reference: Agave `SyscallHash` in `syscalls/src/lib.rs`.

use super::{MEM_OP_BASE_COST, read, read_slices, write};
use crate::error::SyscallError;
use crate::interp::Vm;
use sha2::Digest;

/// Agave's costs and limit. Keccak uses the same numbers as sha256.
const BASE_COST: u64 = 85;
const BYTE_COST: u64 = 1;
const MAX_SLICES: u64 = 20_000;

pub(super) fn sha256(
    vm: &mut Vm,
    [vals, count, result, ..]: [u64; 5],
) -> Result<u64, SyscallError> {
    hash::<sha2::Sha256>(vm, "Sha256", vals, count, result)
}

pub(super) fn keccak256(
    vm: &mut Vm,
    [vals, count, result, ..]: [u64; 5],
) -> Result<u64, SyscallError> {
    hash::<sha3::Keccak256>(vm, "Keccak256", vals, count, result)
}

/// Hashes `count` byte slices, listed at `vals`, into the 32 bytes at `result`.
fn hash<D: Digest>(
    vm: &mut Vm,
    name: &str,
    vals: u64,
    count: u64,
    result: u64,
) -> Result<u64, SyscallError> {
    if count > MAX_SLICES {
        let line = format!(
            "{name} Hashing {count} sequences in one syscall is over the limit {MAX_SLICES}"
        );
        vm.ctx.logs.push(line);
        return Err(SyscallError::TooManySlices);
    }
    vm.ctx.consume(BASE_COST)?;
    write(&mut vm.memory, result, 32)?;
    let mut hasher = D::new();
    for (ptr, len) in read_slices(&vm.memory, vals, count)? {
        // Agave checks the slice before charging for it.
        let bytes = read(&vm.memory, ptr, len)?;
        vm.ctx
            .consume(MEM_OP_BASE_COST.max(BYTE_COST * (len / 2)))?;
        hasher.update(bytes);
    }
    write(&mut vm.memory, result, 32)?.copy_from_slice(&hasher.finalize());
    Ok(0)
}
