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
- [ ] Milestone 6: program input and compute units

## Usage

```
cargo run -- run program.so        # run a program and print its logs
cargo run -- disasm program.bin    # print raw bytecode as assembly
```

`run` takes a v0 ELF built by `cargo build-sbf`. `disasm` takes raw bytecode for now.

## Docs

- [00: Instructions](docs/00-instructions.md)
- [01: Interpreter](docs/01-interpreter.md)
- [02: Memory](docs/02-memory.md)
- [03: Verifier](docs/03-verifier.md)
- [04: ELF loading and function calls](docs/04-elf-and-calls.md)
- [05: Syscalls](docs/05-syscalls.md)
