//! Checks our verifier against the reference `RequisiteVerifier` for v0.

use camshaft::verifier::verify;
use camshaft::version::Version;
use solana_sbpf::{
    assembler::assemble,
    ebpf,
    program::{BuiltinProgram, FunctionRegistry, SBPFVersion},
    test_utils::TestContextObject,
    verifier::{RequisiteVerifier, Verifier},
    vm::Config,
};
use std::sync::Arc;

fn slot(opc: u8, dst: u8, src: u8, off: i16, imm: i32) -> [u8; 8] {
    let [o0, o1] = off.to_le_bytes();
    let [i0, i1, i2, i3] = imm.to_le_bytes();
    [opc, src << 4 | dst, o0, o1, i0, i1, i2, i3]
}

const EXIT: [u8; 8] = [0x95, 0, 0, 0, 0, 0, 0, 0];

/// Both verifiers' results, formatted the same way.
fn both(program: &[u8], version: Version) -> (String, String) {
    let sbpf_version = match version {
        Version::V0 => SBPFVersion::V0,
        Version::V3 => SBPFVersion::V3,
    };
    let reference = RequisiteVerifier::verify(
        program,
        sbpf_version,
        &Config::default(),
        &FunctionRegistry::<usize>::default(),
    );
    (
        format!("{:?}", verify(program, version)),
        format!("{reference:?}"),
    )
}

#[test]
fn every_opcode_verifies_like_the_reference() {
    let regs = [0, 9, 10, 11, 15];
    // At pc 1 in a 4-slot program: -2 hits pc 0, -3 goes negative, 0 hits the second slot,
    // 1 hits the exit, 2 is past the end.
    let offs = [-2, -3, 0, 1, 2, i16::MIN, i16::MAX];
    let imms = [0, 1, -1, 3, 9, 10, 16, 31, 32, 63, 64];
    // What follows the instruction under test: an lddw second half, a non-zero opcode, nothing.
    let seconds = [Some(slot(0, 3, 4, 7, 0x1234)), Some(EXIT), None];
    for opc in 0..=255u8 {
        for dst in regs {
            for src in regs {
                for off in offs {
                    for imm in imms {
                        for second in seconds {
                            let mut program =
                                vec![slot(0xb7, 0, 0, 0, 0), slot(opc, dst, src, off, imm)];
                            program.extend(second.map(|s| [s, EXIT]).into_iter().flatten());
                            let program = program.as_flattened();
                            for version in [Version::V0, Version::V3] {
                                let (ours, reference) = both(program, version);
                                assert_eq!(ours, reference, "{version:?} {program:02x?}");
                            }
                        }
                    }
                }
            }
        }
    }
}

fn asm(src: &str) -> Vec<u8> {
    let config = Config {
        enabled_sbpf_versions: SBPFVersion::V0..=SBPFVersion::V0,
        ..Config::default()
    };
    let executable =
        assemble::<TestContextObject>(src, Arc::new(BuiltinProgram::new_loader(config))).unwrap();
    executable.get_text_bytes().1.to_vec()
}

/// The v0 cases from the reference's `tests/verifier.rs`. Cases that start with
/// `add64 r10, 0` there are V1+ only, so that line is dropped and the pc shifts by one.
#[test]
fn reference_test_suite_cases() {
    let lddw_then_call = [
        slot(ebpf::LD_DW_IMM, 0, 0, 0, 0x55667788),
        slot(ebpf::CALL_IMM, 0, 0, 0, 0),
    ];
    let be3 = [
        slot(ebpf::BE, 1, 0, 0, 3),
        slot(ebpf::MOV64_IMM, 1, 0, 0, 0),
        EXIT,
    ];
    let add_r10 = [slot(ebpf::ADD64_IMM, 10, 0, 0, 0), EXIT];
    let cases: Vec<(Vec<u8>, &str)> = vec![
        (asm("mov32 r0, 0xBEE\nexit"), "Ok(())"),
        (vec![], "Err(NoProgram)"),
        (
            asm("mov32 r0, 1\ndiv32 r0, 0\nexit"),
            "Err(DivisionByZero(1))",
        ),
        (
            be3.as_flattened().to_vec(),
            "Err(UnsupportedLEBEArgument(0))",
        ),
        (
            lddw_then_call.as_flattened().to_vec(),
            "Err(IncompleteLDDW(0))",
        ),
        (lddw_then_call[0].to_vec(), "Err(LDDWCannotBeLast)"),
        (
            asm("mov r11, 1\nexit"),
            "Err(InvalidDestinationRegister(0))",
        ),
        (asm("mov r0, r11\nexit"), "Err(InvalidSourceRegister(0))"),
        (
            asm("ja +1\nlddw r0, 0x1122334455667788\nexit"),
            "Err(JumpToMiddleOfLDDW(2, 0))",
        ),
        (asm("call 1\nlddw r0, 0x1122334455667788\nexit"), "Ok(())"),
        (asm("callx r10\nexit"), "Err(InvalidRegister(0))"),
        (asm("mov64 r0, 0\nja +2\nexit"), "Err(JumpOutOfCode(4, 1))"),
        (
            asm("mov64 r0, 0\nja -3\nexit"),
            "Err(JumpOutOfCode(18446744073709551615, 1))",
        ),
        // Raw bytes: in v0 mode the assembler rewrites `add64 r10, 0` into `or64 r0, 0`.
        (add_r10.as_flattened().to_vec(), "Err(CannotWriteR10(0))"),
        (
            asm("mov64 r0, 0\nmov r10, 1\nexit"),
            "Err(CannotWriteR10(1))",
        ),
    ];
    let shifts = [
        ("lsh32 r0, 16", "Ok(())"),
        ("lsh32 r0, 32", "Err(ShiftWithOverflow(32, 32, 1))"),
        ("lsh32 r0, 64", "Err(ShiftWithOverflow(64, 32, 1))"),
        ("rsh32 r0, 16", "Ok(())"),
        ("rsh32 r0, 32", "Err(ShiftWithOverflow(32, 32, 1))"),
        ("rsh32 r0, 64", "Err(ShiftWithOverflow(64, 32, 1))"),
        ("arsh32 r0, 16", "Ok(())"),
        ("arsh32 r0, 32", "Err(ShiftWithOverflow(32, 32, 1))"),
        ("arsh32 r0, 64", "Err(ShiftWithOverflow(64, 32, 1))"),
        ("lsh64 r0, 32", "Ok(())"),
        ("lsh64 r0, 64", "Err(ShiftWithOverflow(64, 64, 1))"),
        ("rsh64 r0, 32", "Ok(())"),
        ("rsh64 r0, 64", "Err(ShiftWithOverflow(64, 64, 1))"),
        ("arsh64 r0, 32", "Ok(())"),
        ("arsh64 r0, 64", "Err(ShiftWithOverflow(64, 64, 1))"),
    ];
    let shifts =
        shifts.map(|(insn, expected)| (asm(&format!("mov64 r0, 0\n{insn}\nexit")), expected));
    // PQR opcodes are V2 only.
    let sdivs = [
        ebpf::SDIV32_IMM,
        ebpf::SDIV32_REG,
        ebpf::SDIV64_IMM,
        ebpf::SDIV64_REG,
    ]
    .map(|opc| {
        let program = [
            slot(ebpf::MOV64_IMM, 0, 0, 0, 0),
            slot(opc, 0, 1, 0, 2),
            EXIT,
        ];
        (
            program.as_flattened().to_vec(),
            format!("Err(UnknownOpCode({opc}, 1))"),
        )
    });

    let all = cases
        .into_iter()
        .map(|(p, e)| (p, e.to_string()))
        .chain(shifts.into_iter().map(|(p, e)| (p, e.to_string())))
        .chain(sdivs);
    for (program, expected) in all {
        let (ours, reference) = both(&program, Version::V0);
        assert_eq!(reference, expected, "reference, program {program:02x?}");
        assert_eq!(ours, expected, "ours, program {program:02x?}");
    }
}
