//! JSON files that describe one instruction to run: the program id, the instruction data and
//! every account it names, with their state.
//!
//! ```json
//! {
//!   "program_id": "Counter111111111111111111111111111111111111",
//!   "instruction_data": "0500000000000000",
//!   "accounts": [
//!     { "pubkey": "...", "is_writable": true, "owner": "...", "lamports": 1000000, "data": "0000000000000000" }
//!   ]
//! }
//! ```
//!
//! Keys are base58, data is hex. A key listed twice is passed twice, like in a transaction;
//! its state comes from the first listing. Missing fields default to false, 0, empty, or the
//! system program as owner.

use crate::context::{Clock, Pubkey, Rent};
use crate::input::{Account, AccountMeta, Instruction};
use crate::runtime::{DEFAULT_COMPUTE_UNITS, Environment};
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Fixture {
    program_id: String,
    #[serde(default)]
    instruction_data: String,
    #[serde(default)]
    accounts: Vec<FixtureAccount>,
    compute_unit_limit: Option<u64>,
    #[serde(default)]
    clock: Clock,
    #[serde(default)]
    rent: Rent,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FixtureAccount {
    pubkey: String,
    #[serde(default)]
    is_signer: bool,
    #[serde(default)]
    is_writable: bool,
    #[serde(default = "system_program")]
    owner: String,
    #[serde(default)]
    lamports: u64,
    #[serde(default)]
    data: String,
    #[serde(default)]
    executable: bool,
}

fn system_program() -> String {
    "11111111111111111111111111111111".into()
}

/// An instruction to run, the state of its accounts, and the runtime around it.
#[derive(Debug)]
pub struct Parsed {
    pub instruction: Instruction,
    pub accounts: Vec<(Pubkey, Account)>,
    pub env: Environment,
}

pub fn parse(json: &str) -> Result<Parsed, String> {
    let fixture: Fixture = serde_json::from_str(json).map_err(|e| format!("bad fixture: {e}"))?;
    let mut metas = Vec::new();
    let mut accounts: Vec<(Pubkey, Account)> = Vec::new();
    for a in fixture.accounts {
        let key = pubkey(&a.pubkey)?;
        metas.push(AccountMeta {
            pubkey: key,
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        });
        if accounts.iter().all(|(k, _)| *k != key) {
            let account = Account {
                lamports: a.lamports,
                data: hex(&a.data)?,
                owner: pubkey(&a.owner)?,
                executable: a.executable,
            };
            accounts.push((key, account));
        }
    }
    Ok(Parsed {
        instruction: Instruction {
            program_id: pubkey(&fixture.program_id)?,
            accounts: metas,
            data: hex(&fixture.instruction_data)?,
        },
        accounts,
        env: Environment {
            clock: fixture.clock,
            rent: fixture.rent,
            compute_units: fixture.compute_unit_limit.unwrap_or(DEFAULT_COMPUTE_UNITS),
        },
    })
}

pub fn pubkey(text: &str) -> Result<Pubkey, String> {
    let bytes = bs58::decode(text)
        .into_vec()
        .map_err(|e| format!("bad key {text}: {e}"))?;
    bytes
        .try_into()
        .map_err(|_| format!("bad key {text}: not 32 bytes"))
}

fn hex(text: &str) -> Result<Vec<u8>, String> {
    if !text.len().is_multiple_of(2) {
        return Err(format!("bad hex {text}: odd length"));
    }
    (0..text.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&text[i..i + 2], 16).map_err(|e| format!("bad hex {text}: {e}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn repeated_keys_share_state_and_defaults_apply() {
        let json = r#"{
            "program_id": "11111111111111111111111111111111",
            "instruction_data": "0a0b",
            "accounts": [
                { "pubkey": "11111111111111111111111111111112", "is_writable": true, "lamports": 5, "data": "ff" },
                { "pubkey": "11111111111111111111111111111112", "is_signer": true }
            ]
        }"#;
        let parsed = parse(json).unwrap();
        assert_eq!(parsed.instruction.data, [0x0a, 0x0b]);
        assert_eq!(parsed.instruction.accounts.len(), 2);
        assert_eq!(parsed.accounts.len(), 1);
        assert_eq!(parsed.accounts[0].1.data, [0xff]);
        assert_eq!(parsed.accounts[0].1.owner, [0; 32]);
        assert_eq!(parsed.env.compute_units, DEFAULT_COMPUTE_UNITS);
    }

    #[test]
    fn bad_input_is_explained() {
        assert!(parse("{}").unwrap_err().contains("program_id"));
        assert!(hex("abc").unwrap_err().contains("odd length"));
        assert!(pubkey("0OIl").unwrap_err().contains("bad key"));
    }
}
