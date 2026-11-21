//! Static checks before a program runs. Reference: `RequisiteVerifier` in `verifier.rs`.

use crate::insn::{
    AluOp, DecodeError, Insn, RawInsn, Source, Width, decode_at, jump_target, split_slots,
};
use thiserror::Error;

/// Names and fields match the reference `VerifierError`. Only the v0 ones are here.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum VerifierError {
    #[error("program length must be a multiple of 8 bytes")]
    ProgramLengthNotMultiple,
    #[error("program is empty")]
    NoProgram,
    #[error("division by 0 (insn #{0})")]
    DivisionByZero(usize),
    #[error("unsupported size for le/be (insn #{0})")]
    UnsupportedLEBEArgument(usize),
    #[error("lddw cannot be the last instruction")]
    LDDWCannotBeLast,
    #[error("lddw second slot must have opcode 0 (insn #{0})")]
    IncompleteLDDW(usize),
    #[error("jump out of code to #{0} (insn #{1})")]
    JumpOutOfCode(usize, usize),
    #[error("jump into the middle of lddw at #{0} (insn #{1})")]
    JumpToMiddleOfLDDW(usize, usize),
    #[error("invalid source register (insn #{0})")]
    InvalidSourceRegister(usize),
    #[error("cannot write to r10 (insn #{0})")]
    CannotWriteR10(usize),
    #[error("invalid destination register (insn #{0})")]
    InvalidDestinationRegister(usize),
    #[error("unknown opcode {0:#x} (insn #{1})")]
    UnknownOpCode(u8, usize),
    #[error("shift of a {1}-bit value by {0} (insn #{2})")]
    ShiftWithOverflow(u64, u64, usize),
    #[error("callx register must be r0 to r9 (insn #{0})")]
    InvalidRegister(usize),
}

impl From<DecodeError> for VerifierError {
    fn from(e: DecodeError) -> Self {
        match e {
            DecodeError::PartialSlot { .. } => VerifierError::ProgramLengthNotMultiple,
            DecodeError::IncompleteLddw { .. } => VerifierError::LDDWCannotBeLast,
        }
    }
}

/// Checks a v0 program. Does not check that it ever exits.
pub fn verify(program: &[u8]) -> Result<(), VerifierError> {
    let slots = split_slots(program)?;
    if slots.is_empty() {
        return Err(VerifierError::NoProgram);
    }
    let mut pc = 0;
    while pc < slots.len() {
        let insn = decode_at(slots, pc)?;
        check_insn(slots, pc, insn)?;
        let last_slot = pc + insn.slots() - 1;
        // The reference reports register errors at the instruction's last slot.
        check_registers(RawInsn::from_bytes(&slots[pc]), insn, last_slot)?;
        pc += insn.slots();
    }
    Ok(())
}

fn check_insn(slots: &[[u8; 8]], pc: usize, insn: Insn) -> Result<(), VerifierError> {
    use VerifierError::*;
    match insn {
        Insn::Unknown { opc } => Err(UnknownOpCode(opc, pc)),
        Insn::Lddw { .. } if slots[pc + 1][0] != 0 => Err(IncompleteLDDW(pc)),
        Insn::Alu {
            op: AluOp::Div | AluOp::Mod,
            source: Source::Imm(0),
            ..
        } => Err(DivisionByZero(pc)),
        Insn::Alu {
            width,
            op: AluOp::Lsh | AluOp::Rsh | AluOp::Arsh,
            source: Source::Imm(imm),
            ..
        } => {
            let bits = if width == Width::W32 { 32 } else { 64 };
            let shift = i64::from(imm) as u64;
            if imm < 0 || shift >= bits {
                Err(ShiftWithOverflow(shift, bits, pc))
            } else {
                Ok(())
            }
        }
        Insn::Endian {
            bits: 16 | 32 | 64, ..
        } => Ok(()),
        Insn::Endian { .. } => Err(UnsupportedLEBEArgument(pc)),
        Insn::Ja { off } | Insn::Jump { off, .. } => {
            let target = jump_target(pc, off);
            match slots.get(target) {
                None => Err(JumpOutOfCode(target, pc)),
                // Opcode 0 is only valid as the second slot of an lddw.
                Some(slot) if slot[0] == 0 => Err(JumpToMiddleOfLDDW(target, pc)),
                Some(_) => Ok(()),
            }
        }
        // In v0 the register number is in imm. r10 is not allowed.
        Insn::Callx { imm } if !(0..10).contains(&imm) => Err(InvalidRegister(pc)),
        _ => Ok(()),
    }
}

/// Checks the raw register fields, even for instructions that ignore them.
fn check_registers(raw: RawInsn, insn: Insn, pc: usize) -> Result<(), VerifierError> {
    let is_store = matches!(insn, Insn::Store { .. });
    match (raw.src, raw.dst) {
        (11.., _) => Err(VerifierError::InvalidSourceRegister(pc)),
        (_, 0..=9) => Ok(()),
        (_, 10) if is_store => Ok(()),
        (_, 10) => Err(VerifierError::CannotWriteR10(pc)),
        _ => Err(VerifierError::InvalidDestinationRegister(pc)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use VerifierError::*;

    const EXIT: [u8; 8] = [0x95, 0, 0, 0, 0, 0, 0, 0];

    fn verify_slots(slots: &[[u8; 8]]) -> Result<(), VerifierError> {
        verify(slots.as_flattened())
    }

    #[test]
    fn accepts_a_simple_program() {
        // mov64 r0, 1; stxdw [r10-8], r0; exit
        let program = [
            [0xb7, 0, 0, 0, 1, 0, 0, 0],
            [0x7b, 0x0a, 0xf8, 0xff, 0, 0, 0, 0],
            EXIT,
        ];
        assert_eq!(verify_slots(&program), Ok(()));
    }

    #[test]
    fn rejects_bad_shapes() {
        assert_eq!(verify(&[]), Err(NoProgram));
        assert_eq!(verify(&[0x95; 9]), Err(ProgramLengthNotMultiple));
        assert_eq!(
            verify_slots(&[[0x18, 0, 0, 0, 0, 0, 0, 0]]),
            Err(LDDWCannotBeLast)
        );
        assert_eq!(
            verify_slots(&[[0x18, 0, 0, 0, 0, 0, 0, 0], EXIT]),
            Err(IncompleteLDDW(0))
        );
    }

    #[test]
    fn lddw_register_errors_point_at_its_second_slot() {
        let program = [[0x18, 0x0b, 0, 0, 0, 0, 0, 0], [0; 8], EXIT];
        assert_eq!(verify_slots(&program), Err(InvalidDestinationRegister(1)));
    }

    #[test]
    fn r10_is_read_only_except_as_a_store_base() {
        // exit with dst = r10 in its unused field
        assert_eq!(
            verify_slots(&[[0x95, 0x0a, 0, 0, 0, 0, 0, 0]]),
            Err(CannotWriteR10(0))
        );
        // stb [r10-1], 0
        assert_eq!(
            verify_slots(&[[0x72, 0x0a, 0xff, 0xff, 0, 0, 0, 0], EXIT]),
            Ok(())
        );
    }
}
