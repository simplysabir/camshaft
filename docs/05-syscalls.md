# 05: Syscalls

Programs can now call into the runtime: log messages, copy memory, allocate. camshaft runs
a hello-world built with `cargo build-sbf`:

```
$ camshaft run tests/programs/hello.so
Program log: Hello, world!
Program returned 0 after 5 instructions
```

## How a program calls a syscall

There is no `syscall` instruction in v0. A program does `call imm`, where `imm` is the
murmur3 hash of the syscall's name. `sol_log_` is always `0x207559bd`.

```
r1..r5  arguments
call    0x207559bd     ; sol_log_
r0      result
```

When the VM sees `call imm`, it checks the syscall table first, then the program's own
functions. Function keys may never equal a syscall key; the loader rejects that.

## The syscalls (`src/syscalls/`)

| name | what it does |
|---|---|
| `sol_log_(ptr, len)` | logs `Program log: <text>`. The text must be UTF-8 |
| `sol_log_64_(a, b, c, d, e)` | logs five numbers in hex |
| `sol_log_pubkey(ptr)` | logs 32 bytes as a base58 public key |
| `sol_log_compute_units_()` | logs `Program consumption: N units remaining` |
| `abort()` | stops with an error |
| `sol_panic_(file, len, line, col)` | stops with an error naming the file and line |
| `sol_memcpy_(dst, src, n)` | copies `n` bytes. Overlapping ranges are an error |
| `sol_memmove_(dst, src, n)` | copies `n` bytes. Overlap is fine |
| `sol_memset_(dst, c, n)` | fills `n` bytes with the low byte of `c` |
| `sol_memcmp_(a, b, n, result)` | writes `a[i] - b[i]` for the first differing byte, or 0 |
| `sol_alloc_free_(size, free)` | hands out heap memory, 8-byte aligned. Never frees |

## Rules every syscall follows

- **Every pointer goes through the memory map.** A bad pointer gives the same access
  violation a load or store would, wrapped in a syscall error.
- **A length of 0 is always fine**, even with a garbage pointer.
- **Writes are checked before reads.** If both pointers of a copy are bad, the error
  names the destination.
- **The log is capped at 10,000 bytes.** The first line over the cap becomes `Log truncated`.

## Costs

Each syscall costs compute units, charged before it does any work. See
[06](06-input-and-compute.md) for the table.

## Unknown syscalls

A call to a syscall camshaft does not have fails with `UnsupportedInstruction`, the same as
the reference VM. The ELF still has the name, so `camshaft run` prints it:

```
camshaft: unknown syscall `log` at pc 3
```

## How we know it is right

These follow Agave 4.4.1, the runtime on most of mainnet. `tests/runtime.rs` runs real
programs through camshaft and through Agave itself (via mollusk) and compares every log
line, result and compute unit count. `tests/syscalls.rs` adds small hand-built programs for
each error case, and `tests/programs/hello.so` is a real hello-world.
