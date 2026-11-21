//! Runs v0 bytecode. Calls come in M4.

use crate::error::VmError;
use crate::insn::{
    AluOp, ByteOrder, Insn, JmpCond, Source, Width, decode_at, jump_target, split_slots,
};
use crate::memory::{INPUT_START, Memory, STACK_FRAME_SIZE, STACK_START};

pub struct Vm<'a> {
    /// r0 to r10.
    pub reg: [u64; 11],
    /// Slot index of the current instruction.
    pub pc: usize,
    /// Instructions started so far, including one that failed.
    pub insn_count: u64,
    pub memory: Memory<'a>,
}

impl<'a> Vm<'a> {
    /// r1 points at the input and r10 at the top of the first stack frame.
    pub fn new(program: &'a [u8]) -> Self {
        let mut reg = [0; 11];
        reg[1] = INPUT_START;
        reg[10] = STACK_START + STACK_FRAME_SIZE;
        Self {
            reg,
            pc: 0,
            insn_count: 0,
            memory: Memory::new(program),
        }
    }

    /// Runs until `exit` and returns r0.
    ///
    /// The program must pass the verifier (M3); a register above r10 panics.
    pub fn run(&mut self, max_insns: u64) -> Result<u64, VmError> {
        let slots = split_slots(self.memory.program)?;
        loop {
            if self.insn_count >= max_insns {
                return Err(VmError::ExceededMaxInstructions);
            }
            self.insn_count += 1;
            if self.pc >= slots.len() {
                return Err(VmError::ExecutionOverrun);
            }
            if let Some(r0) = self.step(decode_at(slots, self.pc)?)? {
                return Ok(r0);
            }
        }
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
                cond,
                dst,
                source,
                off,
            } => {
                if condition(cond, self.reg[dst as usize], self.operand(source)) {
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
            Insn::Exit => return Ok(Some(self.reg[0])),
            Insn::Call { .. } | Insn::Callx { .. } | Insn::Unknown { .. } => {
                return Err(VmError::UnsupportedInstruction);
            }
        }
        self.pc = next;
        Ok(None)
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

    fn slot(opc: u8, regs: u8, off: i16, imm: i32) -> [u8; 8] {
        let [o0, o1] = off.to_le_bytes();
        let [i0, i1, i2, i3] = imm.to_le_bytes();
        [opc, regs, o0, o1, i0, i1, i2, i3]
    }

    fn run(slots: &[[u8; 8]]) -> Result<u64, VmError> {
        Vm::new(slots.as_flattened()).run(100)
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
        let mut vm = Vm::new(program.as_flattened());
        assert_eq!(vm.run(100), Ok(42));
        assert_eq!((vm.pc, vm.insn_count), (1, 2));
    }
}
