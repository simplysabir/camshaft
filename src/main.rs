use camshaft::error::InstructionError;
use camshaft::executable::Executable;
use camshaft::fixture;
use camshaft::input::{Account, Instruction};
use camshaft::insn::{Insn, decode_at, split_slots};
use camshaft::runtime::{Environment, Outcome, process_instruction};
use camshaft::syscalls;
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(version, about = "An sBPF virtual machine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print raw sBPF v0 bytecode as assembly
    Disasm {
        /// Raw bytecode file (no ELF header yet)
        path: PathBuf,
    },
    /// Run a v0 program (.so) and print its logs, result and account changes
    Run {
        /// ELF file built by `cargo build-sbf`
        path: PathBuf,
        /// JSON file with the accounts and instruction data (see docs/06-input-and-compute.md)
        #[arg(long)]
        input: Option<PathBuf>,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Disasm { path } => read(&path).and_then(|bytes| {
            let listing = camshaft::disasm::disassemble(&bytes).map_err(|e| e.to_string())?;
            print!("{listing}");
            Ok(())
        }),
        Command::Run { path, input } => run(&path, input.as_ref()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("camshaft: {e}");
            ExitCode::FAILURE
        }
    }
}

fn read(path: &PathBuf) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))
}

fn run(path: &PathBuf, input: Option<&PathBuf>) -> Result<(), String> {
    let exe = Executable::from_elf(&read(path)?).map_err(|e| format!("cannot load: {e}"))?;
    exe.verify()
        .map_err(|e| format!("verification failed: {e}"))?;
    let parsed = match input {
        Some(input) => {
            let json = fs::read_to_string(input)
                .map_err(|e| format!("cannot read {}: {e}", input.display()))?;
            fixture::parse(&json)?
        }
        // No input: no accounts, no data, and an all-zero program id.
        None => fixture::Parsed {
            instruction: Instruction {
                program_id: [0; 32],
                accounts: Vec::new(),
                data: Vec::new(),
            },
            accounts: Vec::new(),
            env: Environment::default(),
        },
    };
    let outcome = process_instruction(&exe, &parsed.instruction, &parsed.accounts, &parsed.env);
    for line in &outcome.logs {
        println!("{line}");
    }
    println!();
    println!("Compute units: {}", outcome.compute_units);
    println!("Instructions: {}", outcome.insn_count);
    print_changes(&parsed.accounts, &outcome.accounts);
    match &outcome.result {
        Ok(()) => Ok(()),
        Err(_) if let Some(name) = unknown_syscall(&exe, &outcome) => {
            Err(format!("unknown syscall `{name}` at pc {}", outcome.pc))
        }
        Err(e) => Err(e.to_string()),
    }
}

fn print_changes(before: &[([u8; 32], Account)], after: &[([u8; 32], Account)]) {
    let hex = |data: &[u8]| data.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let key = |k: &[u8; 32]| bs58::encode(k).into_string();
    for ((pubkey, old), (_, new)) in before
        .iter()
        .zip(after)
        .filter(|((_, old), (_, new))| old != new)
    {
        println!("Changed {}:", key(pubkey));
        if old.lamports != new.lamports {
            println!("  lamports: {} -> {}", old.lamports, new.lamports);
        }
        if old.owner != new.owner {
            println!("  owner: {} -> {}", key(&old.owner), key(&new.owner));
        }
        if old.data != new.data {
            println!("  data: {} -> {}", hex(&old.data), hex(&new.data));
        }
    }
}

/// The name of the syscall the program failed on, if it stopped at a `call` to one camshaft
/// does not have.
fn unknown_syscall<'e>(exe: &'e Executable, outcome: &Outcome) -> Option<&'e str> {
    if outcome.result != Err(InstructionError::ProgramFailedToComplete) {
        return None;
    }
    let slots = split_slots(exe.text_bytes()).ok()?;
    let Insn::Call { imm } = decode_at(slots, outcome.pc).ok()? else {
        return None;
    };
    let key = imm as u32;
    if syscalls::lookup(key).is_some() || exe.functions.contains_key(&key) {
        return None;
    }
    exe.syscall_names.get(&key).map(String::as_str)
}
