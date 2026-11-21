//! `sol_set_return_data` and `sol_get_return_data`. Reference: Agave `syscalls/src/lib.rs`.

use super::{CPI_BYTES_PER_UNIT, SYSCALL_BASE_COST, check_outputs, read};
use crate::error::SyscallError;
use crate::interp::Vm;

const MAX_RETURN_DATA: u64 = 1024;

pub(super) fn set(vm: &mut Vm, [addr, len, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    vm.ctx
        .consume(len / CPI_BYTES_PER_UNIT + SYSCALL_BASE_COST)?;
    if len > MAX_RETURN_DATA {
        return Err(SyscallError::ReturnDataTooLarge(len, MAX_RETURN_DATA));
    }
    vm.ctx.return_data = read(&vm.memory, addr, len)?.to_vec();
    Ok(0)
}

/// Copies up to `len` bytes of return data to `addr`, and the program id that set it to
/// `program_id`. Returns the full length of the return data.
pub(super) fn get(vm: &mut Vm, [addr, len, program_id, ..]: [u64; 5]) -> Result<u64, SyscallError> {
    let Vm { ctx, memory, .. } = vm;
    ctx.consume(SYSCALL_BASE_COST)?;
    let total = ctx.return_data.len() as u64;
    let len = len.min(total);
    if len != 0 {
        ctx.consume((len + 32) / CPI_BYTES_PER_UNIT)?;
        check_outputs(memory, &[(addr, len), (program_id, 32)])?;
        memory
            .slice_mut(addr, len as usize)?
            .copy_from_slice(&ctx.return_data[..len as usize]);
        memory
            .slice_mut(program_id, 32)?
            .copy_from_slice(&ctx.program_id);
    }
    Ok(total)
}
