//! `sol_alloc_free_`: a bump allocator over the heap. Reference: Agave `SyscallAllocFree` and
//! `BpfAllocator` in `program-runtime/src/invoke_context.rs`.

use crate::error::SyscallError;
use crate::interp::Vm;
use crate::memory::HEAP_START;
use std::alloc::Layout;

/// Allocations are 8-byte aligned (`BPF_ALIGN_OF_U128`).
const ALIGN: usize = 8;

/// `free_addr == 0` allocates `size` bytes and returns their address. Anything else is a free,
/// which does nothing. Returns 0 when there is no room.
pub(super) fn alloc_free(
    vm: &mut Vm,
    [size, free_addr, ..]: [u64; 5],
) -> Result<u64, SyscallError> {
    // Agave builds a Layout and returns 0 for sizes it rejects. So do we.
    let Ok(layout) = Layout::from_size_align(size as usize, ALIGN) else {
        return Ok(0);
    };
    if free_addr != 0 {
        return Ok(0);
    }
    let start = vm.ctx.heap_used.next_multiple_of(ALIGN as u64);
    let end = start.saturating_add(layout.size() as u64);
    if end > vm.memory.heap.len() as u64 {
        return Ok(0);
    }
    vm.ctx.heap_used = end;
    Ok(HEAP_START + start)
}
