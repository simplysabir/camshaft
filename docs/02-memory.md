# 02: Memory

Every load and store goes through `src/memory.rs`, which checks the address before touching
any bytes.

## The address map

The top 32 bits of an address pick the region. The low 32 bits are the offset inside it.

| region | starts at | can write? | size |
|---|---|---|---|
| program | `0x1_0000_0000` | no | the bytecode |
| stack | `0x2_0000_0000` | yes | 64 frames × 4 KiB |
| heap | `0x3_0000_0000` | yes | 32 KiB |
| input | `0x4_0000_0000` | yes | whatever we pass in |

Anything else (like address 0) is unmapped.

An access is allowed only if **all** its bytes are inside the region. A store also needs the
region to be writable.

## The stack has gaps

Each 4 KiB stack frame is followed by a 4 KiB hole that nothing can touch:

```
virtual:  frame 0 | gap | frame 1 | gap | frame 2 | ...
real:     frame 0 | frame 1 | frame 2 | ...
```

r10 starts at the end of frame 0, so `[r10-8]` is fine and `[r10+0]` hits the gap. A program
that writes past its frame crashes instead of silently overwriting the next one.

One surprise copied from the reference: only the **start** of an access is checked against
the gaps. An 8-byte read at `[r10-4]` starts inside frame 0, so it is allowed and its last
4 bytes come from frame 1.

## Loads and stores

- `ldxb`, `ldxh`, `ldxw`, `ldxdw` read 1, 2, 4 or 8 bytes, little-endian, and fill the rest of the register with zeros.
- `stb [r1+0], 5` and friends store an immediate. `stdw` stores the immediate sign-extended to 64 bits.
- `stxb [r1+0], r2` and friends store the low bytes of a register.
- The address is `register + offset`, wrapping around on overflow.

## Errors

| error | when |
|---|---|
| `StackAccessViolation(type, addr, len, frame)` | the address is within about one frame of the stack (frame -1 to 64) |
| `AccessViolation(type, addr, len, region)` | anything else. `region` is `program`, `stack`, `heap`, `input` or `unallocated` |

After an error, `vm.pc` points at the instruction that failed.

## How we know it is right

`tests/interp.rs` runs each program on camshaft and on Anza's VM, then compares the result
or error, the registers, pc, instruction count, and every byte of the stack, heap and input.
It covers every store and load width in every writable region, and each kind of violation
listed above.
