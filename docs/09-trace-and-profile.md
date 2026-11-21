# 09: Tracing and profiling

Two commands that watch a program run, one instruction at a time. Both take the same
arguments as `run` and print the same logs and result at the end.

## `camshaft trace`

One line per instruction:

```
$ camshaft trace tour.so --input tour.json
     CU     pc  instruction                       changes
      1      5  mov64 r9, r2                      r9=0x4000050f8
      2      6  mov64 r2, r1                      r2=0x400000000
      ...
      7    981  stxdw [r10-0xf58], r1             [0x2000010a8]=0x2000000d0
     ...
    384   1043  syscall sol_log_64_               r0=0x0
```

- **CU**: compute units used so far, including this instruction. A syscall jumps by its cost.
- **pc**: the instruction's slot number.
- **instruction**: like `disasm`, but calls show the syscall or function name.
- **changes**: each register the instruction changed, and each memory write: the value for
  up to 8 bytes, the address range for more (like a `memcpy`).

## `camshaft profile`

Compute units per function, most first:

```
$ camshaft profile tour_v3_unstripped.so --input tour.json --folded tour.folded
  self CU  total CU   calls  function
     3001      3001       1  sol_try_find_program_address
     1501      1501       1  sol_create_program_address
      505       505       5  sol_log_64_
      278      7380       1  entrypoint
      169       451       1  solana_program_entrypoint::deserialize
      ...
```

- **self CU**: spent in the function's own instructions.
- **total CU**: spent in the function and everything it called. The entrypoint's total is the
  whole run.
- A syscall is its own row: its cost is not part of the calling function's self CU.
- If the program crashes outside a syscall, the runtime charges the rest of the budget.
  That shows as a `plus N CU` line under the table.

`--folded` writes one `entrypoint;caller;callee units` line per call stack, the format
flame graph tools read: `inferno-flamegraph < tour.folded > tour.svg`.

## Function names

Names come from the ELF's `.symtab`, demangled (`tour::process`, not `_ZN4tour7process...`).
`cargo build-sbf` strips the copy it puts in `target/deploy`, and programs dumped from mainnet
are stripped too. For names, run the unstripped copy in `target/<target>/release/`; it runs
the same. Without names, functions show as `function_<pc>`, and the entry as `entrypoint`.

## How it works

- The VM takes an `Observer` (`src/interp.rs`) that it calls after every instruction, with the
  instruction, what it cost, and the registers and call depth from just before it ran. A
  plain `run` uses `()`, which does nothing and costs nothing.
- The tracer (`src/trace.rs`) asks the VM to record memory writes and prints a line.
- The profiler (`src/profile.rs`) keeps its own call stack: when the call depth goes up it
  pushes the function at the new pc, when it goes down it pops. Each instruction's cost goes
  to the stack it ran with.
- Names are read separately from loading (`src/elf/symbols.rs`), so a strange symbol table
  can never change what loads or how it runs.

## How we know it is right

`tests/observe.rs` runs every tour scenario, built as v0 and as v3:

- With the tracer and the profiler watching, the result, logs, compute units and accounts are
  the same as a plain run, and the trace has one line per instruction.
- The profile adds up to the compute units charged, unless a crash charged the whole budget.
- The profile of tour's happy path names its most expensive part,
  `sol_try_find_program_address`, at 3,001 units.

Writing these tests showed that tour's scenarios 10 and 13 declared two syscalls by hand.
In a v3 build those calls never reached the syscall, so the v3 runs did not test what they
said. They now use the SDK's declarations, and both versions hit the syscall.
