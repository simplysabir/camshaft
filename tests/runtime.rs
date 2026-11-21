//! Runs real programs on camshaft's runtime and on Agave's (through mollusk), and checks they
//! agree on the result, every log line, compute units, final accounts and return data.

use camshaft::executable::Executable;
use camshaft::fixture::{self, Parsed};
use camshaft::runtime::{Environment, Outcome, process_instruction};
use mollusk_svm::Mollusk;
use mollusk_svm::program::{ProgramCache, loader_keys};
use solana_account::Account;
use solana_clock::Clock;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;
use solana_rent::Rent;
use solana_svm_feature_set::SVMFeatureSet;
use solana_svm_log_collector::LogCollector;
use std::fs;

/// Agave 4.4 with mainnet's feature flags for what camshaft models.
fn agave(env: &Environment) -> Mollusk {
    let (clock, rent) = (&env.clock, &env.rent);
    let mut mollusk = Mollusk {
        feature_set: mainnet_features(),
        logger: Some(LogCollector::new_ref()),
        ..Mollusk::default()
    };
    mollusk.compute_budget.compute_unit_limit = env.compute_units;
    mollusk.program_cache = ProgramCache::new(&mollusk.feature_set, &mollusk.compute_budget, false);
    mollusk.sysvars.clock = Clock {
        slot: clock.slot,
        epoch_start_timestamp: clock.epoch_start_timestamp,
        epoch: clock.epoch,
        leader_schedule_epoch: clock.leader_schedule_epoch,
        unix_timestamp: clock.unix_timestamp,
    };
    #[allow(deprecated)]
    let rent = Rent {
        lamports_per_byte: rent.lamports_per_byte_year,
        exemption_threshold: rent.exemption_threshold.to_le_bytes(),
        burn_percent: rent.burn_percent,
    };
    mollusk.sysvars.rent = rent;
    mollusk
}

/// Mainnet's flags as of 2026-11-21: everything on except these.
fn mainnet_features() -> SVMFeatureSet {
    SVMFeatureSet {
        virtual_address_space_adjustments: false,
        account_data_direct_mapping: false,
        direct_account_pointers_in_program_input: false,
        blake3_syscall_enabled: false,
        enable_big_mod_exp_syscall: false,
        remaining_compute_units_syscall_enabled: false,
        enable_sha512_syscall: false,
        disable_sbpf_v0_execution: false,
        reenable_sbpf_v0_execution: false,
        disable_sbpf_v0_v1_v2_deployment: false,
        deprecate_legacy_vote_ixs: false,
        increase_tx_account_lock_limit: false,
        raise_cpi_nesting_limit_to_8: false,
        block_revenue_sharing: false,
        vote_account_initialize_v2: false,
        relax_fee_payer_constraint: false,
        loader_v3_set_program_data_to_elf_length: false,
        ..SVMFeatureSet::all_enabled()
    }
}

/// Runs `program` with `parsed` on both sides, asserts they agree, and returns camshaft's run.
fn compare(program: &str, parsed: &Parsed) -> Outcome {
    let elf = fs::read(format!("tests/programs/{program}.so")).unwrap();
    let exe = Executable::from_elf(&elf).unwrap();
    exe.verify().unwrap();
    let ours = process_instruction(&exe, &parsed.instruction, &parsed.accounts, &parsed.env);

    let mut mollusk = agave(&parsed.env);
    let program_id = Pubkey::new_from_array(parsed.instruction.program_id);
    mollusk.add_program_with_loader_and_elf(&program_id, &loader_keys::LOADER_V2, &elf);
    let instruction = Instruction {
        program_id,
        accounts: parsed
            .instruction
            .accounts
            .iter()
            .map(|m| AccountMeta {
                pubkey: Pubkey::new_from_array(m.pubkey),
                is_signer: m.is_signer,
                is_writable: m.is_writable,
            })
            .collect(),
        data: parsed.instruction.data.clone(),
    };
    let accounts: Vec<(Pubkey, Account)> = parsed
        .accounts
        .iter()
        .map(|(key, a)| {
            let account = Account {
                lamports: a.lamports,
                data: a.data.clone(),
                owner: Pubkey::new_from_array(a.owner),
                executable: a.executable,
                rent_epoch: 0,
            };
            (Pubkey::new_from_array(*key), account)
        })
        .collect();
    let agave = mollusk.process_instruction(&instruction, &accounts);
    let agave_logs = mollusk
        .logger
        .as_ref()
        .unwrap()
        .borrow()
        .get_recorded_content()
        .to_vec();

    let what = format!("{program} with data {:02x?}", parsed.instruction.data);
    assert_eq!(ours.logs, agave_logs, "logs: {what}");
    assert_eq!(
        format!("{:?}", ours.result),
        format!("{:?}", agave.raw_result),
        "result: {what}"
    );
    assert_eq!(
        ours.compute_units, agave.compute_units_consumed,
        "compute units: {what}"
    );
    assert_eq!(ours.return_data, agave.return_data, "return data: {what}");
    let simplify = |key: &Pubkey, a: &Account| {
        (
            key.to_bytes(),
            a.lamports,
            a.data.clone(),
            a.owner.to_bytes(),
        )
    };
    let expected: Vec<_> = agave
        .resulting_accounts
        .iter()
        .map(|(k, a)| simplify(k, a))
        .collect();
    let got: Vec<_> = ours
        .accounts
        .iter()
        .map(|(k, a)| (*k, a.lamports, a.data.clone(), a.owner))
        .collect();
    assert_eq!(got, expected, "accounts: {what}");
    ours
}

fn fixture(name: &str) -> Parsed {
    fixture::parse(&fs::read_to_string(format!("tests/programs/{name}.json")).unwrap()).unwrap()
}

#[test]
fn counter_adds_to_its_account() {
    let ours = compare("counter", &fixture("counter"));
    assert_eq!(ours.result, Ok(()));
    assert_eq!(ours.accounts[0].1.data, 7u64.to_le_bytes());
}

#[test]
fn counter_errors() {
    // Not owned by the program, bad instruction data, and overflow.
    let mut not_owned = fixture("counter");
    not_owned.accounts[0].1.owner = [0; 32];
    let mut bad_data = fixture("counter");
    bad_data.instruction.data = vec![1, 2, 3];
    let mut overflow = fixture("counter");
    overflow.accounts[0].1.data = u64::MAX.to_le_bytes().to_vec();
    for parsed in [not_owned, bad_data, overflow] {
        assert!(compare("counter", &parsed).result.is_err());
    }
}

#[test]
fn spl_memo_from_mainnet() {
    let ours = compare("spl_memo", &fixture("spl_memo"));
    assert_eq!(ours.result, Ok(()));
    // A memo whose account did not sign, and one that is not UTF-8.
    let mut unsigned = fixture("spl_memo");
    unsigned.instruction.accounts[0].is_signer = false;
    let mut not_utf8 = fixture("spl_memo");
    not_utf8.instruction.data = vec![0xff, 0xfe];
    for parsed in [unsigned, not_utf8] {
        assert!(compare("spl_memo", &parsed).result.is_err());
    }
}

/// Scenario 0 uses every syscall, 11 logs r2, and the others each fail in a different way.
#[test]
fn tour_of_every_syscall_and_failure() {
    for scenario in 0..=13u8 {
        let mut parsed = fixture("tour");
        parsed.instruction.data = vec![scenario];
        if scenario == 12 {
            parsed.instruction.accounts[1].is_writable = true;
            parsed.accounts[1].1.owner = [5; 32];
        }
        compare("tour", &parsed);
    }
}

#[test]
fn hello_with_no_accounts() {
    let parsed = fixture::parse(r#"{ "program_id": "11111111111111111111111111111111" }"#).unwrap();
    compare("hello", &parsed);
}
