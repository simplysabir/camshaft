# 07: Differential testing

The idea: run the same program on camshaft and on Anza's VM (`solana-sbpf`, a dev-dependency
only) and check they end in exactly the same state. Any difference is a bug in camshaft.

## What is compared

`run_both` in `tests/common/mod.rs` runs one program on both VMs and checks:

- the result: the value of r0, or the exact error
- every register and the pc where it stopped
- the instruction count, which is the compute units used
- all of stack, heap and input memory

For ELFs, Anza's programs and random programs, `verify_both` first checks that both
verifiers agree on whether the program is valid.

## What goes in

| source | test | how many |
|---|---|---|
| hand-written programs from milestones 1 to 4 | `tests/interp.rs`, `calls.rs` | thousands of cases |
| Anza's own test ELFs (v0) | `tests/elf.rs` | 15 |
| every assembly program in Anza's `tests/execution.rs` | `tests/reference.rs` | 186 (184 run, 2 are v3 only and both verifiers reject them) |
| random programs | `tests/fuzz.rs` | 256 per `cargo test`, as many as you like with a time limit |
| real programs with syscalls and accounts | `tests/runtime.rs`, against Agave (see [06](06-input-and-compute.md)) | 22 runs |

## Random programs

`tests/fuzz.rs` builds programs the v0 verifier accepts, using `proptest`:

- a start that puts interesting values in r0 and r2 to r9: addresses near the edges of
  each memory region, 0, `u64::MAX`, and random numbers
- a main body and one to three functions, each up to 24 instructions and ending in `exit`
  (the last one sometimes does not, to run off the end of the code)
- ALU ops, byte swaps, loads and stores (mostly based on r1 and r10, so many hit real memory),
  jumps, `call`, `callx`, `lddw`
- a budget of 5, 50, 1,000 or 100,000 instructions, so some run out partway through

Jumps and calls pick a target instruction, not an offset, so they never land in the middle
of an `lddw`. The generator never makes a program the verifier rejects, but the programs
still crash in every way the VM can: access violations, stack gaps, division by zero,
too-deep calls, bad call targets, running out of budget, running off the end.

Run it for an hour:

```
CAMSHAFT_FUZZ_SECS=3600 cargo test --release --test fuzz -- --nocapture
```

It prints how many programs ran and how they ended. Each batch of 256 has a numbered seed
(set the first with `CAMSHAFT_FUZZ_SEED`), so a failure can be run again. On a difference,
proptest shrinks the program to the smallest one that still differs.

## What it found

No differences in camshaft. Two bugs in the test setup:

- Programs assembled from text start at the `entrypoint` label, which the assembler keys by
  pc, not by name. The harness now copies the entry pc from the reference.
- The generator first put r10 in the dst field of jumps. Only stores may do that.

## Not covered here

Syscalls. Anza's VM tests have no Agave syscalls, so syscalls are checked against Agave
itself in `tests/runtime.rs` instead.
