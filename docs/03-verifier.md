# 03: Verifier

`verify(program)` checks a program **before** it runs and rejects anything that could
confuse the VM. Solana runs these checks when a program is deployed, so a program that
fails here can never run on chain.

## What it checks

For every instruction, in order:

| check | error |
|---|---|
| the opcode exists in v0 | `UnknownOpCode(opc, pc)` |
| `lddw` is not the last slot | `LDDWCannotBeLast` |
| the second slot of `lddw` has opcode 0 | `IncompleteLDDW(pc)` |
| no `div` / `mod` by the number 0 | `DivisionByZero(pc)` |
| shift amounts are 0..32 (32-bit) or 0..64 (64-bit) | `ShiftWithOverflow(amount, bits, pc)` |
| `le` / `be` size is 16, 32 or 64 | `UnsupportedLEBEArgument(pc)` |
| a jump lands inside the program | `JumpOutOfCode(target, pc)` |
| a jump does not land in the middle of an `lddw` | `JumpToMiddleOfLDDW(target, pc)` |
| `callx` uses r0 to r9 | `InvalidRegister(pc)` |
| `src` is r0 to r10 | `InvalidSourceRegister(pc)` |
| `dst` is r0 to r9, or r10 only as a store address | `CannotWriteR10(pc)` / `InvalidDestinationRegister(pc)` |

Plus two checks on the whole program: it must not be empty (`NoProgram`), and its length
must be a multiple of 8 (`ProgramLengthNotMultiple`).

## Surprises

- **Register fields are checked on every instruction, even ones that don't use them.**
  An `exit` with 10 in its unused `dst` field is rejected with `CannotWriteR10`.
- **r10 is read-only.** It can be the base of a store (`stxdw [r10-8], r1`) but never a destination.
- **It does not check that the program ever exits.** Running off the end is a runtime error.
- **Errors in an `lddw`'s registers report the pc of its second slot.** The reference does this, so we do too.

## What the verifier buys the interpreter

Once a program passes, the interpreter can trust it: registers are r0 to r10, jumps land on
real instructions, and there is no division by a zero immediate. Loading does not verify by itself, just like the reference. Call
`Executable::verify()` after loading.

## How we know it is right

`tests/verifier.rs`:
- Runs about 740,000 small programs through both camshaft and Anza's verifier: every opcode
  byte with many register, offset and immediate values. The results must be identical.
- Repeats every v0 case from Anza's own verifier tests, with the exact error they expect.
