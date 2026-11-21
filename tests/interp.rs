//! Runs programs on both camshaft and `solana-sbpf` and checks they end in the same state.

use camshaft::error::VmError;
use camshaft::interp::Vm;
use camshaft::memory::HEAP_SIZE;
use solana_sbpf::{
    aligned_memory::AlignedMemory,
    assembler::assemble,
    ebpf::{HOST_ALIGN, MM_INPUT_START},
    elf::Executable,
    interpreter::Interpreter,
    memory_region::MemoryRegion,
    program::{BuiltinProgram, FunctionRegistry, SBPFVersion},
    test_utils::{TestContextObject, create_memory_mapping},
    vm::{CallFrame, Config, ContextObject, EbpfVm},
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
    check_with_input(program, &[], limit)
}

/// Same as `check`, with `input` mapped writable at the input region.
fn check_with_input(program: &[u8], input: &[u8], limit: u64) -> Result<u64, VmError> {
    let executable = Executable::from_text_bytes(
        program,
        loader(),
        SBPFVersion::V0,
        FunctionRegistry::default(),
    )
    .unwrap();
    let mut stack = AlignedMemory::<HOST_ALIGN>::zero_filled(executable.get_config().stack_size());
    let mut heap = AlignedMemory::<HOST_ALIGN>::zero_filled(HEAP_SIZE);
    let mut expected_input = input.to_vec();
    let input_region = MemoryRegion::new(&raw mut *expected_input.as_mut_slice(), MM_INPUT_START);
    let mut context = TestContextObject::new(limit);
    context.memory_mapping =
        create_memory_mapping(&executable, &mut stack, &mut heap, vec![input_region], None)
            .unwrap();
    let mut vm = EbpfVm::new(
        executable.get_loader().clone(),
        SBPFVersion::V0,
        &mut context,
        stack.len(),
    );
    vm.registers[1] = MM_INPUT_START;
    vm.previous_instruction_meter = limit;
    let mut frames = vec![CallFrame::default(); executable.get_config().max_call_depth];
    let registers = vm.registers;
    let mut interpreter = Interpreter::new(&mut vm, &executable, registers, &mut frames);
    while interpreter.step() {}
    let expected_reg = interpreter.reg;
    // Memory errors move the pending count into the context, so add both.
    let expected_count = limit - vm.context().get_remaining() + vm.due_insn_count;

    let mut ours = Vm::new(program);
    ours.memory.input = input.to_vec();
    let result = ours.run(limit);
    let context = format!("program {program:02x?}");
    assert_eq!(
        format!("{result:?}"),
        format!("{:?}", vm.program_result),
        "{context}"
    );
    assert_eq!(ours.reg, expected_reg[..11], "{context}");
    assert_eq!(ours.pc as u64, expected_reg[11], "{context}");
    assert_eq!(ours.insn_count, expected_count, "{context}");
    assert!(
        ours.memory.stack == stack.as_slice(),
        "stack differs: {context}"
    );
    assert!(
        ours.memory.heap == heap.as_slice(),
        "heap differs: {context}"
    );
    assert!(
        ours.memory.input == expected_input,
        "input differs: {context}"
    );
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
    assert_eq!(Vm::new(&back).run(LIMIT), Err(VmError::ExecutionOverrun));
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

const STACK: u64 = 0x2_0000_0000;
const HEAP: u64 = 0x3_0000_0000;
const INPUT: u64 = 0x4_0000_0000;
const WIDTHS: [&str; 4] = ["b", "h", "w", "dw"];

fn input_bytes() -> Vec<u8> {
    (1..=64).collect()
}

#[test]
fn loads_and_stores_match_reference() {
    // An address inside each writable region.
    for base in [STACK + 0x800, HEAP + 0x10, INPUT + 8] {
        for store in WIDTHS {
            for load in WIDTHS {
                for value in VALUES {
                    let src = format!(
                        "lddw r2, {base}\nlddw r3, {}\nstx{store} [r2+8], r3\nldx{load} r0, [r2+8]\nexit",
                        value as i64
                    );
                    check_with_input(&asm(&src), &input_bytes(), LIMIT).unwrap();
                }
                for imm in IMMS {
                    let src = format!(
                        "lddw r2, {base}\nst{store} [r2-4], {imm}\nldx{load} r0, [r2-4]\nexit"
                    );
                    check_with_input(&asm(&src), &input_bytes(), LIMIT).unwrap();
                }
            }
        }
    }
}

#[test]
fn access_violations_match_reference() {
    let program = 0x1_0000_0000u64;
    let heap_end = HEAP + camshaft::memory::HEAP_SIZE as u64;
    let accesses = [
        format!("lddw r2, {program}\nldxdw r0, [r2+0]"), // reading the program is fine
        format!("lddw r2, {program}\nstb [r2+0], 1"),    // writing it is not
        format!("lddw r2, {heap_end}\nldxdw r0, [r2-4]"), // runs off the heap
        format!("lddw r2, {heap_end}\nldxb r0, [r2+0]"),
        "ldxb r0, [r10+0]".into(),  // first stack gap
        "ldxdw r0, [r10-4]".into(), // only the start is checked, so this reads into frame 1
        "stw [r10+4096], 7".into(), // frame 1
        format!("lddw r2, {}\nldxb r0, [r2+0]", STACK + 127 * 4096), // last gap
        format!("lddw r2, {}\nldxb r0, [r2+0]", STACK + 128 * 4096), // past the stack
        format!("lddw r2, {STACK}\nldxb r0, [r2-1]"), // just below the stack
        format!("lddw r2, {STACK}\nldxb r0, [r2-8193]"),
        "ldxdw r0, [r1+60]".into(), // runs off the input
        "lddw r2, 0\nldxb r0, [r2+0]".into(),
        "lddw r2, 0\nldxb r0, [r2-1]".into(), // wraps to u64::MAX
        "lddw r2, 0x500000000\nstb [r2+0], 1".into(),
    ];
    for access in accesses {
        check_with_input(&asm(&format!("{access}\nexit")), &input_bytes(), LIMIT).ok();
    }
    // With no input, the input region is empty.
    check(&asm("ldxb r0, [r1+0]\nexit"), LIMIT).unwrap_err();
}

#[test]
fn violation_errors_name_the_place() {
    use camshaft::error::AccessType::{Load, Store};
    let store_program = asm("lddw r2, 0x100000000\nstb [r2+0], 1\nexit");
    assert_eq!(
        check(&store_program, LIMIT),
        Err(VmError::AccessViolation(Store, 0x1_0000_0000, 1, "program"))
    );
    assert_eq!(
        check(&asm("ldxb r0, [r10+0]\nexit"), LIMIT),
        Err(VmError::StackAccessViolation(Load, STACK + 4096, 1, 1))
    );
    let mut vm = Vm::new(&store_program);
    vm.run(LIMIT).unwrap_err();
    assert_eq!(vm.pc, 2, "pc points at the faulting store");
}

#[test]
fn sums_input_bytes() {
    let sum = "mov64 r0, 0\nmov64 r2, 0\nldxb r3, [r1+0]\nadd64 r0, r3\nadd64 r1, 1\nadd64 r2, 1\njlt r2, 8, -5\nexit";
    assert_eq!(check_with_input(&asm(sum), &input_bytes(), LIMIT), Ok(36));
}
