//! Internal calls, `callx` and returns, compared against `solana-sbpf`.

mod common;

use camshaft::error::VmError;
use common::{assemble_reference, each_version, ours_from_reference_text, run_both};

const LIMIT: u64 = 100_000;

/// Assembles `src` as v0 and as v3, runs each on both VMs and returns camshaft's v0 result.
fn check(src: &str) -> Result<u64, VmError> {
    each_version(|version| {
        let reference = assemble_reference(src, version);
        run_both(
            &reference,
            &ours_from_reference_text(&reference),
            &[],
            LIMIT,
        )
    })
}

#[test]
fn nested_calls_restore_callee_saved_registers() {
    // r6 must survive both calls; function_a overwrites its own copy.
    let src = "
        mov64 r6, 100
        mov64 r1, 3
        call function_a
        add64 r0, r6
        exit
    function_a:
        mov64 r6, 7
        add64 r1, 1
        call function_b
        add64 r0, r1
        exit
    function_b:
        mov64 r0, r1
        mul64 r0, 2
        exit";
    // function_b returns 8, function_a adds r1 = 4, entrypoint adds r6 = 100.
    assert_eq!(check(src), Ok(112));
}

#[test]
fn recursive_factorial_uses_one_stack_frame_per_call() {
    let src = "
        mov64 r1, 10
        call function_fact
        exit
    function_fact:
        mov64 r0, 1
        jle r1, 1, +5
        stxdw [r10-8], r1
        sub64 r1, 1
        call function_fact
        ldxdw r1, [r10-8]
        mul64 r0, r1
        exit";
    assert_eq!(check(src), Ok(3_628_800));
}

/// Recurses `depth` times below a first call from the entrypoint.
fn countdown(depth: u32) -> String {
    format!(
        "
        mov64 r1, {depth}
        call function_down
        exit
    function_down:
        jeq r1, 0, +2
        sub64 r1, 1
        call function_down
        exit"
    )
}

#[test]
fn call_depth_limit_is_64_frames() {
    // The entrypoint's call plus 62 more is 63 frames: fine. One more hits the limit.
    assert_eq!(check(&countdown(62)), Ok(0));
    assert_eq!(check(&countdown(63)), Err(VmError::CallDepthExceeded));
}

#[test]
fn callx_jumps_to_an_address_in_the_code() {
    // The code starts at 0x1_0000_0000, so pc 4 is at 0x1_0000_0020.
    let src = "
        lddw r2, 0x100000020
        callx r2
        exit
        mov64 r0, 42
        exit";
    assert_eq!(check(src), Ok(42));
    // An address inside an instruction rounds down to its start.
    assert_eq!(check(&src.replace("0x100000020", "0x100000023")), Ok(42));
    for outside in ["0x100000030", "0x0", "0xffffffffffffffff"] {
        let result = check(&src.replace("0x100000020", outside));
        assert_eq!(result, Err(VmError::CallOutsideTextSegment), "{outside}");
    }
}

#[test]
fn callx_counts_toward_the_depth_limit() {
    // Calls itself through r2 forever.
    let src = "
        lddw r2, 0x100000010
        callx r2
        exit";
    assert_eq!(check(src), Err(VmError::CallDepthExceeded));
}
