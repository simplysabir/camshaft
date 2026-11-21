//! `camshaft trace`: one line per instruction.
//!
//! ```text
//!      CU     pc  instruction                       changes
//!       3      2  stxdw [r10-0x8], r1               [0x200000ff8]=0x400000000
//!     104      3  syscall sol_log_                  r0=0x0
//! ```
//!
//! CU is the running total, including the line's own cost. `changes` lists the registers the
//! instruction changed and the memory it wrote: the value for up to 8 bytes, else the range.

use crate::disasm::format_insn;
use crate::elf::FunctionNames;
use crate::executable::Executable;
use crate::insn::{CallTarget, Insn};
use crate::interp::{Observer, Step, Vm};
use crate::syscalls;
use std::io::{self, Write};

pub const HEADER: &str = "     CU     pc  instruction                       changes";

pub struct Tracer<'a, W: Write> {
    out: W,
    exe: &'a Executable,
    names: &'a FunctionNames,
    /// Compute units used so far.
    used: u64,
    /// The first write error. Later lines are skipped.
    error: Option<io::Error>,
}

impl<W: Write> Observer for Tracer<'_, W> {
    const RECORDS_WRITES: bool = true;

    fn after(&mut self, vm: &Vm, step: &Step) {
        self.used += step.cost;
        if self.error.is_none() {
            let line = self.line(vm, step);
            self.error = writeln!(self.out, "{line}").err();
        }
    }
}

impl<'a, W: Write> Tracer<'a, W> {
    pub fn new(out: W, exe: &'a Executable, names: &'a FunctionNames) -> Self {
        Self {
            out,
            exe,
            names,
            used: 0,
            error: None,
        }
    }

    /// The output, or the first error writing it.
    pub fn finish(self) -> io::Result<W> {
        match self.error {
            Some(e) => Err(e),
            None => Ok(self.out),
        }
    }

    fn line(&self, vm: &Vm, step: &Step) -> String {
        let text = self.describe(step);
        let mut changes: Vec<String> = (0..11)
            .filter(|&r| vm.reg[r] != step.reg[r])
            .map(|r| format!("r{r}={:#x}", vm.reg[r]))
            .collect();
        let mut writes = vm.memory.writes.clone().unwrap_or_default();
        // A syscall may check an output before it writes it, which records it twice.
        writes.dedup();
        for (addr, len) in writes {
            changes.push(match vm.memory.slice(addr, len) {
                Ok(bytes) if len <= 8 => {
                    let mut value = [0; 8];
                    value[..len].copy_from_slice(bytes);
                    format!("[{addr:#x}]={:#x}", u64::from_le_bytes(value))
                }
                _ => format!("[{addr:#x}..+{len}]"),
            });
        }
        format!(
            "{:>7} {:>6}  {text:<33} {}",
            self.used,
            step.pc,
            changes.join(" ")
        )
        .trim_end()
        .to_string()
    }

    /// Like the disassembler, but with syscall and function names.
    fn describe(&self, step: &Step) -> String {
        match step.insn {
            _ if let Some(name) = step.insn.syscall_key().and_then(syscalls::name) => {
                format!("syscall {name}")
            }
            Insn::Call(CallTarget::Key(key))
                if let Some(&pc) = self.exe.functions.get(&(key as u32)) =>
            {
                format!("call {}", self.names.of(pc))
            }
            insn => format_insn(&insn, step.pc, |pc| Some(self.label(&insn, pc))),
        }
    }

    /// Calls go to functions; jumps go to `lbb_<pc>` inside one.
    fn label(&self, insn: &Insn, pc: usize) -> String {
        match insn {
            Insn::Call(_) => self.names.of(pc),
            _ => format!("lbb_{pc}"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::STACK_START;
    use crate::version::Version;

    #[test]
    fn lists_registers_and_memory_each_instruction_changed() {
        #[rustfmt::skip]
        let program = [
            0xb7, 0x01, 0, 0, 7, 0, 0, 0,       // mov64 r1, 7
            0x7b, 0x1a, 0xf8, 0xff, 0, 0, 0, 0, // stxdw [r10-8], r1
            0x15, 0x01, 0, 0, 0, 0, 0, 0,       // jeq r1, 0, +0
            0x95, 0, 0, 0, 0, 0, 0, 0,          // exit
        ];
        let exe = Executable::from_text(&program, Default::default(), Version::V0);
        let names = FunctionNames::read(&[], exe.entry_pc);
        let mut tracer = Tracer::new(Vec::new(), &exe, &names);
        Vm::new(&exe).run_observed(100, &mut tracer).unwrap();
        let slot = STACK_START + 4096 - 8;
        let expected = format!(
            "      1      0  mov64 r1, 7                       r1=0x7
      2      1  stxdw [r10-0x8], r1               [{slot:#x}]=0x7
      3      2  jeq r1, 0, lbb_3
      4      3  exit
"
        );
        assert_eq!(
            String::from_utf8(tracer.finish().unwrap()).unwrap(),
            expected
        );
    }
}
