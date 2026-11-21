use camshaft::error::VmError;
use camshaft::executable::Executable;
use camshaft::insn::{Insn, decode_at, split_slots};
use camshaft::interp::Vm;
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf, process::ExitCode};

/// Solana's default compute budget for one instruction. Until compute units arrive in
/// milestone 6, each instruction counts as one unit.
const MAX_INSTRUCTIONS: u64 = 200_000;

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
    /// Run a v0 program (.so) and print its logs and result
    Run {
        /// ELF file built by `cargo build-sbf`
        path: PathBuf,
    },
}

fn main() -> ExitCode {
    let result = match Cli::parse().command {
        Command::Disasm { path } => read(&path).and_then(|bytes| {
            let listing = camshaft::disasm::disassemble(&bytes).map_err(|e| e.to_string())?;
            print!("{listing}");
            Ok(())
        }),
        Command::Run { path } => read(&path).and_then(|bytes| run(&bytes)),
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

fn run(bytes: &[u8]) -> Result<(), String> {
    let exe = Executable::from_elf(bytes).map_err(|e| format!("cannot load: {e}"))?;
    exe.verify()
        .map_err(|e| format!("verification failed: {e}"))?;
    let mut vm = Vm::new(&exe);
    let result = vm.run(MAX_INSTRUCTIONS);
    for line in &vm.logs.lines {
        println!("{line}");
    }
    match result {
        Ok(r0) => {
            println!("Program returned {r0} after {} instructions", vm.insn_count);
            Ok(())
        }
        Err(VmError::UnsupportedInstruction) if let Some(name) = unknown_syscall(&exe, vm.pc) => {
            Err(format!("unknown syscall `{name}` at pc {}", vm.pc))
        }
        Err(e) => Err(format!("{e} at pc {}", vm.pc)),
    }
}

/// The name of the syscall a `call` at `pc` asks for, if the ELF named it.
fn unknown_syscall(exe: &Executable, pc: usize) -> Option<&str> {
    let slots = split_slots(exe.text_bytes()).ok()?;
    match decode_at(slots, pc).ok()? {
        Insn::Call { imm } => exe.syscall_names.get(&(imm as u32)).map(String::as_str),
        _ => None,
    }
}
