# camshaft

A Solana virtual machine written from scratch in Rust. It runs sBPF bytecode, the code
that `cargo build-sbf` produces, and shows what happens at each step.

## Status

- [x] Milestone 0: decode and disassemble bytecode
- [x] Milestone 1: run register-only programs
- [x] Milestone 2: memory (loads, stores, access checks)
- [x] Milestone 3: verifier
- [x] Milestone 4: ELF loader and function calls
- [x] Milestone 5: syscalls (logging, memory, allocation)
- [x] Milestone 6: program input, compute units, more syscalls
- [x] Milestone 7: differential testing at scale

## Usage

```
cargo run -- run program.so --input fixture.json   # run one instruction
cargo run -- run program.so                          # run with no accounts
cargo run -- disasm program.bin                      # print raw bytecode as assembly
```

`run` takes a v0 ELF built by `cargo build-sbf` and an optional JSON fixture
(see [docs/06](docs/06-input-and-compute.md)). `disasm` takes raw bytecode for now.

## Docs

- [00: Instructions](docs/00-instructions.md)
- [01: Interpreter](docs/01-interpreter.md)
- [02: Memory](docs/02-memory.md)
- [03: Verifier](docs/03-verifier.md)
- [04: ELF loading and function calls](docs/04-elf-and-calls.md)
- [05: Syscalls](docs/05-syscalls.md)
- [06: Program input and compute units](docs/06-input-and-compute.md)
- [07: Differential testing](docs/07-differential-testing.md)
