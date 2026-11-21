//! memcpy, memmove, memset, memcmp. Reference: Agave `syscalls/src/mem_ops.rs`.

use super::{CPI_BYTES_PER_UNIT, MEM_OP_BASE_COST, read, write};
use crate::error::SyscallError;
use crate::interp::Vm;

/// Every memory syscall costs 10 units, or 1 per 250 bytes if that is more.
fn charge(vm: &mut Vm, n: u64) -> Result<(), SyscallError> {
    vm.ctx.consume(MEM_OP_BASE_COST.max(n / CPI_BYTES_PER_UNIT))
}

pub(super) fn memcpy(vm: &mut Vm, [dst, src, n, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    charge(vm, n)?;
    if !non_overlapping(src, n, dst, n) {
        return Err(SyscallError::CopyOverlapping);
    }
    copy(vm, dst, src, n)
}

pub(super) fn memmove(vm: &mut Vm, [dst, src, n, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    charge(vm, n)?;
    copy(vm, dst, src, n)
}

pub(super) fn memset(vm: &mut Vm, [dst, c, n, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    charge(vm, n)?;
    write(&mut vm.memory, dst, n)?.fill(c as u8);
    Ok(0)
}

/// Writes `a - b` for the first differing bytes, or 0, as an `i32` at `result`.
pub(super) fn memcmp(vm: &mut Vm, [a, b, n, result, _]: [u64; 5]) -> Result<u64, SyscallError> {
    charge(vm, n)?;
    let (a, b) = (read(&vm.memory, a, n)?, read(&vm.memory, b, n)?);
    let diff = a
        .iter()
        .zip(b)
        .find(|(x, y)| x != y)
        .map_or(0, |(&x, &y)| i32::from(x) - i32::from(y));
    // The result is an i32, so its address must be 4-byte aligned. Bounds are checked first.
    let out = vm.memory.slice_mut(result, 4)?;
    if !result.is_multiple_of(4) {
        return Err(SyscallError::UnalignedPointer);
    }
    out.copy_from_slice(&diff.to_le_bytes());
    Ok(0)
}

/// Copies even if the ranges overlap. The destination is checked before the source.
fn copy(vm: &mut Vm, dst: u64, src: u64, n: u64) -> Result<u64, SyscallError> {
    write(&mut vm.memory, dst, n)?;
    let data = read(&vm.memory, src, n)?.to_vec();
    write(&mut vm.memory, dst, n)?.copy_from_slice(&data);
    Ok(0)
}

/// True if `[a, a + a_len)` and `[b, b + b_len)` don't overlap.
fn non_overlapping(a: u64, a_len: u64, b: u64, b_len: u64) -> bool {
    if a > b {
        a - b >= b_len
    } else {
        b - a >= a_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Agave's own `test_is_nonoverlapping` cases.
    #[test]
    fn overlap_rule() {
        for dst in 0..8 {
            assert!(non_overlapping(10, 3, dst, 3));
        }
        for dst in 8..13 {
            assert!(!non_overlapping(10, 3, dst, 3));
        }
        for dst in 13..20 {
            assert!(non_overlapping(10, 3, dst, 3));
        }
    }
}
