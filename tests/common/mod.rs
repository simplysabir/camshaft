//! Runs a program on both camshaft and `solana-sbpf` and checks they end in the same state.
#![allow(dead_code)] // each test file uses a different subset

use camshaft::error::VmError;
use camshaft::executable::Executable as Ours;
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
    verifier::{RequisiteVerifier, Verifier},
    vm::{CallFrame, Config, ContextObject, EbpfVm},
};
use std::sync::Arc;

pub type Reference = Executable<TestContextObject>;

pub fn loader() -> Arc<BuiltinProgram<TestContextObject>> {
    let config = Config {
        enabled_sbpf_versions: SBPFVersion::V0..=SBPFVersion::V0,
        ..Config::default()
    };
    Arc::new(BuiltinProgram::new_loader(config))
}

/// Assembles v0 source with the reference assembler.
pub fn assemble_reference(src: &str) -> Reference {
    assemble::<TestContextObject>(src, loader()).unwrap()
}

/// Just the bytecode of assembled source.
pub fn asm(src: &str) -> Vec<u8> {
    assemble_reference(src).get_text_bytes().1.to_vec()
}

/// Our executable with the same code, function keys and entry as a reference one built from
/// text. The assembler keys functions by pc, so the entry is copied rather than looked up.
pub fn ours_from_reference_text(reference: &Reference) -> Ours {
    let functions = reference
        .get_function_registry()
        .iter()
        .map(|(key, (_name, pc))| (key, pc))
        .collect();
    let mut ours = Ours::from_text(reference.get_text_bytes().1, functions);
    ours.entry_pc = reference.get_entrypoint_instruction_offset();
    ours
}

/// 1 KiB of 0, 1, ..., 255 repeated, for programs that read their input.
pub fn input() -> Vec<u8> {
    (0..=255).cycle().take(1024).collect()
}

/// Runs both verifiers, asserts they agree, and returns whether the program is valid.
pub fn verify_both(reference: &Reference, ours: &Ours) -> bool {
    let expected = RequisiteVerifier::verify(
        reference.get_text_bytes().1,
        SBPFVersion::V0,
        reference.get_config(),
        reference.get_function_registry(),
    );
    let verdict = ours.verify();
    assert_eq!(format!("{verdict:?}"), format!("{expected:?}"), "verifier");
    verdict.is_ok()
}

/// Runs raw bytecode with no functions.
pub fn check(program: &[u8], limit: u64) -> Result<u64, VmError> {
    check_with_input(program, &[], limit)
}

/// Same as `check`, with `input` mapped writable at the input region.
pub fn check_with_input(program: &[u8], input: &[u8], limit: u64) -> Result<u64, VmError> {
    let reference = Executable::from_text_bytes(
        program,
        loader(),
        SBPFVersion::V0,
        FunctionRegistry::default(),
    )
    .unwrap();
    run_both(
        &reference,
        &Ours::from_text(program, Default::default()),
        input,
        limit,
    )
}

/// Runs both executables and asserts the result or error, registers, pc, instruction count
/// and all writable memory match. Returns camshaft's result.
pub fn run_both(
    reference: &Reference,
    ours: &Ours,
    input: &[u8],
    limit: u64,
) -> Result<u64, VmError> {
    let config = reference.get_config();
    let mut stack = AlignedMemory::<HOST_ALIGN>::zero_filled(config.stack_size());
    let mut heap = AlignedMemory::<HOST_ALIGN>::zero_filled(HEAP_SIZE);
    let mut expected_input = input.to_vec();
    let input_region = MemoryRegion::new(&raw mut *expected_input.as_mut_slice(), MM_INPUT_START);
    let mut context = TestContextObject::new(limit);
    context.memory_mapping =
        create_memory_mapping(reference, &mut stack, &mut heap, vec![input_region], None).unwrap();
    let mut vm = EbpfVm::new(
        reference.get_loader().clone(),
        SBPFVersion::V0,
        &mut context,
        stack.len(),
    );
    vm.registers[1] = MM_INPUT_START;
    vm.registers[11] = reference.get_entrypoint_instruction_offset() as u64;
    vm.previous_instruction_meter = limit;
    let mut frames = vec![CallFrame::default(); config.max_call_depth];
    let registers = vm.registers;
    let mut interpreter = Interpreter::new(&mut vm, reference, registers, &mut frames);
    while interpreter.step() {}
    let expected_reg = interpreter.reg;
    // Memory errors move the pending count into the context, so add both.
    let expected_count = limit - vm.context().get_remaining() + vm.due_insn_count;

    let mut vm_ours = Vm::new(ours);
    vm_ours.memory.input = input.to_vec();
    let result = vm_ours.run(limit);
    let context = format!("program {:02x?}", reference.get_text_bytes().1);
    assert_eq!(
        format!("{result:?}"),
        format!("{:?}", vm.program_result),
        "{context}"
    );
    assert_eq!(vm_ours.reg, expected_reg[..11], "{context}");
    assert_eq!(vm_ours.pc as u64, expected_reg[11], "{context}");
    assert_eq!(vm_ours.insn_count, expected_count, "{context}");
    assert!(
        vm_ours.memory.stack == stack.as_slice(),
        "stack differs: {context}"
    );
    assert!(
        vm_ours.memory.heap == heap.as_slice(),
        "heap differs: {context}"
    );
    assert!(
        vm_ours.memory.input == expected_input,
        "input differs: {context}"
    );
    result
}
