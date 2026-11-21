//! Accounts and instruction data in the input region, laid out the way Agave's loader does
//! for programs owned by BPF loader 2 or the upgradeable loader.
//! Reference: Agave `program-runtime/src/serialization.rs` (aligned, stricter ABI off, rent
//! epoch masked, as on mainnet) and the account rules in `transaction-context`.

use crate::context::Pubkey;
use crate::error::InstructionError;

/// Room a program may grow each account's data by in one instruction.
const MAX_DATA_INCREASE: usize = 10 * 1024;
const MAX_DATA_LEN: u64 = 10 * 1024 * 1024;
/// Marks an account entry that is not a repeat of an earlier one.
const NON_DUP_MARKER: u8 = 0xff;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Account {
    pub lamports: u64,
    pub data: Vec<u8>,
    pub owner: Pubkey,
    pub executable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountMeta {
    pub pubkey: Pubkey,
    pub is_signer: bool,
    pub is_writable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Instruction {
    pub program_id: Pubkey,
    pub accounts: Vec<AccountMeta>,
    pub data: Vec<u8>,
}

/// One account the instruction names, after merging repeats of the same key.
struct Entry<'a> {
    key: Pubkey,
    account: &'a Account,
    is_signer: bool,
    is_writable: bool,
    /// For a repeat: the position of the first entry with this key.
    duplicate_of: Option<usize>,
}

/// Pairs each account the instruction names with its state. A key named twice is signer or
/// writable if any mention says so, like in a transaction. Fails if a key has no state.
fn entries<'a>(
    ix: &Instruction,
    accounts: &'a [(Pubkey, Account)],
) -> Result<Vec<Entry<'a>>, InstructionError> {
    ix.accounts
        .iter()
        .enumerate()
        .map(|(i, meta)| {
            let same_key = || ix.accounts.iter().filter(|m| m.pubkey == meta.pubkey);
            let (_, account) = accounts
                .iter()
                .find(|(key, _)| *key == meta.pubkey)
                .ok_or(InstructionError::NotEnoughAccountKeys)?;
            Ok(Entry {
                key: meta.pubkey,
                account,
                is_signer: same_key().any(|m| m.is_signer),
                is_writable: same_key().any(|m| m.is_writable),
                duplicate_of: ix.accounts[..i]
                    .iter()
                    .position(|m| m.pubkey == meta.pubkey),
            })
        })
        .collect()
}

/// Zero bytes that bring `len` up to a multiple of 8.
fn padding(len: usize) -> usize {
    len.next_multiple_of(8) - len
}

/// Builds the input region:
///
/// ```text
/// u64 number of accounts
/// per account:  u8 0xff, signer, writable, executable, 4 zero bytes, key, owner,
///               u64 lamports, u64 data length, data, 10 KiB of zeros + padding to 8,
///               u64 rent epoch (always u64::MAX)
///   or repeat:  u8 index of the first mention, 7 zero bytes
/// u64 instruction data length, instruction data, program id
/// ```
pub fn serialize(
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
) -> Result<Vec<u8>, InstructionError> {
    let mut out = Vec::new();
    out.extend((ix.accounts.len() as u64).to_le_bytes());
    for entry in entries(ix, accounts)? {
        if let Some(first) = entry.duplicate_of {
            out.push(first as u8);
            out.extend([0; 7]);
            continue;
        }
        let account = entry.account;
        out.extend([
            NON_DUP_MARKER,
            entry.is_signer as u8,
            entry.is_writable as u8,
            account.executable as u8,
        ]);
        out.extend([0; 4]);
        out.extend(entry.key);
        out.extend(account.owner);
        out.extend(account.lamports.to_le_bytes());
        out.extend((account.data.len() as u64).to_le_bytes());
        out.extend(&account.data);
        out.resize(
            out.len() + MAX_DATA_INCREASE + padding(account.data.len()),
            0,
        );
        out.extend(u64::MAX.to_le_bytes());
    }
    out.extend((ix.data.len() as u64).to_le_bytes());
    out.extend(&ix.data);
    out.extend(ix.program_id);
    Ok(out)
}

/// Reads the accounts back out of the input region after a successful run, and checks every
/// change against the runtime's rules. Returns the accounts in the order given.
pub fn deserialize(
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
    input: &[u8],
) -> Result<Vec<(Pubkey, Account)>, InstructionError> {
    let mut after = accounts.to_vec();
    let mut at = 8;
    for entry in entries(ix, accounts)? {
        if entry.duplicate_of.is_some() {
            at += 8;
            continue;
        }
        let (_, account) = after
            .iter_mut()
            .find(|(key, _)| *key == entry.key)
            .expect("found by entries");
        let rules = Rules {
            owned: account.owner == ix.program_id,
            writable: entry.is_writable,
        };
        at += 8 + 32; // flags, padding, key
        let owner: Pubkey = bytes(input, at, 32)?.try_into().expect("32 bytes");
        let lamports = u64_at(input, at + 32)?;
        let len = u64_at(input, at + 40)? as usize;
        at += 48;

        if lamports != account.lamports {
            rules.set_lamports(account.lamports, lamports)?;
            account.lamports = lamports;
        }
        let pre_len = entry.account.data.len();
        if len.saturating_sub(pre_len) > MAX_DATA_INCREASE || len as u64 > MAX_DATA_LEN {
            return Err(InstructionError::InvalidRealloc);
        }
        let data = bytes(input, at, len)?;
        // Unchanged data is fine even where changing it would not be.
        match rules.resize(pre_len, len) {
            Ok(()) => account.data = data.to_vec(),
            Err(e) if account.data != data => return Err(e),
            Err(_) => {}
        }
        at += pre_len + MAX_DATA_INCREASE + padding(pre_len) + 8;
        // The owner changes last, so the lamport and data rules above use the old owner.
        if owner != account.owner {
            rules.set_owner(&account.data)?;
            account.owner = owner;
        }
    }
    Ok(after)
}

/// Agave's `BorrowedAccount` checks for one account.
struct Rules {
    /// The running program owns the account (before any owner change).
    owned: bool,
    writable: bool,
}

impl Rules {
    fn set_lamports(&self, before: u64, after: u64) -> Result<(), InstructionError> {
        if !self.owned && after < before {
            return Err(InstructionError::ExternalAccountLamportSpend);
        }
        if !self.writable {
            return Err(InstructionError::ReadonlyLamportChange);
        }
        Ok(())
    }

    fn resize(&self, before: usize, after: usize) -> Result<(), InstructionError> {
        if before != after && !self.owned {
            return Err(InstructionError::AccountDataSizeChanged);
        }
        if !self.writable {
            return Err(InstructionError::ReadonlyDataModified);
        }
        if !self.owned {
            return Err(InstructionError::ExternalAccountDataModified);
        }
        Ok(())
    }

    /// Only the owner can hand an account to a new owner, and only once its data is zeroed.
    fn set_owner(&self, data: &[u8]) -> Result<(), InstructionError> {
        if !self.owned || !self.writable || data.iter().any(|&b| b != 0) {
            return Err(InstructionError::ModifiedProgramId);
        }
        Ok(())
    }
}

fn bytes(input: &[u8], at: usize, len: usize) -> Result<&[u8], InstructionError> {
    input
        .get(at..at.saturating_add(len))
        .ok_or(InstructionError::InvalidArgument)
}

fn u64_at(input: &[u8], at: usize) -> Result<u64, InstructionError> {
    Ok(u64::from_le_bytes(
        bytes(input, at, 8)?.try_into().expect("8 bytes"),
    ))
}

/// Total lamports of the accounts an instruction names, each key counted once.
pub fn lamports(ix: &Instruction, accounts: &[(Pubkey, Account)]) -> u128 {
    accounts
        .iter()
        .filter(|(key, _)| ix.accounts.iter().any(|m| m.pubkey == *key))
        .map(|(_, account)| u128::from(account.lamports))
        .sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup(writable: bool, owner: Pubkey) -> (Instruction, Vec<(Pubkey, Account)>) {
        let key = [1; 32];
        let account = Account {
            lamports: 100,
            data: vec![0; 8],
            owner,
            executable: false,
        };
        let meta = AccountMeta {
            pubkey: key,
            is_signer: false,
            is_writable: writable,
        };
        let ix = Instruction {
            program_id: [9; 32],
            accounts: vec![meta.clone(), meta],
            data: vec![5, 6],
        };
        (ix, vec![(key, account)])
    }

    /// Offset of the account's data in the input, after the count and its 88-byte header.
    const DATA: usize = 8 + 88;

    #[test]
    fn layout_matches_agave() {
        let (ix, accounts) = setup(true, [9; 32]);
        let input = serialize(&ix, &accounts).unwrap();
        assert_eq!(input[..8], 2u64.to_le_bytes());
        assert_eq!(input[8..12], [0xff, 0, 1, 0]);
        assert_eq!(input[DATA - 16..DATA - 8], 100u64.to_le_bytes());
        // Data, its 10 KiB of room, then the masked rent epoch.
        let rent_epoch = DATA + 8 + MAX_DATA_INCREASE;
        assert_eq!(input[rent_epoch..rent_epoch + 8], u64::MAX.to_le_bytes());
        // The second mention is a repeat of entry 0.
        assert_eq!(input[rent_epoch + 8..rent_epoch + 16], [0; 8]);
        let tail = &input[rent_epoch + 16..];
        assert_eq!(tail, [&2u64.to_le_bytes()[..], &[5, 6], &[9; 32]].concat());
    }

    #[test]
    fn owner_may_change_its_writable_account() {
        let (ix, accounts) = setup(true, [9; 32]);
        let mut input = serialize(&ix, &accounts).unwrap();
        input[DATA] = 7;
        input[DATA - 16..DATA - 8].copy_from_slice(&90u64.to_le_bytes());
        let after = deserialize(&ix, &accounts, &input).unwrap();
        assert_eq!((after[0].1.lamports, after[0].1.data[0]), (90, 7));
    }

    #[test]
    fn changes_the_runtime_forbids() {
        let change_data = |input: &mut Vec<u8>| input[DATA] = 7;
        let spend =
            |input: &mut Vec<u8>| input[DATA - 16..DATA - 8].copy_from_slice(&90u64.to_le_bytes());
        // (writable, owner, what the program does, the error it causes)
        type Case<'a> = (bool, Pubkey, &'a dyn Fn(&mut Vec<u8>), InstructionError);
        let cases: [Case; 4] = [
            (
                false,
                [9; 32],
                &change_data,
                InstructionError::ReadonlyDataModified,
            ),
            (
                true,
                [3; 32],
                &change_data,
                InstructionError::ExternalAccountDataModified,
            ),
            (
                true,
                [3; 32],
                &spend,
                InstructionError::ExternalAccountLamportSpend,
            ),
            (
                false,
                [9; 32],
                &spend,
                InstructionError::ReadonlyLamportChange,
            ),
        ];
        for (writable, owner, change, expected) in cases {
            let (ix, accounts) = setup(writable, owner);
            let mut input = serialize(&ix, &accounts).unwrap();
            change(&mut input);
            assert_eq!(deserialize(&ix, &accounts, &input), Err(expected));
        }
    }
}
