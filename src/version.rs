//! The sBPF versions camshaft runs. An ELF names its version in the `e_flags` header field.
//! Reference: `SBPFVersion` in anza-xyz/sbpf `program.rs`.

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Version {
    /// The original format, still what `cargo build-sbf` makes by default.
    #[default]
    V0,
    /// SIMD-0178 static syscalls, SIMD-0189 strict ELF headers, SIMD-0377 jmp32 and
    /// `callx` by dst. v1 and v2 never reached mainnet, so camshaft skips them.
    V3,
}

impl Version {
    /// v0 leaves an unmapped gap after each stack frame. v3 does not.
    pub fn stack_frame_gaps(self) -> bool {
        self == Version::V0
    }
}
