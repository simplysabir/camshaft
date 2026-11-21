//! Loads the v0 test ELFs with both camshaft and `solana-sbpf` and checks they agree.

mod common;

use camshaft::executable::Executable as Ours;
use common::{Reference, loader, run_both};
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

/// Everything a load decides: read-only bytes and address, code address and bytes,
/// entry pc, and function keys.
type Loaded = (Vec<u8>, u64, u64, Vec<u8>, usize, BTreeMap<u32, usize>);

fn loaded_reference(e: &Reference) -> Loaded {
    let (text_addr, text) = e.get_text_bytes();
    let functions = e
        .get_function_registry()
        .iter()
        .map(|(key, (_, pc))| (key, pc))
        .collect();
    let ro_addr = e.get_ro_region().vm_addr_range().start;
    (
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
        e.ro.clone(),
        e.ro_addr,
        e.text_addr(),
        e.text_bytes().to_vec(),
        e.entry_pc,
        e.functions.clone(),
    )
}

/// Loads `bytes` both ways and asserts the same outcome. Returns both on success.
fn load_both(bytes: &[u8], what: &str) -> Option<(Reference, Ours)> {
    match (Reference::load(bytes, loader()), Ours::from_elf(bytes)) {
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
    for (name, bytes) in fixtures() {
        // The reference VM has none of Agave's syscalls, so programs that call them are only
        // compared on loading. tests/syscalls.rs runs those.
        if let Some((reference, ours)) = load_both(&bytes, &name)
            && ours.syscall_names.is_empty()
        {
            run_both(&reference, &ours, &input, 10_000_000).ok();
        }
    }
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
    let bytes = fs::read(format!("{FIXTURES}/relative_call_sbpfv0.so")).unwrap();
    let (reference, ours) = load_both(&bytes, "relative_call").unwrap();
    assert_eq!(run_both(&reference, &ours, &[5], 1000), Ok(11));
}

/// Corrupts every byte of a few small fixtures, and truncates them at every length.
/// Both loaders must accept or reject each variant the same way.
#[test]
fn damaged_files_load_like_the_reference() {
    for name in [
        "relative_call",
        "reloc_64_relative_data",
        "struct_func_pointer",
    ] {
        let original = fs::read(format!("{FIXTURES}/{name}_sbpfv0.so")).unwrap();
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
