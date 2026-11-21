# 01: Interpreter

camshaft can now run a program, as long as it only uses registers (no memory or calls yet).

## The loop

```
loop:
    if we ran too many instructions -> stop (ExceededMaxInstructions)
    if pc is past the end           -> stop (ExecutionOverrun)
    decode the instruction at pc
    do it
    move pc to the next instruction (or the jump target)
```

`exit` stops the loop and returns r0.

## Starting registers

| register | start value | why |
|---|---|---|
| r1 | `0x4_0000_0000` | where the input will live (milestone 6) |
| r10 | `0x2_0000_1000` | top of the first 4 KiB stack frame |
| others | 0 | |

## Rules that are easy to get wrong

- **Numbers wrap.** `u64::MAX + 1` is 0. Nothing ever overflows into an error.
- **32-bit math only looks at the low 32 bits of each input.** The result then fills 64 bits:
  - `add32`, `sub32`, `mul32` copy the sign bit up (sign-extend). `0x7fffffff + 1` becomes `0xffffffff80000000`.
  - Every other 32-bit op fills the top half with zeros.
  - The plan said all 32-bit ops zero-extend. That is wrong for v0; the reference wins.
- **An `imm` in a 64-bit op is sign-extended first.** `div64 r1, -1` divides by `0xffffffffffffffff`.
- **Shifts wrap the amount**: mod 32 for 32-bit, mod 64 for 64-bit. `lsh64 r1, 65` shifts by 1.
- **Division by a zero register** is a `DivideByZero` error. `div32` only checks the low 32 bits, so `1 << 32` counts as zero.
- **`le16` / `le32`** just cut the value down. **`be`** also reverses the bytes. Any other size is an `InvalidInstruction` error.
- **Signed jumps** (`jsgt` and friends) treat both sides as signed. All others compare unsigned.
- **A jump lands at `pc + 1 + offset`.** Landing outside the program is not an error until we try to run from there.

## Not done yet

- Loads and stores (milestone 2), `call` and `callx` (milestone 4). They return `UnsupportedInstruction` for now.
- No verifier yet (milestone 3). A program using r11 or higher will crash the interpreter.

## How we know it is right

`tests/interp.rs` builds thousands of small programs with Anza's assembler and runs each one
through both camshaft and Anza's interpreter. Both must agree on the result or error, every
register, the final pc and the instruction count. The programs cover every math op, every jump,
and edge values like 0, `i64::MIN` and `u64::MAX`.
