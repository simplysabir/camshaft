# Test programs

The `*_sbpfv0.so` and `*_sbpfv3.so` files are SBPF test ELFs copied from
[anza-xyz/sbpf](https://github.com/anza-xyz/sbpf) (`tests/elfs/`), licensed Apache-2.0 or MIT.
Their sources live in that repo next to the same names (the v3 ones without the suffix).

`counter`, `tour` and `hello` are our own programs; their sources are in the folders of the
same names. Each `.so` is what `cargo build-sbf` puts in `target/deploy` (stripped), and
`_v3.so` is the same program built with `--arch v3`. `tour_unstripped.so` and
`tour_v3_unstripped.so` keep their symbol tables, for the profiler's function names.
