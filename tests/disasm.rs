//! Checks our disassembler output against `solana-sbpf`.

use camshaft::{disasm, insn::decode_program, version::Version};
use solana_sbpf::{
    disassembler::disassemble_instruction,
    ebpf,
    elf::Executable,
    program::{BuiltinProgram, FunctionRegistry, SBPFVersion},
    static_analysis::{Analysis, CfgNode},
    test_utils::TestContextObject,
};
use std::{collections::BTreeMap, sync::Arc};

fn slot(opc: u8, regs: u8, off: i16, imm: i32) -> [u8; 8] {
    let [o0, o1] = off.to_le_bytes();
    let [i0, i1, i2, i3] = imm.to_le_bytes();
    [opc, regs, o0, o1, i0, i1, i2, i3]
}

/// All 256 opcode bytes, each with many register, offset and imm values.
#[test]
fn every_opcode_formats_like_the_reference() {
    // At pc 3 with labels on 0..8: off -4 hits pc 0, -5 is negative, 4 is past the end.
    const PC: usize = 3;
    const LABELLED: usize = 8;
    let label_of = |target: usize| (target < LABELLED).then(|| format!("lbb_{target}"));
    let cfg_nodes: BTreeMap<usize, CfgNode> = (0..LABELLED)
        .map(|pc| {
            (
                pc,
                CfgNode {
                    label: format!("lbb_{pc}"),
                    ..CfgNode::default()
                },
            )
        })
        .collect();
    let loader = BuiltinProgram::<TestContextObject>::new_mock();
    let registry = FunctionRegistry::default();

    for opc in 0..=255u8 {
        for regs in [0x00, 0x21, 0xfa, 0x9f] {
            for off in [0, 1, -1, -4, -5, 4, i16::MIN, i16::MAX] {
                for imm in [0, 5, -1, 16, 17, 32, 64, 0x101, i32::MIN, i32::MAX] {
                    let mut bytes = slot(opc, regs, off, imm).to_vec();
                    if opc == ebpf::LD_DW_IMM {
                        bytes.extend(slot(0xff, 0xff, -1, imm.rotate_left(7)));
                    }

                    let mut reference = ebpf::get_insn_unchecked(&bytes, 0);
                    if opc == ebpf::LD_DW_IMM {
                        ebpf::augment_lddw_unchecked(&bytes, &mut reference);
                    }
                    reference.ptr = PC;

                    for (version, sbpf_version) in [
                        (Version::V0, SBPFVersion::V0),
                        (Version::V3, SBPFVersion::V3),
                    ] {
                        let [(0, ours)] = decode_program(&bytes, version).unwrap()[..] else {
                            panic!("{bytes:02x?} did not decode to one instruction");
                        };
                        let ours = disasm::format_insn(&ours, PC, label_of);
                        let expected = disassemble_instruction(
                            &reference,
                            PC,
                            &cfg_nodes,
                            &registry,
                            &loader,
                            sbpf_version,
                        );
                        assert_eq!(ours, expected, "{version:?} bytes {bytes:02x?}");
                    }
                }
            }
        }
    }
}

fn reference_listing(program: &[u8]) -> String {
    let loader = Arc::new(BuiltinProgram::<TestContextObject>::new_mock());
    let executable = Executable::from_text_bytes(
        program,
        loader,
        SBPFVersion::V0,
        FunctionRegistry::default(),
    )
    .unwrap();
    let analysis = Analysis::from_executable(&executable).unwrap();
    let mut out = Vec::new();
    analysis.disassemble(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

fn assert_listing_matches(program: &[u8]) {
    assert_eq!(
        disasm::disassemble(program, Version::V0).unwrap(),
        reference_listing(program)
    );
}

#[test]
fn straight_line_listing_matches() {
    // Every non-jump opcode, then lddw and exit.
    let mut program = Vec::new();
    for opc in 0..=255u8 {
        let branches = opc & 0x07 == 0x05 && !matches!(opc, ebpf::CALL_IMM | ebpf::CALL_REG);
        if opc != ebpf::LD_DW_IMM && !branches {
            program.extend(slot(opc, 0x21, -8, 16));
        }
    }
    program.extend(slot(ebpf::LD_DW_IMM, 0x01, 0, -1));
    program.extend(slot(0, 0, 0, 0x1234));
    program.extend(slot(ebpf::EXIT, 0, 0, 0));
    assert_listing_matches(&program);
}

#[test]
fn forward_jump_listing_matches() {
    // Every jump goes to `ja +0`, which goes to exit.
    let jumps = [
        ebpf::JEQ64_IMM,
        ebpf::JEQ64_REG,
        ebpf::JGT64_IMM,
        ebpf::JGT64_REG,
        ebpf::JGE64_IMM,
        ebpf::JGE64_REG,
        ebpf::JLT64_IMM,
        ebpf::JLT64_REG,
        ebpf::JLE64_IMM,
        ebpf::JLE64_REG,
        ebpf::JSET64_IMM,
        ebpf::JSET64_REG,
        ebpf::JNE64_IMM,
        ebpf::JNE64_REG,
        ebpf::JSGT64_IMM,
        ebpf::JSGT64_REG,
        ebpf::JSGE64_IMM,
        ebpf::JSGE64_REG,
        ebpf::JSLT64_IMM,
        ebpf::JSLT64_REG,
        ebpf::JSLE64_IMM,
        ebpf::JSLE64_REG,
    ];
    let ja = jumps.len();
    let mut program = Vec::new();
    for (pc, opc) in jumps.into_iter().enumerate() {
        let off = i16::try_from(ja - pc - 1).unwrap();
        program.extend(slot(opc, 0x21, off, -7));
    }
    program.extend(slot(ebpf::JA, 0, 0, 0));
    program.extend(slot(ebpf::EXIT, 0, 0, 0));
    assert_listing_matches(&program);
}

#[test]
fn invalid_and_backward_targets_match() {
    let program = [
        slot(ebpf::JEQ64_IMM, 0x01, 2, 0),  // 0: -> 3
        slot(ebpf::LD_DW_IMM, 0x02, 0, -1), // 1: lddw, slots 1 and 2
        slot(0, 0, 0, -1),
        slot(ebpf::JNE64_REG, 0x21, -2, 0), // 3: -> 2, the second half of the lddw
        slot(ebpf::JLT64_IMM, 0x01, 16, 7), // 4: -> 21, past the end
        slot(ebpf::JA, 0, -6, 0),           // 5: -> 0, the entrypoint
    ]
    .concat();
    assert_listing_matches(&program);
}
