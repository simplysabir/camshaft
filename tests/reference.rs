//! Runs the reference's own test programs on both camshaft and `solana-sbpf`.
//!
//! `tests/reference/execution.s` holds every assembly program from `tests/execution.rs` in
//! anza-xyz/sbpf v0.28.0 (Apache-2.0 or MIT), one after each `--- test_name` line.

mod common;

use camshaft::version::Version;
use common::{VERSIONS, input, loader, ours_from_reference_text, run_both, verify_both};
use solana_sbpf::{assembler::assemble, test_utils::TestContextObject};
use std::fs;

const LIMIT: u64 = 100_000;

/// Assembles every program as `version` and runs it on both VMs. Returns how many ran and
/// how many both verifiers rejected.
fn run_corpus(version: Version) -> (usize, usize) {
    let corpus = fs::read_to_string("tests/reference/execution.s").unwrap();
    let (mut ran, mut rejected) = (0, 0);
    for program in corpus.split("--- ").skip(1) {
        let (name, source) = program.split_once('\n').unwrap();
        let reference = assemble::<TestContextObject>(source, loader(version)).unwrap();
        let ours = ours_from_reference_text(&reference);
        eprintln!("running {name} as {version:?}");
        // Two programs use v2 instructions, which neither v0 nor v3 has.
        if !verify_both(&reference, &ours) {
            rejected += 1;
            continue;
        }
        // The original tests each pass their own input; one patterned buffer serves them all.
        run_both(&reference, &ours, &input(), LIMIT).ok();
        ran += 1;
    }
    (ran, rejected)
}

#[test]
fn reference_programs_match() {
    for version in VERSIONS {
        assert_eq!(run_corpus(version), (184, 2), "{version:?}");
    }
}
