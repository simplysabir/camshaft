//! Logging, panic and abort. Reference: Agave `syscalls/src/logging.rs` and `SyscallAbort`,
//! `SyscallPanic` in `lib.rs`.

use super::{SYSCALL_BASE_COST, read};
use crate::error::SyscallError;
use crate::interp::Vm;

pub(super) fn abort(_: &mut Vm, _: [u64; 5]) -> Result<u64, SyscallError> {
    Err(SyscallError::Abort)
}

pub(super) fn panic(
    vm: &mut Vm,
    [file, len, line, column, _]: [u64; 5],
) -> Result<u64, SyscallError> {
    vm.ctx.consume(len)?;
    let file = utf8(read(&vm.memory, file, len)?)?;
    Err(SyscallError::Panic(file.into(), line, column))
}

pub(super) fn log(vm: &mut Vm, [addr, len, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    vm.ctx.consume(SYSCALL_BASE_COST.max(len))?;
    let line = format!("Program log: {}", utf8(read(&vm.memory, addr, len)?)?);
    vm.ctx.logs.push(line);
    Ok(0)
}

pub(super) fn log_64(vm: &mut Vm, [a, b, c, d, e]: [u64; 5]) -> Result<u64, SyscallError> {
    vm.ctx.consume(SYSCALL_BASE_COST)?;
    vm.ctx.logs.push(format!(
        "Program log: {a:#x}, {b:#x}, {c:#x}, {d:#x}, {e:#x}"
    ));
    Ok(0)
}

pub(super) fn log_pubkey(vm: &mut Vm, [addr, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    vm.ctx.consume(SYSCALL_BASE_COST)?;
    let key = bs58::encode(vm.memory.slice(addr, 32)?).into_string();
    vm.ctx.logs.push(format!("Program log: {key}"));
    Ok(0)
}

/// Logs the units left after paying for this call.
pub(super) fn log_compute_units(vm: &mut Vm, _: [u64; 5]) -> Result<u64, SyscallError> {
    vm.ctx.consume(SYSCALL_BASE_COST)?;
    let line = format!("Program consumption: {} units remaining", vm.ctx.remaining);
    vm.ctx.logs.push(line);
    Ok(0)
}

fn utf8(bytes: &[u8]) -> Result<&str, SyscallError> {
    std::str::from_utf8(bytes).map_err(|e| SyscallError::InvalidString(e, bytes.to_vec()))
}
