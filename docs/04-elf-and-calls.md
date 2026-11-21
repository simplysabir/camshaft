# 04: ELF loading and function calls

camshaft loads a real `.so` built by `cargo build-sbf` and runs programs that
call their own functions.

## What is in a `.so`

A compiled program is an ELF file. The parts that matter:

| part | what it holds |
|---|---|
| `.text` | the bytecode |
| `.rodata`, `.data.rel.ro`, `.eh_frame` | read-only data, like string literals |
| dynamic symbols and relocations | a to-do list of spots in the code the loader must fix up |
| `e_entry` | where execution starts |

`.bss` and writable `.data` are rejected: Solana programs cannot have global mutable state.

## Loading, step by step (`src/elf/`)

1. **Parse** (`parse.rs`): read headers and tables, checking every size, offset and alignment.
2. **Validate**: right machine, a shared object, exactly one `.text`, entrypoint inside `.text`.
3. **Relocate** a copy of the file:
   - `call imm` with a relative offset becomes `call key`, where key = murmur3 hash of the target pc.
   - `R_BPF_64_64` / `R_BPF_64_RELATIVE`: put the real address (`0x1_0000_0000 + addr`) into `lddw`s and data.
   - `R_BPF_64_32`: a call by name. A function in this file gets its key; anything else is a syscall, keyed by the hash of its name (`sol_log_` and friends, see [05](05-syscalls.md)).
4. **Lay out read-only memory**: `.text` and the read-only sections, placed at `0x1_0000_0000 + address`.

The result is an `Executable`: read-only bytes, where they start, where the code is, the entry pc,
and a table from call key to pc.

## Calls

```
call  → save r6..r9, r10 and the return pc, move r10 to the next frame, jump
exit  → if a call is active: restore them and jump back
        otherwise: the program ends and returns r0
```

- **`call imm`** looks `imm` up in the syscalls, then in the function table. A key that is
  neither fails with `UnsupportedInstruction`.
- **`callx`** jumps to an address held in a register: `(address - code start) / 8`.
  An address outside the code fails with `CallOutsideTextSegment`.
- **Each call gets a fresh 4 KiB stack frame.** r10 moves forward 8 KiB: the frame plus the
  gap after it. That is why a function can use `[r10-8]` without clobbering its caller.
- **At most 64 frames.** The entrypoint counts as one, so 63 nested calls are fine and
  the 64th fails with `CallDepthExceeded`.

## Surprises

- Keys for internal functions are hashes of their **pc**, not their name. Only `entrypoint`
  is hashed by name.
- `callx` with an address in the middle of an instruction rounds down to its start.
- Loading does not run the verifier. Call `Executable::verify()` yourself.

## How we know it is right

- `tests/elf.rs` loads every v0 test ELF from Anza's repo with both loaders and compares the
  read-only bytes, addresses, code, entry pc and function table, then runs them on both VMs.
- It also corrupts three small ELFs at every byte, and cuts them off at every length: about
  24,000 broken files. Both loaders must accept or reject each one with the same error.
- `tests/calls.rs` runs nested calls, recursive factorial, the depth limit and `callx`
  through both VMs.
