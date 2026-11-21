//! Runs a program on both camshaft and `solana-sbpf` and checks they end in the same state.
#![allow(dead_code)] // each test file uses a different subset

use camshaft::error::VmError;
use camshaft::executable::Executable as Ours;
use camshaft::interp::Vm;
use camshaft::memory::HEAP_SIZE;
use camshaft::version::Version;
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

pub const VERSIONS: [Version; 2] = [Version::V0, Version::V3];

pub fn sbpf_version(version: Version) -> SBPFVersion {
    match version {
        Version::V0 => SBPFVersion::V0,
        Version::V3 => SBPFVersion::V3,
    }
}

fn our_version(reference: &Reference) -> Version {
    match reference.get_sbpf_version() {
        SBPFVersion::V0 => Version::V0,
        SBPFVersion::V3 => Version::V3,
        other => panic!("camshaft does not run {other:?}"),
    }
}

/// A reference loader for one version. The assembler builds for it.
pub fn loader(version: Version) -> Arc<BuiltinProgram<TestContextObject>> {
    let version = sbpf_version(version);
    let config = Config {
        enabled_sbpf_versions: version..=version,
        ..Config::default()
    };
    Arc::new(BuiltinProgram::new_loader(config))
}

/// A reference loader that accepts v0 to v3 ELFs, like Agave's.
pub fn elf_loader() -> Arc<BuiltinProgram<TestContextObject>> {
    let config = Config {
        enabled_sbpf_versions: SBPFVersion::V0..=SBPFVersion::V3,
        ..Config::default()
    };
    Arc::new(BuiltinProgram::new_loader(config))
}

/// Assembles source with the reference assembler.
pub fn assemble_reference(src: &str, version: Version) -> Reference {
    assemble::<TestContextObject>(src, loader(version)).unwrap()
}

/// Just the v0 bytecode of assembled source.
pub fn asm(src: &str) -> Vec<u8> {
    assemble_reference(src, Version::V0)
        .get_text_bytes()
        .1
        .to_vec()
}

/// Our executable with the same code, version, function keys and entry as a reference one
/// built from text. The assembler keys functions by pc, so the entry is copied rather than
/// looked up.
pub fn ours_from_reference_text(reference: &Reference) -> Ours {
    let functions = reference
        .get_function_registry()
        .iter()
        .map(|(key, (_name, pc))| (key, pc))
        .collect();
    let text = reference.get_text_bytes().1;
    let mut ours = Ours::from_text(text, functions, our_version(reference));
    ours.entry_pc = reference.get_entrypoint_instruction_offset();
    ours
}

/// Runs `run` as v0 and as v3, and returns the v0 result.
pub fn each_version(mut run: impl FnMut(Version) -> Result<u64, VmError>) -> Result<u64, VmError> {
    let v0 = run(Version::V0);
    run(Version::V3).ok();
    v0
}

/// 1 KiB of 0, 1, ..., 255 repeated, for programs that read their input.
pub fn input() -> Vec<u8> {
    (0..=255).cycle().take(1024).collect()
}

/// Runs both verifiers, asserts they agree, and returns whether the program is valid.
pub fn verify_both(reference: &Reference, ours: &Ours) -> bool {
    let expected = RequisiteVerifier::verify(
        reference.get_text_bytes().1,
        reference.get_sbpf_version(),
        reference.get_config(),
        reference.get_function_registry(),
    );
    let verdict = ours.verify();
    assert_eq!(format!("{verdict:?}"), format!("{expected:?}"), "verifier");
    verdict.is_ok()
}

/// Runs raw bytecode with no functions, as v0 and as v3. Returns the v0 result.
pub fn check(program: &[u8], limit: u64) -> Result<u64, VmError> {
    check_with_input(program, &[], limit)
}

/// Same as `check`, with `input` mapped writable at the input region.
pub fn check_with_input(program: &[u8], input: &[u8], limit: u64) -> Result<u64, VmError> {
    each_version(|version| {
        let reference = Executable::from_text_bytes(
            program,
            loader(version),
            sbpf_version(version),
            FunctionRegistry::default(),
        )
        .unwrap();
        let ours = Ours::from_text(program, Default::default(), version);
        run_both(&reference, &ours, input, limit)
    })
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
        reference.get_sbpf_version(),
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
    let context = format!(
        "{:?} program {:02x?}",
        reference.get_sbpf_version(),
        reference.get_text_bytes().1
    );
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
