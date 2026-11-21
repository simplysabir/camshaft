//! Runs one instruction the way Agave's BPF loader does: serialize the accounts, run the
//! program, then check and apply what it changed. Reference: `execute` in
//! `programs/bpf_loader/src/lib.rs` and `process_executable_chain` in `invoke_context.rs`.

use crate::context::{Clock, Pubkey, Rent};
use crate::error::{InstructionError, SyscallError, VmError};
use crate::executable::Executable;
use crate::input::{self, Account, Instruction};
use crate::interp::Vm;
use crate::memory::INPUT_START;
use base64::Engine;

/// Solana's default compute budget for one instruction.
pub const DEFAULT_COMPUTE_UNITS: u64 = 200_000;

/// What the chain would see after the instruction.
#[derive(Debug)]
pub struct Outcome {
    pub result: Result<(), InstructionError>,
    /// Every log line, including the `invoke`, `consumed` and `success` lines.
    pub logs: Vec<String>,
    pub compute_units: u64,
    /// The accounts after the instruction. Unchanged if it failed.
    pub accounts: Vec<(Pubkey, Account)>,
    pub return_data: Vec<u8>,
    /// Instructions the VM ran.
    pub insn_count: u64,
    /// Where the VM stopped.
    pub pc: usize,
}

/// The runtime state an instruction sees besides its accounts.
#[derive(Debug, Clone)]
pub struct Environment {
    pub clock: Clock,
    pub rent: Rent,
    pub compute_units: u64,
}

impl Default for Environment {
    fn default() -> Self {
        Self {
            clock: Clock::default(),
            rent: Rent::default(),
            compute_units: DEFAULT_COMPUTE_UNITS,
        }
    }
}

pub fn process_instruction(
    exe: &Executable,
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
    env: &Environment,
) -> Outcome {
    let id = bs58::encode(ix.program_id).into_string();
    let mut vm = Vm::new(exe);
    vm.ctx.program_id = ix.program_id;
    vm.ctx.clock = env.clock.clone();
    vm.ctx.rent = env.rent.clone();
    vm.ctx.logs.push(format!("Program {id} invoke [1]"));

    let (result, accounts) = match run(&mut vm, ix, accounts, env.compute_units, &id) {
        Ok(after) => {
            vm.ctx.logs.push(format!("Program {id} success"));
            // The transaction checks the balance once the instruction is over, after the line above.
            if input::lamports(ix, &after) == input::lamports(ix, accounts) {
                (Ok(()), after)
            } else {
                (
                    Err(InstructionError::UnbalancedInstruction),
                    accounts.to_vec(),
                )
            }
        }
        Err((error, message)) => {
            vm.ctx.logs.push(format!("Program {id} failed: {message}"));
            (Err(error), accounts.to_vec())
        }
    };
    Outcome {
        result,
        logs: vm.ctx.logs.lines,
        compute_units: env.compute_units - vm.ctx.remaining,
        accounts,
        return_data: vm.ctx.return_data,
        insn_count: vm.insn_count,
        pc: vm.pc,
    }
}

/// Serializes, runs and deserializes. A failure carries the error and the text Agave logs.
fn run(
    vm: &mut Vm,
    ix: &Instruction,
    accounts: &[(Pubkey, Account)],
    budget: u64,
    id: &str,
) -> Result<Vec<(Pubkey, Account)>, (InstructionError, String)> {
    let failed = |e: InstructionError| (e.clone(), e.to_string());
    vm.memory.input = input::serialize(ix, accounts).map_err(failed)?;
    // r2 points at the instruction data, which sits just before the 32-byte program id.
    let data_offset = vm.memory.input.len() - 32 - ix.data.len();
    vm.reg[2] = INPUT_START + data_offset as u64;
    let result = vm.run(budget);

    let consumed = budget - vm.ctx.remaining;
    vm.ctx.logs.push(format!(
        "Program {id} consumed {consumed} of {budget} compute units"
    ));
    if !vm.ctx.return_data.is_empty() {
        let data = base64::engine::general_purpose::STANDARD.encode(&vm.ctx.return_data);
        vm.ctx.logs.push(format!("Program return: {id} {data}"));
    }

    match result {
        Ok(0) => {}
        Ok(code) => return Err(failed(InstructionError::from_return_code(code))),
        Err(VmError::SyscallError(e)) => {
            return Err(match *e {
                SyscallError::ComputationalBudgetExceeded => {
                    failed(InstructionError::ComputationalBudgetExceeded)
                }
                e => (InstructionError::ProgramFailedToComplete, e.to_string()),
            });
        }
        // A fault outside a syscall uses up the whole budget.
        Err(e) => {
            vm.ctx.remaining = 0;
            return Err((InstructionError::ProgramFailedToComplete, e.to_string()));
        }
    }
    input::deserialize(ix, accounts, &vm.memory.input).map_err(failed)
}
