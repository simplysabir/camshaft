//! Errors a program run can end with. Names and messages match the reference, because
//! Solana prints them in the program's log.

use crate::insn::DecodeError;
use std::fmt;
use std::str::Utf8Error;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccessType {
    Load,
    Store,
}

impl fmt::Display for AccessType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            AccessType::Load => "reading",
            AccessType::Store => "writing",
        })
    }
}

/// Why the VM stopped. Messages match the reference `EbpfError`.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VmError {
    #[error("divide by zero at BPF instruction")]
    DivideByZero,
    #[error("attempted to execute past the end of the text segment at BPF instruction")]
    ExecutionOverrun,
    #[error("exceeded CUs meter at BPF instruction")]
    ExceededMaxInstructions,
    #[error("invalid BPF instruction")]
    InvalidInstruction,
    #[error("unsupported BPF instruction")]
    UnsupportedInstruction,
    #[error("exceeded max BPF to BPF call depth")]
    CallDepthExceeded,
    #[error("callx attempted to call outside of the text segment")]
    CallOutsideTextSegment,
    #[error("Access violation {0} {2} bytes at address {1:#x} (in {3} region)")]
    AccessViolation(AccessType, u64, u64, &'static str),
    #[error("Access violation in stack frame {3} at address {1:#x} of size {2:?}")]
    StackAccessViolation(AccessType, u64, u64, i64),
    #[error("Syscall error: {0}")]
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
    #[error(
        "Could not create program address with signer seeds: Length of the seed is too long for address generation"
    )]
    BadSeeds,
    #[error("Unaligned pointer")]
    UnalignedPointer,
    #[error("Invalid pointer")]
    InvalidPointer,
    #[error("Overlapping copy")]
    CopyOverlapping,
    #[error("Return data too large ({0} > {1})")]
    ReturnDataTooLarge(u64, u64),
    #[error("Hashing too many sequences")]
    TooManySlices,
    #[error("InvalidLength")]
    InvalidLength,
    /// Agave reports this as an `InstructionError`, not a syscall error.
    #[error("Computational budget exceeded")]
    ComputationalBudgetExceeded,
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

/// Why an instruction failed. Names and messages match Solana's `InstructionError`.
/// Only the variants a single top-level instruction can produce are here.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum InstructionError {
    #[error("invalid program argument")]
    InvalidArgument,
    #[error("invalid instruction data")]
    InvalidInstructionData,
    #[error("invalid account data for instruction")]
    InvalidAccountData,
    #[error("account data too small for instruction")]
    AccountDataTooSmall,
    #[error("insufficient funds for instruction")]
    InsufficientFunds,
    #[error("incorrect program id for instruction")]
    IncorrectProgramId,
    #[error("missing required signature for instruction")]
    MissingRequiredSignature,
    #[error("instruction requires an uninitialized account")]
    AccountAlreadyInitialized,
    #[error("instruction requires an initialized account")]
    UninitializedAccount,
    #[error("sum of account balances before and after instruction do not match")]
    UnbalancedInstruction,
    #[error("instruction illegally modified the program id of an account")]
    ModifiedProgramId,
    #[error("instruction spent from the balance of an account it does not own")]
    ExternalAccountLamportSpend,
    #[error("instruction modified data of an account it does not own")]
    ExternalAccountDataModified,
    #[error("instruction changed the balance of a read-only account")]
    ReadonlyLamportChange,
    #[error("instruction modified data of a read-only account")]
    ReadonlyDataModified,
    #[error("insufficient account keys for instruction")]
    NotEnoughAccountKeys,
    #[error("program other than the account's owner changed the size of the account data")]
    AccountDataSizeChanged,
    #[error("instruction tries to borrow reference for an account which is already borrowed")]
    AccountBorrowFailed,
    #[error("custom program error: {0:#x}")]
    Custom(u32),
    #[error("program returned invalid error code")]
    InvalidError,
    #[error("Length of the seed is too long for address generation")]
    MaxSeedLengthExceeded,
    #[error("Provided seeds do not result in a valid address")]
    InvalidSeeds,
    #[error("Failed to reallocate account data")]
    InvalidRealloc,
    #[error("Computational budget exceeded")]
    ComputationalBudgetExceeded,
    #[error("Program failed to complete")]
    ProgramFailedToComplete,
    #[error("Account is immutable")]
    Immutable,
    #[error("Incorrect authority provided")]
    IncorrectAuthority,
    #[error("Failed to serialize or deserialize account data")]
    BorshIoError,
    #[error("An account does not have enough lamports to be rent-exempt")]
    AccountNotRentExempt,
    #[error("Invalid account owner")]
    InvalidAccountOwner,
    #[error("Program arithmetic overflowed")]
    ArithmeticOverflow,
    #[error("Unsupported sysvar")]
    UnsupportedSysvar,
    #[error("Provided owner is not allowed")]
    IllegalOwner,
    #[error("Accounts data allocations exceeded the maximum allowed per transaction")]
    MaxAccountsDataAllocationsExceeded,
    #[error("Max instruction trace length exceeded")]
    MaxInstructionTraceLengthExceeded,
    #[error("Builtin programs must consume compute units")]
    BuiltinProgramsMustConsumeComputeUnits,
}

impl InstructionError {
    /// What a non-zero value in r0 at `exit` means. Built-in errors live in the upper 32 bits;
    /// anything in the lower 32 bits alone is a custom error code.
    pub fn from_return_code(code: u64) -> Self {
        use InstructionError::*;
        match code >> 32 {
            0 => Custom(code as u32),
            _ if code & 0xffff_ffff != 0 => InvalidError,
            1 => Custom(0),
            2 => InvalidArgument,
            3 => InvalidInstructionData,
            4 => InvalidAccountData,
            5 => AccountDataTooSmall,
            6 => InsufficientFunds,
            7 => IncorrectProgramId,
            8 => MissingRequiredSignature,
            9 => AccountAlreadyInitialized,
            10 => UninitializedAccount,
            11 => NotEnoughAccountKeys,
            12 => AccountBorrowFailed,
            13 => MaxSeedLengthExceeded,
            14 => InvalidSeeds,
            15 => BorshIoError,
            16 => AccountNotRentExempt,
            17 => UnsupportedSysvar,
            18 => IllegalOwner,
            19 => MaxAccountsDataAllocationsExceeded,
            20 => InvalidRealloc,
            21 => MaxInstructionTraceLengthExceeded,
            22 => BuiltinProgramsMustConsumeComputeUnits,
            23 => InvalidAccountOwner,
            24 => ArithmeticOverflow,
            25 => Immutable,
            26 => IncorrectAuthority,
            _ => InvalidError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn return_codes() {
        use InstructionError::*;
        assert_eq!(InstructionError::from_return_code(7), Custom(7));
        assert_eq!(InstructionError::from_return_code(1 << 32), Custom(0));
        assert_eq!(
            InstructionError::from_return_code(3 << 32),
            InvalidInstructionData
        );
        assert_eq!(InstructionError::from_return_code(27 << 32), InvalidError);
        assert_eq!(
            InstructionError::from_return_code((2 << 32) | 1),
            InvalidError
        );
    }
}
