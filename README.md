# camshaft

A Solana virtual machine written from scratch in Rust. It runs sBPF bytecode, the code
that `cargo build-sbf` produces, and shows what happens at each step.

## Status

- [x] Milestone 0: decode and disassemble bytecode
- [x] Milestone 1: run register-only programs
- [x] Milestone 2: memory (loads, stores, access checks)
- [x] Milestone 3: verifier
- [ ] Milestone 4: ELF loader and function calls

## Usage

```
cargo run -- disasm program.bin
```

`program.bin` is raw bytecode for now. ELF (`.so`) support comes later.

## Docs

- [00: Instructions](docs/00-instructions.md)
- [01: Interpreter](docs/01-interpreter.md)
- [02: Memory](docs/02-memory.md)
- [03: Verifier](docs/03-verifier.md)
