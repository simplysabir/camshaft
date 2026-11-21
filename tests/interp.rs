//! Runs programs on both camshaft and `solana-sbpf` and checks they end in the same state.

use camshaft::interp::{Vm, VmError};
use solana_sbpf::{
    assembler::assemble,
    create_vm,
    elf::Executable,
    interpreter::Interpreter,
    program::{BuiltinProgram, FunctionRegistry, SBPFVersion},
    test_utils::TestContextObject,
    vm::{CallFrame, Config},
};
use std::sync::Arc;

const LIMIT: u64 = 1000;

fn loader() -> Arc<BuiltinProgram<TestContextObject>> {
    let config = Config {
        enabled_sbpf_versions: SBPFVersion::V0..=SBPFVersion::V0,
        ..Config::default()
    };
    Arc::new(BuiltinProgram::new_loader(config))
}

fn asm(src: &str) -> Vec<u8> {
    let executable = assemble::<TestContextObject>(src, loader()).unwrap();
    executable.get_text_bytes().1.to_vec()
}

/// Runs `program` on both VMs, asserts they agree, and returns camshaft's result.
fn check(program: &[u8], limit: u64) -> Result<u64, VmError> {
    let executable = Executable::from_text_bytes(
        program,
        loader(),
        SBPFVersion::V0,
        FunctionRegistry::default(),
    )
    .unwrap();
    let mut context = TestContextObject::new(limit);
    create_vm!(vm, &executable, &mut context, stack, heap, vec![], None);
    vm.previous_instruction_meter = limit;
    let mut frames = vec![CallFrame::default(); executable.get_config().max_call_depth];
    let registers = vm.registers;
    let mut interpreter = Interpreter::new(&mut vm, &executable, registers, &mut frames);
    while interpreter.step() {}
    let expected_reg = interpreter.reg;

    let mut ours = Vm::default();
    let result = ours.run(program, limit);
    let context = format!("program {program:02x?}");
    assert_eq!(
        format!("{result:?}"),
        format!("{:?}", vm.program_result),
        "{context}"
    );
    assert_eq!(ours.reg, expected_reg[..11], "{context}");
    assert_eq!(ours.pc as u64, expected_reg[11], "{context}");
    assert_eq!(ours.insn_count, vm.due_insn_count, "{context}");
    result
}

/// Edge values: zero, small, shift widths, 32-bit and 64-bit boundaries.
const VALUES: [u64; 20] = [
    0,
    1,
    2,
    7,
    31,
    32,
    33,
    63,
    64,
    65,
    0x7fff_ffff,
    0x8000_0000,
    0xffff_ffff,
    0x1_0000_0000,
    0x1_0000_0001,
    i64::MAX as u64,
    i64::MIN as u64,
    u64::MAX,
    u64::MAX - 1,
    0xdead_beef_cafe_f00d,
];
const IMMS: [i32; 13] = [0, 1, -1, 2, 7, 31, 32, 33, 63, 64, 65, i32::MAX, i32::MIN];
const ALU_OPS: [&str; 12] = [
    "add", "sub", "mul", "div", "mod", "or", "and", "lsh", "rsh", "arsh", "xor", "mov",
];
const JUMPS: [&str; 11] = [
    "jeq", "jne", "jgt", "jge", "jlt", "jle", "jset", "jsgt", "jsge", "jslt", "jsle",
];

#[test]
fn alu_matches_reference() {
    for op in ALU_OPS {
        for width in ["32", "64"] {
            for a in VALUES {
                let a = a as i64;
                for b in VALUES {
                    check(
                        &asm(&format!(
                            "lddw r1, {a}\nlddw r2, {}\n{op}{width} r1, r2\nexit",
                            b as i64
                        )),
                        LIMIT,
                    )
                    .ok();
                }
                for imm in IMMS {
                    // The verifier rejects division by a zero immediate; the reference would panic.
                    if imm == 0 && matches!(op, "div" | "mod") {
                        continue;
                    }
                    check(
                        &asm(&format!("lddw r1, {a}\n{op}{width} r1, {imm}\nexit")),
                        LIMIT,
                    )
                    .ok();
                }
            }
        }
    }
}

#[test]
fn neg_and_byte_swaps_match_reference() {
    let ops = [
        "neg32 r1", "neg64 r1", "le16 r1", "le32 r1", "le64 r1", "be16 r1", "be32 r1", "be64 r1",
    ];
    for a in VALUES {
        for op in ops {
            check(&asm(&format!("lddw r1, {}\n{op}\nexit", a as i64)), LIMIT).ok();
        }
    }
    // be with an invalid width fails at runtime.
    let be17 = [0xdc, 0x01, 0, 0, 17, 0, 0, 0, 0x95, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(check(&be17, LIMIT), Err(VmError::InvalidInstruction));
}

#[test]
fn jumps_match_reference() {
    for jump in JUMPS {
        for a in VALUES {
            let a = a as i64;
            for b in VALUES {
                let src = format!(
                    "lddw r1, {a}\nlddw r2, {}\n{jump} r1, r2, +1\nexit\nmov64 r0, 1\nexit",
                    b as i64
                );
                check(&asm(&src), LIMIT).unwrap();
            }
            for imm in IMMS {
                let src = format!("lddw r1, {a}\n{jump} r1, {imm}, +1\nexit\nmov64 r0, 1\nexit");
                check(&asm(&src), LIMIT).unwrap();
            }
        }
    }
}

#[test]
fn control_flow_matches_reference() {
    let sum = "mov64 r0, 0\nmov64 r1, 10\nadd64 r0, r1\nsub64 r1, 1\njne r1, 0, -3\nexit";
    assert_eq!(check(&asm(sum), LIMIT), Ok(55));

    assert_eq!(
        check(&asm("mov64 r0, 1"), LIMIT),
        Err(VmError::ExecutionOverrun)
    );
    assert_eq!(
        check(&asm("ja +5\nexit"), LIMIT),
        Err(VmError::ExecutionOverrun)
    );
    // A negative target overflows the reference's pc check in debug builds, so check ours alone.
    let back = asm("ja -2\nexit");
    assert_eq!(
        Vm::default().run(&back, LIMIT),
        Err(VmError::ExecutionOverrun)
    );
    assert_eq!(
        check(&asm("ja -1"), LIMIT),
        Err(VmError::ExceededMaxInstructions)
    );

    // The limit counts lddw once. Three instructions run here.
    let three = asm("lddw r0, 5\nmov64 r1, 1\nexit");
    assert_eq!(check(&three, 3), Ok(5));
    assert_eq!(check(&three, 2), Err(VmError::ExceededMaxInstructions));

    // call to an unregistered hash
    let call = [0x85, 0, 0, 0, 99, 0, 0, 0, 0x95, 0, 0, 0, 0, 0, 0, 0];
    assert_eq!(check(&call, LIMIT), Err(VmError::UnsupportedInstruction));
}
