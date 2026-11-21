# 08: sBPF v3

camshaft runs v3 programs as well as v0. Build one with:

```
cargo build-sbf --arch v3
```

An ELF says its version in the `e_flags` header field: 0 is v0, 3 is v3. camshaft rejects
1, 2 and 4. v1 and v2 never reached mainnet, and v4 is not out yet. Agave 4.4, which runs
most of mainnet, accepts v0 to v3.

## What v3 changes

v3 is three SIMDs, and stack frames without gaps:

| change | v0 | v3 |
|---|---|---|
| syscalls (SIMD-0178) | `call key`: a syscall if the key names one, else a function | `call` with src 0 is a syscall by key; with src 1 it calls `pc + 1 + imm` |
| ELF file (SIMD-0189) | sections, symbols and relocations, fixed up at load time | two program headers in a fixed order, nothing to fix up |
| read-only data (SIMD-0189) | in region 1, next to the code | at address `0x0`; the code is not readable as data |
| jumps (SIMD-0377) | 64-bit compares only | also 22 `jeq32`-style jumps that compare the low 32 bits |
| `callx` (SIMD-0377) | register number in imm | register number in dst |
| stack frames | 4 KiB frame, then a 4 KiB unmapped gap; r10 moves 8 KiB per call | frames back to back; r10 moves 4 KiB per call |

Some write-ups list dynamic stack frames (SIMD-0166) and explicit sign extension as v3.
In Anza's VM those belong to v1 and v2 only, so camshaft follows the reference and leaves
them out.

## Memory map

| address | v0 | v3 |
|---|---|---|
| `0x0_0000_0000` | nothing | read-only data |
| `0x1_0000_0000` | code and read-only data | nothing readable (the code runs from here) |
| `0x2_0000_0000` | stack, with gaps | stack, no gaps |
| `0x3_0000_0000` | heap | heap |
| `0x4_0000_0000` | input | input |

An access violation in v3's read-only data reports the region as `allocated`, because the
reference names regions by their address and region 0 has no name.

## Loading a v3 ELF (`src/elf/v3.rs`)

The file is just: header, program headers, read-only data (optional), code. The loader checks:

- The header: ELF magic, 64-bit, little-endian, `EM_BPF`, program headers right after
  the header. Anything off fails with `invalid file header`.
- Each program header: loadable, the right flags and address (read-only at `0x0`, code at
  `0x1_0000_0000`), placed right after the one before it, 8-byte aligned, inside the file.
  Anything off fails with `invalid program header`.
- The entry: a whole instruction inside the code.

There are no relocations, so the loader just copies the two segments out.

## How the code is split

- `src/version.rs`: the `Version` enum.
- The decoder does most of the work. It reads `callx`'s register from the right field,
  only knows the 32-bit jumps in v3, and turns each `call` into a `CallTarget`: a v0 key, a
  v3 syscall, a v3 relative call, or an invalid v3 call. After decoding, only the memory map
  and the stack frame step still look at the version.
- `src/elf/mod.rs` reads `e_flags` and hands the file to `v0.rs` or `v3.rs`.

## How we know it is right

Every differential test runs as v3 too (see [07](07-differential-testing.md)):

- every opcode with many field values, through both verifiers and both disassemblers
- the hand-written programs, run as v0 and as v3
- all 186 of Anza's assembly programs, assembled as v3
- Anza's 13 v3 test ELFs, loaded and run, two of them also corrupted at every byte
- random v3 programs: 32-bit jumps, relative calls, `call` with every src, `callx` by dst
- `counter`, `tour` and `hello` built with `--arch v3` and compared against Agave 4.4.1:
  same result, logs, compute units, accounts and return data as Agave gives
