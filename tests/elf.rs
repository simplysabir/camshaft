//! Loads the v0 and v3 test ELFs with both camshaft and `solana-sbpf` and checks they agree.

mod common;

use camshaft::elf::ElfError;
use camshaft::executable::Executable as Ours;
use camshaft::insn::decode_program;
use camshaft::syscalls;
use common::{Reference, elf_loader, run_both, verify_both};
use std::collections::BTreeMap;
use std::fs;

const FIXTURES: &str = "tests/programs";

fn fixtures() -> Vec<(String, Vec<u8>)> {
    let mut files: Vec<_> = fs::read_dir(FIXTURES)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "so"))
        .map(|path| (path.display().to_string(), fs::read(&path).unwrap()))
        .collect();
    files.sort();
    files
}

/// Everything a load decides: version, read-only bytes and address, code address and bytes,
/// entry pc, and function keys. The version is compared by name.
type Loaded = (
    String,
    Vec<u8>,
    u64,
    u64,
    Vec<u8>,
    usize,
    BTreeMap<u32, usize>,
);

fn loaded_reference(e: &Reference) -> Loaded {
    let (text_addr, text) = e.get_text_bytes();
    let functions = e
        .get_function_registry()
        .iter()
        .map(|(key, (_, pc))| (key, pc))
        .collect();
    let ro_addr = e.get_ro_region().vm_addr_range().start;
    (
        format!("{:?}", e.get_sbpf_version()),
        e.get_ro_section().to_vec(),
        ro_addr,
        text_addr,
        text.to_vec(),
        e.get_entrypoint_instruction_offset(),
        functions,
    )
}

fn loaded_ours(e: &Ours) -> Loaded {
    (
        format!("{:?}", e.version),
        e.ro.clone(),
        e.ro_addr,
        e.text_addr,
        e.text.clone(),
        e.entry_pc,
        e.functions.clone(),
    )
}

/// Loads `bytes` both ways and asserts the same outcome. Returns both on success.
fn load_both(bytes: &[u8], what: &str) -> Option<(Reference, Ours)> {
    // e_flags 1 and 2 are v1 and v2, which the reference loads and camshaft does not.
    let e_flags = bytes
        .get(48..52)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()));
    if let Some(1 | 2) = e_flags {
        assert_eq!(
            Ours::from_elf(bytes).err(),
            Some(ElfError::UnsupportedSBPFVersion),
            "{what}"
        );
        return None;
    }
    match (Reference::load(bytes, elf_loader()), Ours::from_elf(bytes)) {
        (Ok(reference), Ok(ours)) => {
            assert!(
                loaded_reference(&reference) == loaded_ours(&ours),
                "load differs: {what}"
            );
            Some((reference, ours))
        }
        (Err(expected), Err(got)) => {
            assert_eq!(format!("{got:?}"), format!("{expected:?}"), "{what}");
            None
        }
        (expected, got) => panic!(
            "{what}: reference {:?}, ours {:?}",
            expected.err(),
            got.err()
        ),
    }
}

#[test]
fn fixtures_load_and_run_like_the_reference() {
    let input: Vec<u8> = (1..=64).collect();
    let mut ran = Vec::new();
    for (name, bytes) in fixtures() {
        // The reference VM has none of Agave's syscalls, so programs that call them are only
        // compared on loading. tests/runtime.rs runs those against Agave.
        if let Some((reference, ours)) = load_both(&bytes, &name)
            && verify_both(&reference, &ours)
            && !calls_a_syscall(&ours)
        {
            run_both(&reference, &ours, &input, 10_000_000).ok();
            ran.push(name);
        }
    }
    // Every ELF from Anza's repo that loads, v0 and v3.
    assert_eq!(ran.len(), 25, "ran {ran:#?}");
}

/// Whether the code calls one of camshaft's syscalls.
fn calls_a_syscall(exe: &Ours) -> bool {
    let insns = decode_program(&exe.text, exe.version).unwrap();
    insns
        .iter()
        .any(|(_, insn)| insn.syscall_key().and_then(syscalls::lookup).is_some())
}

#[test]
fn writable_sections_are_rejected() {
    for name in ["bss_section", "data_section"] {
        let bytes = fs::read(format!("{FIXTURES}/{name}_sbpfv0.so")).unwrap();
        assert!(Ours::from_elf(&bytes).is_err(), "{name}");
        load_both(&bytes, name);
    }
}

#[test]
fn relative_call_runs() {
    // entrypoint(x) = function_sum(function_stack_ref(x) , x) = (x + 1) + x
    for version in ["v0", "v3"] {
        let bytes = fs::read(format!("{FIXTURES}/relative_call_sbpf{version}.so")).unwrap();
        let (reference, ours) = load_both(&bytes, version).unwrap();
        assert_eq!(run_both(&reference, &ours, &[5], 1000), Ok(11), "{version}");
    }
}

/// Corrupts every byte of a few small fixtures, and truncates them at every length.
/// Both loaders must accept or reject each variant the same way.
#[test]
fn damaged_files_load_like_the_reference() {
    for name in [
        "relative_call_sbpfv0",
        "reloc_64_relative_data_sbpfv0",
        "struct_func_pointer_sbpfv0",
        "relative_call_sbpfv3",
        "strict_header_sbpfv3",
    ] {
        let original = fs::read(format!("{FIXTURES}/{name}.so")).unwrap();
        for len in 0..original.len() {
            load_both(&original[..len], &format!("{name} truncated to {len}"));
        }
        for at in 0..original.len() {
            let byte = original[at];
            for value in [0x00, 0xff, byte ^ 0x01, byte.wrapping_add(8)] {
                let mut bytes = original.clone();
                bytes[at] = value;
                load_both(&bytes, &format!("{name} byte {at} = {value:#x}"));
            }
        }
    }
}
