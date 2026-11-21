//! A counter. Account 0 holds a u64 and must be owned by this program. The instruction data
//! is the amount to add as a u64, or empty to add 1.
//! Built with `cargo build-sbf` into `tests/programs/counter.so`.

use solana_program::{account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, msg};
use solana_program::{program_error::ProgramError, pubkey::Pubkey};

entrypoint!(process);

fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let counter = accounts.first().ok_or(ProgramError::NotEnoughAccountKeys)?;
    if counter.owner != program_id {
        return Err(ProgramError::IncorrectProgramId);
    }
    let amount = match data {
        [] => 1,
        _ => u64::from_le_bytes(data.try_into().map_err(|_| ProgramError::InvalidInstructionData)?),
    };
    let mut bytes = counter.try_borrow_mut_data()?;
    let slot = bytes.get_mut(..8).ok_or(ProgramError::AccountDataTooSmall)?;
    let count = u64::from_le_bytes(slot.try_into().unwrap());
    let count = count.checked_add(amount).ok_or(ProgramError::ArithmeticOverflow)?;
    slot.copy_from_slice(&count.to_le_bytes());
    msg!("count: {}", count);
    Ok(())
}
