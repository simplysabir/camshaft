//! Errors a program run can end with. Names match the reference `EbpfError`.

use crate::insn::DecodeError;
use std::str::Utf8Error;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Load,
    Store,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum VmError {
    #[error("division by zero")]
    DivideByZero,
    #[error("pc ran past the end of the program")]
    ExecutionOverrun,
    #[error("instruction limit reached")]
    ExceededMaxInstructions,
    #[error("invalid instruction")]
    InvalidInstruction,
    #[error("unsupported instruction")]
    UnsupportedInstruction,
    #[error("call depth limit reached")]
    CallDepthExceeded,
    #[error("call target is outside the code")]
    CallOutsideTextSegment,
    #[error("access violation: {0:?} of {2} bytes at {1:#x} in {3}")]
    AccessViolation(AccessType, u64, u64, &'static str),
    #[error("stack access violation: {0:?} of {2} bytes at {1:#x} in frame {3}")]
    StackAccessViolation(AccessType, u64, u64, i64),
    #[error("syscall error: {0}")]
    SyscallError(Box<SyscallError>),
    #[error(transparent)]
    Decode(#[from] DecodeError),
}

/// Why a syscall failed. Names and messages match Agave's `SyscallError`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum SyscallError {
    #[error("{0}: {1:?}")]
    InvalidString(Utf8Error, Vec<u8>),
    #[error("SBF program panicked")]
    Abort,
    #[error("SBF program Panicked in {0} at {1}:{2}")]
    Panic(String, u64, u64),
    #[error("Unaligned pointer")]
    UnalignedPointer,
    #[error("Overlapping copy")]
    CopyOverlapping,
    #[error("InvalidLength")]
    InvalidLength,
    /// A pointer argument hit unmapped or read-only memory.
    #[error(transparent)]
    Memory(VmError),
}

impl From<VmError> for SyscallError {
    fn from(e: VmError) -> Self {
        SyscallError::Memory(e)
    }
}

impl From<SyscallError> for VmError {
    fn from(e: SyscallError) -> Self {
        VmError::SyscallError(Box::new(e))
    }
}
