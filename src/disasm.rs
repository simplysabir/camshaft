//! Prints instructions as text, matching the reference `disassembler.rs`.

use crate::insn::{
    self, AluOp, ByteOrder, CallTarget, DecodeError, Insn, JmpCond, Size, Source, Width,
    decode_program,
};
use crate::version::Version;
use std::collections::BTreeMap;

/// Disassembles raw bytecode. Labels can differ from the reference; see docs/00-instructions.md.
pub fn disassemble(program: &[u8], version: Version) -> Result<String, DecodeError> {
    let insns = decode_program(program, version)?;
    // pc 0 plus every jump or call target that lands on an instruction.
    let labels: BTreeMap<usize, String> = insns
        .iter()
        .filter_map(|(pc, insn)| target_of(insn, *pc))
        .chain([0])
        .filter(|target| insns.binary_search_by_key(target, |&(pc, _)| pc).is_ok())
        .map(|pc| (pc, block_label(pc)))
        .collect();
    let label_of = |pc: usize| labels.get(&pc).cloned();

    let mut out = String::new();
    for (pc, insn) in &insns {
        if let Some(label) = labels.get(pc) {
            out += &format!("{label}:\n");
        }
        out += &format!("    {}\n", format_insn(insn, *pc, label_of));
    }
    Ok(out)
}

/// Formats one instruction. `label_of` names jump and call targets; a jump to an unnamed
/// target prints `[invalid]`.
pub fn format_insn(insn: &Insn, pc: usize, label_of: impl Fn(usize) -> Option<String>) -> String {
    let label = || target_of(insn, pc).and_then(&label_of);
    let target = || label().unwrap_or_else(|| "[invalid]".into());
    match *insn {
        Insn::Lddw { dst, imm } => format!("lddw r{dst}, {imm:#x}"),
        Insn::Load {
            size,
            dst,
            src,
            off,
        } => {
            format!(
                "ldx{} r{dst}, [r{src}{}]",
                size_suffix(size),
                signed_hex(off)
            )
        }
        Insn::Store {
            size,
            dst,
            off,
            value,
        } => {
            let x = if let Source::Reg(_) = value { "x" } else { "" };
            format!(
                "st{x}{} [r{dst}{}], {}",
                size_suffix(size),
                signed_hex(off),
                operand(value)
            )
        }
        Insn::Alu {
            width,
            op,
            dst,
            source,
        } => {
            format!(
                "{}{} r{dst}, {}",
                alu_name(op),
                width_suffix(width),
                operand(source)
            )
        }
        Insn::Neg { width, dst } => format!("neg{} r{dst}", width_suffix(width)),
        Insn::Endian {
            order: ByteOrder::Le,
            dst,
            bits,
        } => format!("le{bits} r{dst}"),
        Insn::Endian {
            order: ByteOrder::Be,
            dst,
            bits,
        } => format!("be{bits} r{dst}"),
        Insn::Ja { .. } => format!("ja {}", target()),
        Insn::Jump {
            width,
            cond,
            dst,
            source,
            ..
        } => {
            // 64-bit jumps have no suffix.
            let suffix = if width == Width::W32 { "32" } else { "" };
            format!(
                "{}{suffix} r{dst}, {}, {}",
                jmp_name(cond),
                operand(source),
                target()
            )
        }
        Insn::Call(CallTarget::Key(imm) | CallTarget::Syscall(imm)) => format!("syscall {imm}"),
        Insn::Call(CallTarget::Relative(imm) | CallTarget::Invalid(imm)) => {
            format!("call {}", label().unwrap_or_else(|| imm.to_string()))
        }
        // The reference prints the register number cut to one byte.
        Insn::Callx { reg } => format!("callx r{}", reg as u8),
        Insn::Exit => "exit".into(),
        Insn::Unknown { opc } => format!("unknown opcode={opc:#x}"),
    }
}

/// Where a jump, or a v3 relative call, goes.
fn target_of(insn: &Insn, pc: usize) -> Option<usize> {
    match *insn {
        Insn::Ja { off } | Insn::Jump { off, .. } => Some(insn::jump_target(pc, off)),
        Insn::Call(CallTarget::Relative(off)) => {
            usize::try_from(pc as i64 + 1 + i64::from(off)).ok()
        }
        _ => None,
    }
}

fn block_label(pc: usize) -> String {
    if pc == 0 {
        "entrypoint".into()
    } else {
        format!("lbb_{pc}")
    }
}

/// Prints `+0x10` or `-0x8`.
fn signed_hex(off: i16) -> String {
    let sign = if off < 0 { '-' } else { '+' };
    format!("{sign}{:#x}", off.unsigned_abs())
}

fn operand(source: Source) -> String {
    match source {
        Source::Imm(imm) => imm.to_string(),
        Source::Reg(reg) => format!("r{reg}"),
    }
}

fn size_suffix(size: Size) -> &'static str {
    match size {
        Size::B => "b",
        Size::H => "h",
        Size::W => "w",
        Size::DW => "dw",
    }
}

fn width_suffix(width: Width) -> &'static str {
    match width {
        Width::W32 => "32",
        Width::W64 => "64",
    }
}

fn alu_name(op: AluOp) -> &'static str {
    match op {
        AluOp::Add => "add",
        AluOp::Sub => "sub",
        AluOp::Mul => "mul",
        AluOp::Div => "div",
        AluOp::Or => "or",
        AluOp::And => "and",
        AluOp::Lsh => "lsh",
        AluOp::Rsh => "rsh",
        AluOp::Mod => "mod",
        AluOp::Xor => "xor",
        AluOp::Mov => "mov",
        AluOp::Arsh => "arsh",
    }
}

fn jmp_name(cond: JmpCond) -> &'static str {
    match cond {
        JmpCond::Eq => "jeq",
        JmpCond::Gt => "jgt",
        JmpCond::Ge => "jge",
        JmpCond::Set => "jset",
        JmpCond::Ne => "jne",
        JmpCond::Sgt => "jsgt",
        JmpCond::Sge => "jsge",
        JmpCond::Lt => "jlt",
        JmpCond::Le => "jle",
        JmpCond::Slt => "jslt",
        JmpCond::Sle => "jsle",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(bytes: [u8; 8]) -> String {
        disassemble(&bytes, Version::V0)
            .unwrap()
            .lines()
            .nth(1)
            .unwrap()
            .trim()
            .to_string()
    }

    #[test]
    fn formats_operands_like_the_reference() {
        assert_eq!(one([0x61, 0x21, 0, 0, 0, 0, 0, 0]), "ldxw r1, [r2+0x0]");
        assert_eq!(
            one([0x72, 0x0a, 0xf8, 0xff, 0xff, 0xff, 0xff, 0xff]),
            "stb [r10-0x8], -1"
        );
        assert_eq!(
            one([0x7b, 0x1a, 0x00, 0x80, 0, 0, 0, 0]),
            "stxdw [r10-0x8000], r1"
        );
        assert_eq!(one([0x0c, 0x21, 0, 0, 0, 0, 0, 0]), "add32 r1, r2");
        assert_eq!(
            one([0xb7, 0x0f, 0, 0, 0, 0, 0, 0x80]),
            "mov64 r15, -2147483648"
        );
        assert_eq!(one([0x87, 0x03, 0, 0, 0, 0, 0, 0]), "neg64 r3");
        assert_eq!(one([0xdc, 0x01, 0, 0, 17, 0, 0, 0]), "be17 r1");
        assert_eq!(one([0x85, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]), "syscall -1");
        assert_eq!(one([0x8d, 0x09, 0, 0, 0x01, 0x01, 0, 0]), "callx r1");
        assert_eq!(one([0x0d, 0, 0, 0, 0, 0, 0, 0]), "unknown opcode=0xd");
    }

    #[test]
    fn listing_labels_jump_targets() {
        #[rustfmt::skip]
        let program = [
            0x15, 0x01, 0x02, 0x00, 0, 0, 0, 0,          // 0: jeq r1, 0, +2  -> 3
            0x18, 0x02, 0, 0, 0xff, 0xff, 0xff, 0xff,    // 1: lddw r2, -1 (slots 1 and 2)
            0, 0, 0, 0, 0xff, 0xff, 0xff, 0xff,
            0x5d, 0x21, 0xfe, 0xff, 0, 0, 0, 0,          // 3: jne r1, r2, -2 -> 2, inside lddw
            0xa5, 0x01, 0x10, 0x00, 7, 0, 0, 0,          // 4: jlt r1, 7, +16 -> 21, past the end
            0x05, 0x00, 0xfa, 0xff, 0, 0, 0, 0,          // 5: ja -6 -> 0
        ];
        let expected = "\
entrypoint:
    jeq r1, 0, lbb_3
    lddw r2, 0xffffffffffffffff
lbb_3:
    jne r1, r2, [invalid]
    jlt r1, 7, [invalid]
    ja entrypoint
";
        assert_eq!(disassemble(&program, Version::V0).unwrap(), expected);
    }

    #[test]
    fn v3_listing_labels_call_targets() {
        #[rustfmt::skip]
        let program = [
            0x85, 0x10, 0, 0, 1, 0, 0, 0, // 0: call +1 (src 1) -> 2
            0x16, 0x01, 0, 0, 5, 0, 0, 0, // 1: jeq32 r1, 5, +0 -> 2
            0x8d, 0x03, 0, 0, 0, 0, 0, 0, // 2: callx r3, from dst
            0x95, 0, 0, 0, 0, 0, 0, 0,    // 3: exit
        ];
        let expected = "\
entrypoint:
    call lbb_2
    jeq32 r1, 5, lbb_2
lbb_2:
    callx r3
    exit
";
        assert_eq!(disassemble(&program, Version::V3).unwrap(), expected);
    }

    #[test]
    fn empty_program_is_empty_listing() {
        assert_eq!(disassemble(&[], Version::V0).unwrap(), "");
    }
}
