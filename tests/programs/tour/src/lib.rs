//! Uses every syscall camshaft supports, so its runs can be compared with Agave's.
//! The first byte of the instruction data picks a scenario: 0 is the happy path, the others
//! each fail in one particular way. Built with `cargo build-sbf` into `tests/programs/tour.so`.
//!
//! Accounts: 0 is writable and owned by this program, with at least 32 bytes of data.
//! 1 is read-only.

use solana_program::{
    account_info::AccountInfo,
    clock::Clock,
    entrypoint,
    entrypoint::ProgramResult,
    hash::hashv,
    keccak,
    log::{sol_log_64, sol_log_compute_units},
    msg,
    program::{get_return_data, set_return_data},
    program_error::ProgramError,
    program_memory::{sol_memcmp, sol_memcpy, sol_memmove, sol_memset},
    pubkey::Pubkey,
    rent::Rent,
    sysvar::Sysvar,
};

extern "C" {
    fn sol_sha256(vals: *const u8, len: u64, out: *mut u8) -> u64;
}

entrypoint!(process);

fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    match data.first().copied().unwrap_or(0) {
        0 => tour(program_id, accounts),
        1 => panic!("tour panicked"),
        2 => Err(ProgramError::Custom(42)),
        // Writes to the read-only account.
        3 => {
            accounts[1].try_borrow_mut_data()?[0] = 1;
            Ok(())
        }
        // Creates a lamport out of nothing.
        4 => {
            **accounts[0].try_borrow_mut_lamports()? += 1;
            Ok(())
        }
        // Reads unmapped memory.
        5 => {
            let value = unsafe { core::ptr::read_volatile(8 as *const u64) };
            sol_log_64(value, 0, 0, 0, 0);
            Ok(())
        }
        // Never stops.
        6 => {
            let mut x = 0u64;
            loop {
                x = core::hint::black_box(x + 1);
            }
        }
        // 16 seeds plus a bump is too many, so every bump fails and each one costs 1500.
        7 => {
            let seeds = [&b"x"[..]; 16];
            Pubkey::find_program_address(&seeds, program_id);
            Ok(())
        }
        // More than 10,000 bytes of log.
        8 => {
            for i in 0..200 {
                msg!("line {} {}", i, "x".repeat(60));
            }
            Ok(())
        }
        // Hands account 0 to another owner while its data is not zero.
        9 => {
            accounts[0].assign(&Pubkey::default());
            Ok(())
        }
        // Asks sol_sha256 to hash more slices than it allows.
        10 => {
            let mut out = [0u8; 32];
            unsafe { sol_sha256(core::ptr::null(), 20_001, out.as_mut_ptr()) };
            Ok(())
        }
        _ => Err(ProgramError::InvalidInstructionData),
    }
}

fn tour(program_id: &Pubkey, accounts: &[AccountInfo]) -> ProgramResult {
    let hash = hashv(&[b"hello", b"world"]).to_bytes();
    Pubkey::new_from_array(hash).log();
    Pubkey::new_from_array(keccak::hashv(&[b"hello"]).to_bytes()).log();

    match Pubkey::create_program_address(&[b"tour"], program_id) {
        Ok(pda) => pda.log(),
        Err(_) => msg!("on curve"),
    }
    let (pda, bump) = Pubkey::find_program_address(&[b"tour"], program_id);
    pda.log();
    sol_log_64(bump.into(), 0, 0, 0, 0);

    let clock = Clock::get()?;
    sol_log_64(clock.slot, clock.unix_timestamp as u64, clock.epoch, 0, 0);
    let rent = Rent::get()?;
    #[allow(deprecated)]
    sol_log_64(rent.lamports_per_byte_year, rent.burn_percent.into(), rent.minimum_balance(100), 0, 0);

    let mut buf = [0u8; 32];
    let copy;
    let cmp;
    unsafe {
        sol_memset(&mut buf, 7, 32);
        sol_memcpy(&mut buf[..16], &hash[..16], 16);
        copy = buf;
        sol_memmove(buf[16..].as_mut_ptr(), copy.as_ptr(), 16);
        cmp = sol_memcmp(&buf, &hash, 32);
    }
    sol_log_64(cmp as u64, buf[16].into(), 0, 0, 0);

    accounts[0].try_borrow_mut_data()?[..32].copy_from_slice(&hash);

    set_return_data(b"tour");
    let (from, data) = get_return_data().unwrap();
    sol_log_64(data.len() as u64, (from == *program_id).into(), 0, 0, 0);
    sol_log_compute_units();
    Ok(())
}
