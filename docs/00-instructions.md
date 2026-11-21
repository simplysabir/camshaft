# 00: Instructions

camshaft can now read sBPF bytecode and print it as assembly.

## One instruction = 8 bytes

```
byte 0     opcode     what to do
byte 1     registers  low 4 bits = dst, high 4 bits = src
bytes 2-3  offset     16-bit signed number
bytes 4-7  imm        32-bit signed number
```

Example: `b7 01 00 00 05 00 00 00` is `mov64 r1, 5`.

`lddw` is the one exception. It loads a 64-bit number, which needs two slots (16 bytes).
The first slot holds the low 32 bits and the second slot holds the high 32 bits.

The **pc** counts slots, not instructions. After an `lddw` at pc 5, the next instruction is at pc 7.

## Reading the opcode

The lowest 3 bits say what kind of instruction it is:

| bits | kind | examples |
|---|---|---|
| 0 | 64-bit constant | `lddw` |
| 1 | load from memory | `ldxw r1, [r2+0x8]` |
| 2 | store a number | `stw [r1+0x0], 5` |
| 3 | store a register | `stxw [r1+0x0], r2` |
| 4 | 32-bit math | `add32 r1, 5` |
| 5 | jumps, calls, exit | `jeq r1, 5, lbb_7` |
| 6 | unused in v0 | |
| 7 | 64-bit math | `add64 r1, r2` |

For math and jumps, bit 3 picks the second operand: `0` means the `imm` number, `1` means
the `src` register. So `add64 r1, 5` is `0x07` and `add64 r1, r2` is `0x0f`.

v0 has 91 valid opcodes. Anything else prints as `unknown opcode=0x..`.

## Surprises

- `callx` (opcode `0x8d`) reads its register number from `imm`, not from `src` or `dst`.
- `le` and `be` (byte swaps) read the size (16, 32 or 64) from `imm`.
- A jump lands at `pc + offset + 1`. If that isn't the start of an instruction, it prints as `[invalid]`.

## Code

- `src/insn.rs` turns bytes into an `Insn` value. `decode_at(slots, pc)` decodes one
  instruction. `decode_program` decodes all of them.
- `src/disasm.rs` turns an `Insn` into text.
- Unknown opcodes are not errors here. The verifier (milestone 3) rejects them.

## How we know it is right

`tests/disasm.rs` runs every one of the 256 opcode bytes, with many different field
values, through both camshaft and Anza's `solana-sbpf`, and checks that the text is the same.

## Known differences

- `call` always prints `syscall <number>`. Names come once we load ELF files (milestone 4) and syscalls (milestone 5).
- Labels: we name jump targets `lbb_N`. Anza runs extra analysis and sometimes names
  them `function_N`, for example for unreachable code. The instructions themselves always match.

## Setup note

`solana-sbpf` 0.28 needs Rust 1.95, so `rust-toolchain.toml` pins it.
