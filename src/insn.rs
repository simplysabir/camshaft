//! Turns sBPF bytecode into `Insn` values. Reference: `ebpf.rs` in anza-xyz/sbpf.

use crate::version::Version;
use thiserror::Error;

/// Bytes per instruction slot. `lddw` uses two slots.
pub const INSN_SIZE: usize = 8;

const LDDW: u8 = 0x18;

/// A decoded instruction. Registers are raw 0..=15; the verifier rejects r11+.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Insn {
    /// Loads a 64-bit constant. Uses two slots.
    Lddw {
        dst: u8,
        imm: u64,
    },
    /// `dst = *(src + off)`
    Load {
        size: Size,
        dst: u8,
        src: u8,
        off: i16,
    },
    /// `*(dst + off) = value`
    Store {
        size: Size,
        dst: u8,
        off: i16,
        value: Source,
    },
    Alu {
        width: Width,
        op: AluOp,
        dst: u8,
        source: Source,
    },
    Neg {
        width: Width,
        dst: u8,
    },
    /// `bits` should be 16, 32 or 64. The verifier checks it.
    Endian {
        order: ByteOrder,
        dst: u8,
        bits: i32,
    },
    Ja {
        off: i16,
    },
    /// `W32` (v3 only) compares the low 32 bits of each side.
    Jump {
        width: Width,
        cond: JmpCond,
        dst: u8,
        source: Source,
        off: i16,
    },
    Call(CallTarget),
    /// Jumps to the address in register `reg`. v0 keeps the number in imm, v3 in dst.
    Callx {
        reg: i32,
    },
    Exit,
    /// Not an opcode in this version.
    Unknown {
        opc: u8,
    },
}

/// What a `call imm` calls. The version and the src field decide.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CallTarget {
    /// v0: a key naming a syscall, or else a function in the function table.
    Key(i32),
    /// v3 with src 0: a key naming a syscall.
    Syscall(i32),
    /// v3 with src 1: the instruction at `pc + 1 + offset`.
    Relative(i32),
    /// v3 with any other src: fails when it runs. Holds the imm.
    Invalid(i32),
}

/// The second operand: an immediate or a register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Imm(i32),
    Reg(u8),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Size {
    B,
    H,
    W,
    DW,
}

impl Size {
    pub fn bytes(self) -> usize {
        match self {
            Size::B => 1,
            Size::H => 2,
            Size::W => 4,
            Size::DW => 8,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    W32,
    W64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOp {
    Add,
    Sub,
    Mul,
    Div,
    Or,
    And,
    Lsh,
    Rsh,
    Mod,
    Xor,
    Mov,
    Arsh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ByteOrder {
    Le,
    Be,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JmpCond {
    Eq,
    Gt,
    Ge,
    Set,
    Ne,
    Sgt,
    Sge,
    Lt,
    Le,
    Slt,
    Sle,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DecodeError {
    #[error("program length {len} is not a multiple of {INSN_SIZE} bytes")]
    PartialSlot { len: usize },
    #[error("lddw at pc {pc} is missing its second slot")]
    IncompleteLddw { pc: usize },
}

/// One slot split into its fields.
pub(crate) struct RawInsn {
    pub(crate) opc: u8,
    pub(crate) dst: u8,
    pub(crate) src: u8,
    pub(crate) off: i16,
    pub(crate) imm: i32,
}

impl RawInsn {
    pub(crate) fn from_bytes(b: &[u8; INSN_SIZE]) -> Self {
        Self {
            opc: b[0],
            dst: b[1] & 0x0f,
            src: b[1] >> 4,
            off: i16::from_le_bytes([b[2], b[3]]),
            imm: i32::from_le_bytes([b[4], b[5], b[6], b[7]]),
        }
    }
}

impl Insn {
    /// How many slots this instruction uses.
    pub fn slots(&self) -> usize {
        if matches!(self, Insn::Lddw { .. }) {
            2
        } else {
            1
        }
    }
}

/// Decodes a program into `(pc, insn)` pairs. pc counts slots, not instructions.
pub fn decode_program(program: &[u8], version: Version) -> Result<Vec<(usize, Insn)>, DecodeError> {
    let slots = split_slots(program)?;
    let mut insns = Vec::with_capacity(slots.len());
    let mut pc = 0;
    while pc < slots.len() {
        let insn = decode_at(slots, pc, version)?;
        insns.push((pc, insn));
        pc += insn.slots();
    }
    Ok(insns)
}

/// Where a jump at `pc` lands: `pc + 1 + off`. A negative result wraps to a huge pc,
/// which every caller treats as out of range.
pub fn jump_target(pc: usize, off: i16) -> usize {
    (pc + 1).wrapping_add_signed(isize::from(off))
}

/// Splits a program into 8-byte slots.
pub fn split_slots(program: &[u8]) -> Result<&[[u8; INSN_SIZE]], DecodeError> {
    match program.as_chunks::<INSN_SIZE>() {
        (slots, []) => Ok(slots),
        _ => Err(DecodeError::PartialSlot { len: program.len() }),
    }
}

/// Decodes the instruction at slot `pc`. Panics if `pc` is out of bounds.
pub fn decode_at(
    slots: &[[u8; INSN_SIZE]],
    pc: usize,
    version: Version,
) -> Result<Insn, DecodeError> {
    let raw = RawInsn::from_bytes(&slots[pc]);
    if raw.opc != LDDW {
        return Ok(decode_single(raw, version));
    }
    let high = slots
        .get(pc + 1)
        .ok_or(DecodeError::IncompleteLddw { pc })?;
    // Only the second slot's imm is used.
    let high = RawInsn::from_bytes(high).imm;
    let imm = u64::from(raw.imm as u32) | u64::from(high as u32) << 32;
    Ok(Insn::Lddw { dst: raw.dst, imm })
}

/// Decodes a one-slot instruction.
/// ALU/jump opcode bits: `op(4) source(1) class(3)`. Memory: `mode(3) size(2) class(3)`.
fn decode_single(raw: RawInsn, version: Version) -> Insn {
    let RawInsn {
        opc,
        dst,
        src,
        off,
        imm,
    } = raw;
    let unknown = Insn::Unknown { opc };
    let is_imm = opc & 0x08 == 0;
    let source = if is_imm {
        Source::Imm(imm)
    } else {
        Source::Reg(src)
    };
    match opc & 0x07 {
        class @ 0x01..=0x03 => {
            // v0 loads and stores only use mode 0x60.
            if opc & 0xe0 != 0x60 {
                return unknown;
            }
            let size = match opc & 0x18 {
                0x00 => Size::W,
                0x08 => Size::H,
                0x10 => Size::B,
                _ => Size::DW,
            };
            match class {
                0x01 => Insn::Load {
                    size,
                    dst,
                    src,
                    off,
                },
                0x02 => Insn::Store {
                    size,
                    dst,
                    off,
                    value: Source::Imm(imm),
                },
                _ => Insn::Store {
                    size,
                    dst,
                    off,
                    value: Source::Reg(src),
                },
            }
        }
        class @ (0x04 | 0x07) => {
            let width = if class == 0x04 {
                Width::W32
            } else {
                Width::W64
            };
            let op = match opc & 0xf0 {
                0x00 => AluOp::Add,
                0x10 => AluOp::Sub,
                0x20 => AluOp::Mul,
                0x30 => AluOp::Div,
                0x40 => AluOp::Or,
                0x50 => AluOp::And,
                0x60 => AluOp::Lsh,
                0x70 => AluOp::Rsh,
                0x80 if is_imm => return Insn::Neg { width, dst },
                0x90 => AluOp::Mod,
                0xa0 => AluOp::Xor,
                0xb0 => AluOp::Mov,
                0xc0 => AluOp::Arsh,
                // Byte swaps are 32-bit class only. The source bit picks le or be.
                0xd0 if width == Width::W32 => {
                    let order = if is_imm { ByteOrder::Le } else { ByteOrder::Be };
                    return Insn::Endian {
                        order,
                        dst,
                        bits: imm,
                    };
                }
                _ => return unknown,
            };
            Insn::Alu {
                width,
                op,
                dst,
                source,
            }
        }
        // 0x05 is the 64-bit jump class, which also holds ja, call, callx and exit.
        // 0x06 is v3's 32-bit jump class.
        class @ (0x05 | 0x06) => {
            let width = match class {
                0x05 => Width::W64,
                _ if version == Version::V3 => Width::W32,
                _ => return unknown,
            };
            if width == Width::W64 {
                match (opc & 0xf0, is_imm) {
                    (0x00, true) => return Insn::Ja { off },
                    (0x80, true) => {
                        return Insn::Call(match (version, src) {
                            (Version::V0, _) => CallTarget::Key(imm),
                            (Version::V3, 0) => CallTarget::Syscall(imm),
                            (Version::V3, 1) => CallTarget::Relative(imm),
                            (Version::V3, _) => CallTarget::Invalid(imm),
                        });
                    }
                    (0x80, false) => {
                        let reg = match version {
                            Version::V0 => imm,
                            Version::V3 => dst.into(),
                        };
                        return Insn::Callx { reg };
                    }
                    (0x90, true) => return Insn::Exit,
                    _ => {}
                }
            }
            let cond = match opc & 0xf0 {
                0x10 => JmpCond::Eq,
                0x20 => JmpCond::Gt,
                0x30 => JmpCond::Ge,
                0x40 => JmpCond::Set,
                0x50 => JmpCond::Ne,
                0x60 => JmpCond::Sgt,
                0x70 => JmpCond::Sge,
                0xa0 => JmpCond::Lt,
                0xb0 => JmpCond::Le,
                0xc0 => JmpCond::Slt,
                0xd0 => JmpCond::Sle,
                _ => return unknown,
            };
            Insn::Jump {
                width,
                cond,
                dst,
                source,
                off,
            }
        }
        // 0x00: lddw, handled in `decode_at`.
        _ => unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Decodes `opc` with zeroed fields. The extra slot completes an `lddw`.
    fn decode_opcode(opc: u8, version: Version) -> Insn {
        decode_at(&[[opc, 0, 0, 0, 0, 0, 0, 0], [0; 8]], 0, version).unwrap()
    }

    fn decode_one(bytes: [u8; 8]) -> Insn {
        match decode_program(&bytes, Version::V0).unwrap()[..] {
            [(0, insn)] => insn,
            ref other => panic!("expected one instruction, got {other:?}"),
        }
    }

    #[test]
    fn splits_fields_little_endian() {
        // jsgt r2, r1, -2. imm is ignored in the register form.
        let insn = decode_one([0x6d, 0x12, 0xfe, 0xff, 0x01, 0x02, 0x03, 0x04]);
        assert_eq!(
            insn,
            Insn::Jump {
                width: Width::W64,
                cond: JmpCond::Sgt,
                dst: 2,
                source: Source::Reg(1),
                off: -2
            }
        );
        // mov64 r15, -2
        let insn = decode_one([0xb7, 0x0f, 0, 0, 0xfe, 0xff, 0xff, 0xff]);
        assert_eq!(
            insn,
            Insn::Alu {
                width: Width::W64,
                op: AluOp::Mov,
                dst: 15,
                source: Source::Imm(-2)
            }
        );
    }

    #[test]
    fn lddw_merges_two_slots() {
        let program = [
            0x18, 0x01, 0, 0, 0xef, 0xcd, 0xab, 0x89, // low half, bit 31 set
            0xff, 0xff, 0xff, 0xff, 0x67, 0x45, 0x23, 0x01, // high half, other fields ignored
            0x95, 0, 0, 0, 0, 0, 0, 0, // exit
        ];
        assert_eq!(
            decode_program(&program, Version::V0).unwrap(),
            [
                (
                    0,
                    Insn::Lddw {
                        dst: 1,
                        imm: 0x0123_4567_89ab_cdef
                    }
                ),
                (2, Insn::Exit)
            ]
        );
    }

    #[test]
    fn rejects_malformed_programs() {
        assert_eq!(
            decode_program(&[0x95; 9], Version::V0),
            Err(DecodeError::PartialSlot { len: 9 })
        );
        let lone_lddw = [0x95, 0, 0, 0, 0, 0, 0, 0, 0x18, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            decode_program(&lone_lddw, Version::V0),
            Err(DecodeError::IncompleteLddw { pc: 1 })
        );
        assert_eq!(decode_program(&[], Version::V0), Ok(vec![]));
    }

    #[test]
    fn near_miss_opcodes_are_unknown() {
        // Opcodes that look valid but are not v0.
        for opc in [
            0x0d, 0x8c, 0x8f, 0x9d, 0xd7, 0xdf, 0xf7, 0x20, 0x06, 0x0e, 0xe4, 0xe5,
        ] {
            assert_eq!(
                decode_opcode(opc, Version::V0),
                Insn::Unknown { opc },
                "{opc:#x}"
            );
        }
    }

    #[test]
    fn v0_has_91_opcodes_and_v3_adds_22_jumps() {
        let valid = |version| {
            (0..=255u8)
                .filter(|&opc| !matches!(decode_opcode(opc, version), Insn::Unknown { .. }))
                .count()
        };
        assert_eq!((valid(Version::V0), valid(Version::V3)), (91, 113));
    }

    #[test]
    fn callx_register_moves_to_dst_in_v3() {
        // callx with dst 3 and imm 5
        let slots = [[0x8d, 0x03, 0, 0, 5, 0, 0, 0]];
        assert_eq!(
            decode_at(&slots, 0, Version::V0),
            Ok(Insn::Callx { reg: 5 })
        );
        assert_eq!(
            decode_at(&slots, 0, Version::V3),
            Ok(Insn::Callx { reg: 3 })
        );
    }
}
