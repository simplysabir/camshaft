//! Errors a program run can end with. Names match the reference `EbpfError`.

use crate::insn::DecodeError;
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
    #[error(transparent)]
    Decode(#[from] DecodeError),
}
