//! `sol_get_clock_sysvar` and `sol_get_rent_sysvar`. Reference: Agave `syscalls/src/sysvar.rs`.

use super::{SYSCALL_BASE_COST, write};
use crate::error::SyscallError;
use crate::interp::Vm;
use crate::memory::INPUT_START;

pub(super) fn clock(vm: &mut Vm, [addr, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    let bytes = vm.ctx.clock.to_bytes();
    copy_out(vm, addr, &bytes)
}

pub(super) fn rent(vm: &mut Vm, [addr, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    let bytes = vm.ctx.rent.to_bytes();
    copy_out(vm, addr, &bytes)
}

/// Costs 100 plus the struct size. The destination must be below the input region, writable,
/// then 8-byte aligned.
fn copy_out(vm: &mut Vm, addr: u64, bytes: &[u8]) -> Result<u64, SyscallError> {
    vm.ctx.consume(SYSCALL_BASE_COST + bytes.len() as u64)?;
    if addr >= INPUT_START {
        return Err(SyscallError::InvalidPointer);
    }
    let out = write(&mut vm.memory, addr, bytes.len() as u64)?;
    if !addr.is_multiple_of(8) {
        return Err(SyscallError::UnalignedPointer);
    }
    out.copy_from_slice(bytes);
    Ok(0)
}
