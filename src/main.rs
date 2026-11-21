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
}

fn main() -> ExitCode {
    let Command::Disasm { path } = Cli::parse().command;
    let listing = fs::read(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))
        .and_then(|bytes| camshaft::disasm::disassemble(&bytes).map_err(|e| e.to_string()));
    match listing {
        Ok(listing) => {
            print!("{listing}");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("camshaft: {e}");
            ExitCode::FAILURE
        }
    }
}
