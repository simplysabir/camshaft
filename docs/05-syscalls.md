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

## Not done yet

- **Compute costs.** In Agave, `sol_log_` costs at least 100 units. Costs come with the
  compute meter in milestone 6; until then `sol_log_compute_units_` reports the
  instructions left.
- **Program input.** Programs that read their accounts fail for now, because the input
  region is empty. Milestone 6 fills it.

## Unknown syscalls

A call to a syscall camshaft does not have fails with `UnsupportedInstruction`, the same as
the reference VM. The ELF still has the name, so `camshaft run` prints it:

```
camshaft: unknown syscall `log` at pc 3
```

## How we know it is right

These follow Agave v3.0.8's source (`syscalls/src`), the version `cargo build-sbf` 3.0.8
targets. Anza's `sbpf` VM has none of Agave's syscalls, so there is no oracle to compare
against yet. Instead:

- `tests/syscalls.rs` tests each syscall and each error case against Agave's code: bad UTF-8,
  unmapped pointers, zero lengths, overlap, alignment, heap exhaustion.
- `tests/programs/hello.so` is a real program, built from `tests/programs/hello/`.
- Comparing against a real Agave runtime (like litesvm or mollusk) comes later, along
  with compute units.
