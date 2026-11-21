//! Runs v0 and v3 bytecode.

use crate::context::Context;
use crate::error::VmError;
use crate::executable::Executable;
use crate::insn::{
    AluOp, ByteOrder, CallTarget, INSN_SIZE, Insn, JmpCond, Source, Width, decode_at, jump_target,
    split_slots,
};
use crate::memory::{INPUT_START, MAX_CALL_DEPTH, Memory, STACK_FRAME_SIZE, STACK_START};
use crate::syscalls;

/// Watches a run one instruction at a time. The tracer and profiler are observers.
pub trait Observer {
    /// Set to have `vm.memory.writes` list each instruction's writes.
    const RECORDS_WRITES: bool = false;

    /// Called after each instruction runs, including one that fails.
    fn after(&mut self, vm: &Vm, step: &Step);
}

/// A plain run, watched by no one.
impl Observer for () {
    fn after(&mut self, _: &Vm, _: &Step) {}
}

/// An instruction and the state just before it ran.
pub struct Step {
    pub pc: usize,
    pub insn: Insn,
    /// r0 to r10.
    pub reg: [u64; 11],
    /// Calls in progress: 0 in the entrypoint.
    pub depth: usize,
    /// Compute units the instruction used: 1, plus a syscall's own cost.
    pub cost: u64,
}

/// What a call saves so the matching `exit` can return to the caller.
struct Frame {
    /// r6 to r9, which the callee may overwrite.
    saved: [u64; 4],
    frame_pointer: u64,
    return_pc: usize,
}

pub struct Vm<'a> {
    /// r0 to r10.
    pub reg: [u64; 11],
    /// Slot index of the current instruction.
    pub pc: usize,
    /// Instructions started so far, including one that failed.
    pub insn_count: u64,
    pub memory: Memory<'a>,
    /// Program id, sysvars, logs, heap, return data and the compute meter.
    pub ctx: Context,
    exe: &'a Executable,
    frames: Vec<Frame>,
}

impl<'a> Vm<'a> {
    /// r1 points at the input and r10 at the top of the first stack frame.
    pub fn new(exe: &'a Executable) -> Self {
        let mut reg = [0; 11];
        reg[1] = INPUT_START;
        reg[10] = STACK_START + STACK_FRAME_SIZE;
        Self {
            reg,
            pc: exe.entry_pc,
            insn_count: 0,
            memory: Memory::new(&exe.ro, exe.ro_addr, exe.version),
            ctx: Context::default(),
            exe,
            frames: Vec::new(),
        }
    }

    /// Runs with `budget` compute units until `exit`, and returns r0.
    ///
    /// The program must pass the verifier (M3); a register above r10 panics.
    pub fn run(&mut self, budget: u64) -> Result<u64, VmError> {
        self.run_observed(budget, &mut ())
    }

    /// `run`, calling `observer` after every instruction.
    pub fn run_observed<O: Observer>(
        &mut self,
        budget: u64,
        observer: &mut O,
    ) -> Result<u64, VmError> {
        self.memory.writes = O::RECORDS_WRITES.then(Vec::new);
        self.ctx.remaining = budget;
        let slots = split_slots(&self.exe.text)?;
        loop {
            if self.ctx.remaining == 0 {
                return Err(VmError::ExceededMaxInstructions);
            }
            let (reg, remaining, depth) = (self.reg, self.ctx.remaining, self.frames.len());
            self.ctx.remaining -= 1;
            self.insn_count += 1;
            if self.pc >= slots.len() {
                return Err(VmError::ExecutionOverrun);
            }
            let (pc, insn) = (self.pc, decode_at(slots, self.pc, self.exe.version)?);
            if let Some(writes) = &mut self.memory.writes {
                writes.clear();
            }
            let result = self.step(insn);
            let step = Step {
                pc,
                insn,
                reg,
                depth,
                cost: remaining - self.ctx.remaining,
            };
            observer.after(self, &step);
            if let Some(r0) = result? {
                return Ok(r0);
            }
        }
    }

    /// Calls in progress: 0 in the entrypoint.
    pub fn depth(&self) -> usize {
        self.frames.len()
    }

    /// Executes one instruction. Returns `Some(r0)` on `exit`.
    fn step(&mut self, insn: Insn) -> Result<Option<u64>, VmError> {
        let mut next = self.pc + insn.slots();
        match insn {
            Insn::Lddw { dst, imm } => self.reg[dst as usize] = imm,
            Insn::Alu {
                width,
                op,
                dst,
                source,
            } => {
                let (a, b) = (self.reg[dst as usize], self.operand(source));
                let result = match width {
                    Width::W32 => alu32(op, a as u32, b as u32),
                    Width::W64 => alu64(op, a, b),
                };
                self.reg[dst as usize] = result.ok_or(VmError::DivideByZero)?;
            }
            Insn::Neg { width, dst } => {
                let r = &mut self.reg[dst as usize];
                *r = match width {
                    Width::W32 => u64::from((*r as u32).wrapping_neg()),
                    Width::W64 => r.wrapping_neg(),
                };
            }
            Insn::Endian { order, dst, bits } => {
                let r = &mut self.reg[dst as usize];
                *r = byte_swap(order, bits, *r).ok_or(VmError::InvalidInstruction)?;
            }
            Insn::Ja { off } => next = jump_target(self.pc, off),
            Insn::Jump {
                width,
                cond,
                dst,
                source,
                off,
            } => {
                let (a, b) = (self.reg[dst as usize], self.operand(source));
                let (a, b) = match width {
                    Width::W64 => (a, b),
                    // Sign-extending both low halves keeps the answer of every 32-bit
                    // comparison, signed or unsigned, so one 64-bit `condition` serves both.
                    Width::W32 => (a as i32 as u64, b as i32 as u64),
                };
                if condition(cond, a, b) {
                    next = jump_target(self.pc, off);
                }
            }
            Insn::Load {
                size,
                dst,
                src,
                off,
            } => {
                let addr = self.reg[src as usize].wrapping_add_signed(off.into());
                self.reg[dst as usize] = self.memory.load(addr, size)?;
            }
            Insn::Store {
                size,
                dst,
                off,
                value,
            } => {
                let addr = self.reg[dst as usize].wrapping_add_signed(off.into());
                self.memory.store(addr, size, self.operand(value))?;
            }
            Insn::Call(_) if let Some(syscall) = insn.syscall_key().and_then(syscalls::lookup) => {
                self.reg[0] = syscall(self, std::array::from_fn(|i| self.reg[i + 1]))?;
            }
            Insn::Call(target) => {
                let target = self
                    .function_pc(target)
                    .ok_or(VmError::UnsupportedInstruction)?;
                self.push_frame()?;
                next = self.call_target(target)?;
            }
            Insn::Callx { reg } => {
                let addr = self.reg[reg as usize];
                self.push_frame()?;
                next =
                    self.call_target(addr.wrapping_sub(self.exe.text_addr) / INSN_SIZE as u64)?;
            }
            Insn::Exit => match self.frames.pop() {
                None => return Ok(Some(self.reg[0])),
                Some(frame) => {
                    self.reg[6..10].copy_from_slice(&frame.saved);
                    self.reg[10] = frame.frame_pointer;
                    next = frame.return_pc;
                }
            },
            Insn::Unknown { .. } => return Err(VmError::UnsupportedInstruction),
        }
        self.pc = next;
        Ok(None)
    }

    /// Where a `call` that is not a syscall goes, if anywhere. A v3 relative call must land
    /// inside the code; a v0 key's pc is checked after the frame is pushed, as in the reference.
    fn function_pc(&self, target: CallTarget) -> Option<u64> {
        match target {
            CallTarget::Key(key) => self.exe.functions.get(&(key as u32)).map(|&pc| pc as u64),
            CallTarget::Relative(off) => {
                let target = self.pc as i64 + 1 + i64::from(off);
                (0..self.text_slots() as i64)
                    .contains(&target)
                    .then_some(target as u64)
            }
            CallTarget::Syscall(_) | CallTarget::Invalid(_) => None,
        }
    }

    /// Saves the caller's state and moves r10 to the next stack frame.
    fn push_frame(&mut self) -> Result<(), VmError> {
        self.frames.push(Frame {
            saved: std::array::from_fn(|i| self.reg[6 + i]),
            frame_pointer: self.reg[10],
            return_pc: self.pc + 1,
        });
        if self.frames.len() == MAX_CALL_DEPTH as usize {
            return Err(VmError::CallDepthExceeded);
        }
        // Skip this frame, and in v0 the unmapped gap after it.
        let frames = if self.exe.version.stack_frame_gaps() {
            2
        } else {
            1
        };
        self.reg[10] = self.reg[10].wrapping_add(frames * STACK_FRAME_SIZE);
        Ok(())
    }

    fn call_target(&self, pc: u64) -> Result<usize, VmError> {
        usize::try_from(pc)
            .ok()
            .filter(|&pc| pc < self.text_slots())
            .ok_or(VmError::CallOutsideTextSegment)
    }

    fn text_slots(&self) -> usize {
        self.exe.text.len() / INSN_SIZE
    }

    /// The second operand as 64 bits. Immediates are sign-extended.
    fn operand(&self, source: Source) -> u64 {
        match source {
            Source::Imm(imm) => i64::from(imm) as u64,
            Source::Reg(src) => self.reg[src as usize],
        }
    }
}

/// `None` means division by zero.
fn alu64(op: AluOp, a: u64, b: u64) -> Option<u64> {
    Some(match op {
        AluOp::Add => a.wrapping_add(b),
        AluOp::Sub => a.wrapping_sub(b),
        AluOp::Mul => a.wrapping_mul(b),
        AluOp::Div => a.checked_div(b)?,
        AluOp::Mod => a.checked_rem(b)?,
        AluOp::Or => a | b,
        AluOp::And => a & b,
        AluOp::Xor => a ^ b,
        AluOp::Lsh => a.wrapping_shl(b as u32),
        AluOp::Rsh => a.wrapping_shr(b as u32),
        AluOp::Arsh => (a as i64).wrapping_shr(b as u32) as u64,
        AluOp::Mov => b,
    })
}

/// `None` means division by zero. In v0, add, sub and mul sign-extend; the rest zero-extend.
fn alu32(op: AluOp, a: u32, b: u32) -> Option<u64> {
    let sign_extend = |x: u32| x as i32 as i64 as u64;
    Some(match op {
        AluOp::Add => sign_extend(a.wrapping_add(b)),
        AluOp::Sub => sign_extend(a.wrapping_sub(b)),
        AluOp::Mul => sign_extend(a.wrapping_mul(b)),
        AluOp::Div => a.checked_div(b)?.into(),
        AluOp::Mod => a.checked_rem(b)?.into(),
        AluOp::Or => (a | b).into(),
        AluOp::And => (a & b).into(),
        AluOp::Xor => (a ^ b).into(),
        AluOp::Lsh => a.wrapping_shl(b).into(),
        AluOp::Rsh => a.wrapping_shr(b).into(),
        AluOp::Arsh => u64::from((a as i32).wrapping_shr(b) as u32),
        AluOp::Mov => b.into(),
    })
}

/// `le` keeps the low `bits`, since the VM is little-endian. `be` also reverses their bytes.
fn byte_swap(order: ByteOrder, bits: i32, v: u64) -> Option<u64> {
    Some(match (order, bits) {
        (ByteOrder::Le, 16) => u64::from(v as u16),
        (ByteOrder::Le, 32) => u64::from(v as u32),
        (ByteOrder::Le, 64) => v,
        (ByteOrder::Be, 16) => u64::from((v as u16).swap_bytes()),
        (ByteOrder::Be, 32) => u64::from((v as u32).swap_bytes()),
        (ByteOrder::Be, 64) => v.swap_bytes(),
        _ => return None,
    })
}

fn condition(cond: JmpCond, a: u64, b: u64) -> bool {
    let (sa, sb) = (a as i64, b as i64);
    match cond {
        JmpCond::Eq => a == b,
        JmpCond::Ne => a != b,
        JmpCond::Gt => a > b,
        JmpCond::Ge => a >= b,
        JmpCond::Lt => a < b,
        JmpCond::Le => a <= b,
        JmpCond::Set => a & b != 0,
        JmpCond::Sgt => sa > sb,
        JmpCond::Sge => sa >= sb,
        JmpCond::Slt => sa < sb,
        JmpCond::Sle => sa <= sb,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::version::Version;

    fn slot(opc: u8, regs: u8, off: i16, imm: i32) -> [u8; 8] {
        let [o0, o1] = off.to_le_bytes();
        let [i0, i1, i2, i3] = imm.to_le_bytes();
        [opc, regs, o0, o1, i0, i1, i2, i3]
    }

    fn run(slots: &[[u8; 8]]) -> Result<u64, VmError> {
        let exe = Executable::from_text(slots.as_flattened(), Default::default(), Version::V0);
        Vm::new(&exe).run(100)
    }

    const EXIT: [u8; 8] = [0x95, 0, 0, 0, 0, 0, 0, 0];

    #[test]
    fn add32_sign_extends_in_v0() {
        // mov32 r0, 0x7fffffff; add32 r0, 1
        let program = [slot(0xb4, 0, 0, 0x7fff_ffff), slot(0x04, 0, 0, 1), EXIT];
        assert_eq!(run(&program), Ok(0xffff_ffff_8000_0000));
    }

    #[test]
    fn or32_zero_extends() {
        // mov64 r0, -1; or32 r0, 0
        let program = [slot(0xb7, 0, 0, -1), slot(0x44, 0, 0, 0), EXIT];
        assert_eq!(run(&program), Ok(0xffff_ffff));
    }

    #[test]
    fn div32_checks_only_the_low_bits() {
        // lddw r1, 1 << 32; mov64 r0, 7; div32 r0, r1
        let program = [
            slot(0x18, 1, 0, 0),
            slot(0, 0, 0, 1),
            slot(0xb7, 0, 0, 7),
            slot(0x3c, 0x10, 0, 0),
            EXIT,
        ];
        assert_eq!(run(&program), Err(VmError::DivideByZero));
    }

    #[test]
    fn stops_at_the_limit_and_at_the_end() {
        assert_eq!(
            run(&[slot(0x05, 0, -1, 0)]),
            Err(VmError::ExceededMaxInstructions)
        );
        assert_eq!(run(&[slot(0xb7, 0, 0, 1)]), Err(VmError::ExecutionOverrun));
    }

    #[test]
    fn exit_returns_r0_and_counts_instructions() {
        let program = [slot(0xb7, 0, 0, 42), EXIT];
        let exe = Executable::from_text(program.as_flattened(), Default::default(), Version::V0);
        let mut vm = Vm::new(&exe);
        assert_eq!(vm.run(100), Ok(42));
        assert_eq!((vm.pc, vm.insn_count), (1, 2));
    }
}
