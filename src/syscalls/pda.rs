//! Program derived addresses: `sol_create_program_address` and `sol_try_find_program_address`.
//! Reference: Agave `syscalls/src/lib.rs` and `Pubkey::create_program_address`.

use super::{check_outputs, read, read_slices};
use crate::context::Pubkey;
use crate::error::SyscallError;
use crate::interp::Vm;
use crate::memory::Memory;
use curve25519_dalek::edwards::CompressedEdwardsY;
use sha2::{Digest, Sha256};

const COST: u64 = 1500;
const MAX_SEEDS: usize = 16;
const MAX_SEED_LEN: u64 = 32;

/// sha256(seeds, program id, "ProgramDerivedAddress"), unless that lands on the ed25519 curve.
/// A PDA must not be on the curve, so that no private key can sign for it.
pub fn create_program_address(seeds: &[&[u8]], program_id: &[u8]) -> Option<Pubkey> {
    if seeds.len() > MAX_SEEDS || seeds.iter().any(|seed| seed.len() as u64 > MAX_SEED_LEN) {
        return None;
    }
    let mut hasher = Sha256::new();
    for seed in seeds {
        hasher.update(seed);
    }
    hasher.update(program_id);
    hasher.update(b"ProgramDerivedAddress");
    let hash: Pubkey = hasher.finalize().into();
    let on_curve = CompressedEdwardsY(hash).decompress().is_some();
    (!on_curve).then_some(hash)
}

/// Returns 0 and writes the address, or returns 1 if the seeds land on the curve.
pub(super) fn create(
    vm: &mut Vm,
    [seeds, count, program_id, address, _]: [u64; 5],
) -> Result<u64, SyscallError> {
    vm.ctx.consume(COST)?;
    let (seeds, program_id) = seeds_and_program_id(&vm.memory, seeds, count, program_id)?;
    let Some(pda) = create_program_address(&seeds, program_id) else {
        return Ok(1);
    };
    check_outputs(&mut vm.memory, &[(address, 32)])?;
    vm.memory.slice_mut(address, 32)?.copy_from_slice(&pda);
    Ok(0)
}

/// Tries bump seeds 255 down to 1, paying for each miss. Returns 0 and writes the address and
/// bump, or returns 1 if none works.
pub(super) fn try_find(
    vm: &mut Vm,
    [seeds, count, program_id, address, bump_addr]: [u64; 5],
) -> Result<u64, SyscallError> {
    vm.ctx.consume(COST)?;
    let (seeds, program_id) = seeds_and_program_id(&vm.memory, seeds, count, program_id)?;
    let mut bump = u8::MAX;
    for _ in 0..u8::MAX {
        let bump_seed = [bump];
        let with_bump: Vec<&[u8]> = seeds.iter().copied().chain([&bump_seed[..]]).collect();
        if let Some(pda) = create_program_address(&with_bump, program_id) {
            check_outputs(&mut vm.memory, &[(bump_addr, 1), (address, 32)])?;
            vm.memory.slice_mut(bump_addr, 1)?[0] = bump;
            vm.memory.slice_mut(address, 32)?.copy_from_slice(&pda);
            return Ok(0);
        }
        bump = bump.saturating_sub(1);
        vm.ctx.consume(COST)?;
    }
    Ok(1)
}

/// Reads the seed list and the 32-byte program id. More than 16 seeds, or a seed over
/// 32 bytes, is an error here rather than a return value of 1.
fn seeds_and_program_id<'m>(
    memory: &'m Memory<'_>,
    seeds: u64,
    count: u64,
    program_id: u64,
) -> Result<(Vec<&'m [u8]>, &'m [u8]), SyscallError> {
    let slices = read_slices(memory, seeds, count)?;
    if slices.len() > MAX_SEEDS {
        return Err(SyscallError::BadSeeds);
    }
    let seeds = slices
        .into_iter()
        .map(|(ptr, len)| {
            if len > MAX_SEED_LEN {
                Err(SyscallError::BadSeeds)
            } else {
                read(memory, ptr, len)
            }
        })
        .collect::<Result<_, _>>()?;
    Ok((seeds, memory.slice(program_id, 32)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exact addresses are checked against Agave in tests/runtime.rs. Here: the result is off
    /// the curve, and a seed over 32 bytes is refused.
    #[test]
    fn address_is_off_curve_and_seeds_are_limited() {
        let program = [7; 32];
        // About half of all hashes are on the curve, so some seeds work and some don't.
        let found: Vec<_> = (0u8..16)
            .filter_map(|i| create_program_address(&[&[i]], &program))
            .collect();
        assert!(!found.is_empty() && found.len() < 16);
        assert!(
            found
                .iter()
                .all(|pda| CompressedEdwardsY(*pda).decompress().is_none())
        );
        assert_eq!(create_program_address(&[&[0; 33]], &program), None);
    }
}
