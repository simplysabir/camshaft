//! Runs the reference's own test programs on both camshaft and `solana-sbpf`.
//!
//! `tests/reference/execution.s` holds every assembly program from `tests/execution.rs` in
//! anza-xyz/sbpf v0.28.0 (Apache-2.0 or MIT), one after each `--- test_name` line.

mod common;

use common::{input, loader, ours_from_reference_text, run_both, verify_both};
use solana_sbpf::{assembler::assemble, test_utils::TestContextObject};
use std::fs;

const LIMIT: u64 = 100_000;

#[test]
fn reference_programs_match() {
    let corpus = fs::read_to_string("tests/reference/execution.s").unwrap();
    let (mut ran, mut rejected) = (0, 0);
    for program in corpus.split("--- ").skip(1) {
        let (name, source) = program.split_once('\n').unwrap();
        let reference = assemble::<TestContextObject>(source, loader()).unwrap();
        let ours = ours_from_reference_text(&reference);
        eprintln!("running {name}");
        // Programs written for v3 use instructions v0 does not have.
        if !verify_both(&reference, &ours) {
            rejected += 1;
            continue;
        }
        // The original tests each pass their own input; one patterned buffer serves them all.
        run_both(&reference, &ours, &input(), LIMIT).ok();
        ran += 1;
    }
    assert_eq!((ran, rejected), (184, 2));
}
