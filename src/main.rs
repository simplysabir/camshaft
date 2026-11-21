use camshaft::elf::FunctionNames;
use camshaft::error::InstructionError;
use camshaft::executable::Executable;
use camshaft::fixture::{self, Parsed};
use camshaft::input::{Account, Instruction};
use camshaft::insn::{decode_at, split_slots};
use camshaft::profile::Profiler;
use camshaft::runtime::{Environment, Outcome, process_instruction, process_instruction_observed};
use camshaft::syscalls;
use camshaft::trace::{self, Tracer};
use camshaft::version::Version;
use clap::{Args, Parser, Subcommand};
use std::io::{self, Write};
use std::{fs, path::PathBuf, process::ExitCode};

#[derive(Parser)]
#[command(version, about = "An sBPF virtual machine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print raw sBPF bytecode as assembly
    Disasm {
        /// Raw bytecode file (no ELF header yet)
        path: PathBuf,
        /// Decode as v3 instead of v0
        #[arg(long)]
        v3: bool,
    },
    /// Run a v0 or v3 program (.so) and print its logs, result and account changes
    Run(Program),
    /// Run, printing one line per instruction: compute units, pc, instruction, changes
    Trace(Program),
    /// Run, then print the compute units each function used, most first
    Profile {
        #[command(flatten)]
        program: Program,
        /// Also write folded stacks here, for a flame graph
        #[arg(long)]
        folded: Option<PathBuf>,
    },
}

/// A program and the instruction to run it with.
#[derive(Args)]
struct Program {
    /// ELF file built by `cargo build-sbf`. For function names in a profile, use the
    /// unstripped copy in `target/<target>/release/`
    path: PathBuf,
    /// JSON file with the accounts and instruction data (see docs/06-input-and-compute.md)
    #[arg(long)]
    input: Option<PathBuf>,
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Disasm { path, v3 } => read(&path).and_then(|bytes| {
            let version = if v3 { Version::V3 } else { Version::V0 };
            let listing =
                camshaft::disasm::disassemble(&bytes, version).map_err(|e| e.to_string())?;
            print!("{listing}");
            Ok(())
        }),
        Command::Run(program) => run(&program),
        Command::Trace(program) => trace(&program),
        Command::Profile { program, folded } => profile(&program, folded.as_ref()),
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

/// Loads and verifies the program, reads the input, and names the functions.
fn load(program: &Program) -> Result<(Executable, Parsed, FunctionNames), String> {
    let bytes = read(&program.path)?;
    let exe = Executable::from_elf(&bytes).map_err(|e| format!("cannot load: {e}"))?;
    exe.verify()
        .map_err(|e| format!("verification failed: {e}"))?;
    let parsed = match &program.input {
        Some(input) => {
            let json = fs::read_to_string(input)
                .map_err(|e| format!("cannot read {}: {e}", input.display()))?;
            fixture::parse(&json)?
        }
        // No input: no accounts, no data, and an all-zero program id.
        None => Parsed {
            instruction: Instruction {
                program_id: [0; 32],
                accounts: Vec::new(),
                data: Vec::new(),
            },
            accounts: Vec::new(),
            env: Environment::default(),
        },
    };
    let names = FunctionNames::read(&bytes, exe.entry_pc);
    Ok((exe, parsed, names))
}

fn run(program: &Program) -> Result<(), String> {
    let (exe, parsed, _) = load(program)?;
    let outcome = process_instruction(&exe, &parsed.instruction, &parsed.accounts, &parsed.env);
    report(&exe, &parsed, &outcome)
}

fn trace(program: &Program) -> Result<(), String> {
    let (exe, parsed, names) = load(program)?;
    println!("{}", trace::HEADER);
    let stdout = io::stdout().lock();
    let mut tracer = Tracer::new(stdout, &exe, &names);
    let outcome = process_instruction_observed(
        &exe,
        &parsed.instruction,
        &parsed.accounts,
        &parsed.env,
        &mut tracer,
    );
    tracer
        .finish()
        .and_then(|mut out| out.flush())
        .map_err(|e| format!("cannot write the trace: {e}"))?;
    println!();
    report(&exe, &parsed, &outcome)
}

fn profile(program: &Program, folded: Option<&PathBuf>) -> Result<(), String> {
    let (exe, parsed, names) = load(program)?;
    let mut profiler = Profiler::new(exe.entry_pc);
    let outcome = process_instruction_observed(
        &exe,
        &parsed.instruction,
        &parsed.accounts,
        &parsed.env,
        &mut profiler,
    );
    if let Some(path) = folded {
        fs::write(path, profiler.folded(&names))
            .map_err(|e| format!("cannot write {}: {e}", path.display()))?;
    }
    let result = report(&exe, &parsed, &outcome);
    println!();
    print!("{}", profiler.report(&names, outcome.compute_units));
    result
}

/// Prints the logs, compute units and account changes, then turns the result into the exit
/// status.
fn report(exe: &Executable, parsed: &Parsed, outcome: &Outcome) -> Result<(), String> {
    for line in &outcome.logs {
        println!("{line}");
    }
    println!();
    println!("Compute units: {}", outcome.compute_units);
    println!("Instructions: {}", outcome.insn_count);
    print_changes(&parsed.accounts, &outcome.accounts);
    match &outcome.result {
        Ok(()) => Ok(()),
        Err(_) if let Some(name) = unknown_syscall(exe, outcome) => {
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
    let slots = split_slots(&exe.text).ok()?;
    let key = decode_at(slots, outcome.pc, exe.version)
        .ok()?
        .syscall_key()?;
    if syscalls::lookup(key).is_some() || exe.functions.contains_key(&key) {
        return None;
    }
    exe.syscall_names.get(&key).map(String::as_str)
}
