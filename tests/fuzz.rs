//! Random programs run on both camshaft and `solana-sbpf`.
//!
//! A normal `cargo test` tries a few hundred programs. For a long run:
//! `CAMSHAFT_FUZZ_SECS=3600 cargo test --release --test fuzz -- --nocapture`

mod common;

use camshaft::memory::{HEAP_START, INPUT_START, PROGRAM_START, STACK_START};
use camshaft::version::Version;
use common::{
    Reference, VERSIONS, input, loader, ours_from_reference_text, run_both, sbpf_version,
    verify_both,
};
use proptest::prelude::*;
use proptest::sample::Index;
use proptest::test_runner::{Config, RngAlgorithm, TestRng, TestRunner};
use solana_sbpf::program::FunctionRegistry;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

/// One instruction before layout. Jumps and calls name their target by instruction index, so
/// they always land on the start of an instruction once offsets are worked out.
#[derive(Debug, Clone)]
enum Op {
    /// Any one-slot instruction that is not a jump or call.
    Plain([u8; 8]),
    Lddw {
        dst: u8,
        value: u64,
    },
    /// `wide: false` makes a v3 conditional jump compare 32 bits. v0 has only 64-bit jumps.
    Jump {
        opc: u8,
        wide: bool,
        dst: u8,
        src: u8,
        imm: i32,
        target: Index,
    },
    /// `call` to one of the functions.
    Call(Index),
    /// Loads a function's address into `reg`, then `callx reg`.
    Callx {
        reg: u8,
        function: Index,
    },
    Exit,
}

impl Op {
    fn slots(&self) -> usize {
        match self {
            Op::Lddw { .. } => 2,
            Op::Callx { .. } => 3,
            _ => 1,
        }
    }
}

/// A main body followed by functions. Every part ends with `exit`, except sometimes the last.
#[derive(Debug, Clone)]
struct Program {
    version: Version,
    parts: Vec<Vec<Op>>,
}

fn slot(opc: u8, dst: u8, src: u8, off: i16, imm: i32) -> [u8; 8] {
    let [o0, o1] = off.to_le_bytes();
    let [i0, i1, i2, i3] = imm.to_le_bytes();
    [opc, src << 4 | dst, o0, o1, i0, i1, i2, i3]
}

impl Program {
    /// Lays the program out as bytecode, and returns the pc where each function starts.
    fn layout(&self) -> (Vec<u8>, Vec<usize>) {
        let (mut pcs, mut functions, mut pc) = (Vec::new(), Vec::new(), 0);
        for (i, part) in self.parts.iter().enumerate() {
            if i > 0 {
                functions.push(pc);
            }
            for op in part {
                pcs.push(pc);
                pc += op.slots();
            }
        }
        let function_pc = |f: Index| functions[f.index(functions.len())];

        let mut out = Vec::new();
        for (op, &pc) in self.parts.iter().flatten().zip(&pcs) {
            match *op {
                Op::Plain(s) => out.extend(s),
                Op::Lddw { dst, value } => {
                    out.extend(slot(0x18, dst, 0, 0, value as i32));
                    out.extend(slot(0, 0, 0, 0, (value >> 32) as i32));
                }
                Op::Jump {
                    opc,
                    wide,
                    dst,
                    src,
                    imm,
                    target,
                } => {
                    let off = *target.get(&pcs) as i64 - pc as i64 - 1;
                    // Class 0x05 is 64-bit jumps, 0x06 the v3 32-bit ones. `ja` has no 32-bit form.
                    let jmp32 = self.version == Version::V3 && !wide && opc != 0x05;
                    let opc = if jmp32 { opc + 1 } else { opc };
                    out.extend(slot(opc, dst, src, off as i16, imm));
                }
                Op::Call(f) => out.extend(match self.version {
                    // v0 names the function by key, which here is its pc.
                    Version::V0 => slot(0x85, 0, 0, 0, function_pc(f) as i32),
                    // v3 jumps relative, with src 1.
                    Version::V3 => slot(
                        0x85,
                        0,
                        1,
                        0,
                        (function_pc(f) as i64 - pc as i64 - 1) as i32,
                    ),
                }),
                Op::Callx { reg, function } => {
                    let addr = PROGRAM_START + 8 * function_pc(function) as u64;
                    out.extend(slot(0x18, reg, 0, 0, addr as i32));
                    out.extend(slot(0, 0, 0, 0, (addr >> 32) as i32));
                    out.extend(callx(reg));
                }
                Op::Exit => out.extend(slot(0x95, 0, 0, 0, 0)),
            }
        }
        (out, functions)
    }
}

/// v0 reads the register from imm and v3 from dst, so set both.
fn callx(reg: u8) -> [u8; 8] {
    slot(0x8d, reg, 0, 0, reg.into())
}

/// Registers allowed in the dst field: r0 to r9. Only stores may name r10 there.
fn reg() -> impl Strategy<Value = u8> {
    0..10u8
}

/// Registers an instruction may read, r10 included.
fn any_reg() -> impl Strategy<Value = u8> {
    0..11u8
}

fn imm() -> impl Strategy<Value = i32> {
    prop_oneof![
        3 => prop::sample::select(vec![0, 1, -1, 2, 8, 31, 32, 63, 64, i32::MIN, i32::MAX]),
        1 => any::<i32>(),
    ]
}

/// Values near region edges, plus edge and random numbers.
fn value() -> impl Strategy<Value = u64> {
    let bases = vec![
        PROGRAM_START,
        STACK_START,
        HEAP_START,
        INPUT_START,
        0,
        u64::MAX,
        1 << 31,
        1 << 32,
    ];
    prop_oneof![
        3 => (prop::sample::select(bases), -16..4112i64)
            .prop_map(|(base, delta)| base.wrapping_add_signed(delta)),
        1 => any::<u64>(),
    ]
}

/// Offsets that hit inside and just outside a stack frame or the input.
fn off() -> impl Strategy<Value = i16> {
    prop_oneof![
        3 => -64..64i16,
        1 => prop::sample::select(vec![-4096, -4097, -4104, 1016, 1020, 1024, i16::MIN, i16::MAX]),
        1 => any::<i16>(),
    ]
}

/// ALU and memory instructions the v0 verifier accepts.
fn plain() -> impl Strategy<Value = [u8; 8]> {
    // add sub mul div or and lsh rsh neg mod xor mov arsh, for 32 and 64 bits, imm and reg.
    let alu_ops: Vec<u8> = (0..=0xc).map(|op| op << 4).collect();
    let alu = (
        prop::sample::select(alu_ops),
        prop::bool::ANY,
        prop::bool::ANY,
        reg(),
        any_reg(),
        imm(),
    )
        .prop_map(|(op, wide, by_reg, dst, src, imm)| {
            let width = if wide { 64 } else { 32 };
            // neg has no register form.
            let by_reg = by_reg && op != 0x80;
            let opc = op | if wide { 0x07 } else { 0x04 } | if by_reg { 0x08 } else { 0 };
            // The verifier rejects a zero divisor and an out-of-range shift as immediates.
            let imm = match op {
                0x30 | 0x90 if imm == 0 => 1,
                0x60 | 0x70 | 0xc0 => imm.rem_euclid(width),
                _ => imm,
            };
            slot(opc, dst, src, 0, imm)
        });
    let end = (
        prop::sample::select(vec![0xd4u8, 0xdc]),
        reg(),
        prop::sample::select(vec![16, 32, 64]),
    )
        .prop_map(|(opc, dst, imm)| slot(opc, dst, 0, 0, imm));
    // ldx, st and stx for 1, 2, 4 and 8 bytes. Bases lean on r1 (input) and r10 (stack).
    let base = prop_oneof![2 => Just(1u8), 2 => Just(10u8), 1 => any_reg()];
    let size = prop::sample::select(vec![0x10u8, 0x08, 0x00, 0x18]);
    let load = (size.clone(), reg(), base.clone(), off())
        .prop_map(|(size, dst, base, off)| slot(0x61 | size, dst, base, off, 0));
    let store = (size.clone(), base.clone(), off(), imm())
        .prop_map(|(size, base, off, imm)| slot(0x62 | size, base, 0, off, imm));
    let store_reg = (size, base, any_reg(), off())
        .prop_map(|(size, base, src, off)| slot(0x63 | size, base, src, off, 0));
    prop_oneof![6 => alu, 1 => end, 2 => load, 1 => store, 2 => store_reg]
}

fn op() -> impl Strategy<Value = Op> {
    // ja jeq jgt jge jset jne jsgt jsge jlt jle jslt jsle
    let jumps = vec![
        0x05u8, 0x15, 0x25, 0x35, 0x45, 0x55, 0x65, 0x75, 0xa5, 0xb5, 0xc5, 0xd5,
    ];
    let jump = (
        prop::sample::select(jumps),
        prop::bool::ANY,
        prop::bool::ANY,
        reg(),
        any_reg(),
        imm(),
        any::<Index>(),
    )
        .prop_map(|(opc, by_reg, wide, dst, src, imm, target)| Op::Jump {
            opc: if opc == 0x05 {
                opc
            } else {
                opc | (by_reg as u8) << 3
            },
            wide,
            dst,
            src,
            imm,
            target,
        });
    prop_oneof![
        12 => plain().prop_map(Op::Plain),
        1 => (reg(), value()).prop_map(|(dst, value)| Op::Lddw { dst, value }),
        3 => jump,
        1 => any::<Index>().prop_map(Op::Call),
        1 => (reg(), any::<Index>()).prop_map(|(reg, function)| Op::Callx { reg, function }),
        // A call with any src and imm, and a callx to whatever a register holds.
        1 => (any_reg(), imm()).prop_map(|(src, imm)| Op::Plain(slot(0x85, 0, src, 0, imm))),
        1 => reg().prop_map(|reg| Op::Plain(callx(reg))),
        1 => Just(Op::Exit),
    ]
}

fn part() -> impl Strategy<Value = Vec<Op>> {
    prop::collection::vec(op(), 0..24).prop_map(|mut ops| {
        ops.push(Op::Exit);
        ops
    })
}

fn program() -> impl Strategy<Value = Program> {
    // Start with interesting values in r0 and r2 to r9.
    let prefix = prop::collection::vec(value(), 9).prop_map(|values| {
        [0, 2, 3, 4, 5, 6, 7, 8, 9]
            .into_iter()
            .zip(values)
            .map(|(dst, value)| Op::Lddw { dst, value })
            .collect::<Vec<_>>()
    });
    let parts = (
        prop::sample::select(VERSIONS.to_vec()),
        prefix,
        part(),
        prop::collection::vec(part(), 1..4),
        prop::bool::weighted(0.1),
    );
    parts.prop_map(|(version, prefix, main, functions, run_off_the_end)| {
        let mut parts = vec![[prefix, main].concat()];
        parts.extend(functions);
        if run_off_the_end {
            parts.last_mut().unwrap().pop();
        }
        Program { version, parts }
    })
}

/// Runs one program on both VMs; `run_both` panics on any difference. Returns how it ended.
fn check(program: &Program, budget: u64) -> String {
    let (text, mut functions) = program.layout();
    // An empty last function starts where the one before it does.
    functions.dedup();
    let mut registry = FunctionRegistry::default();
    for pc in functions {
        // The key is the pc, like the reference assembler.
        let name = format!("function_{pc}");
        registry
            .register_function(pc as u32, name.as_bytes(), pc)
            .unwrap();
    }
    let version = program.version;
    let reference =
        Reference::from_text_bytes(&text, loader(version), sbpf_version(version), registry)
            .unwrap();
    let ours = ours_from_reference_text(&reference);
    assert!(
        verify_both(&reference, &ours),
        "the generator made an invalid program"
    );
    let ending = match run_both(&reference, &ours, &input(), budget) {
        Ok(_) => "Ok".into(),
        Err(e) => format!("{e:?}")
            .split(['(', ' '])
            .next()
            .unwrap()
            .to_string(),
    };
    format!("{version:?} {ending}")
}

/// Programs per batch. Each batch gets its own numbered seed.
const BATCH: u32 = 256;

#[test]
fn random_programs_match() {
    let env = |name, default| std::env::var(name).map_or(default, |s| s.parse().unwrap());
    let deadline = Instant::now() + Duration::from_secs(env("CAMSHAFT_FUZZ_SECS", 0));
    let mut seed = env("CAMSHAFT_FUZZ_SEED", 0);
    let budgets = prop::sample::select(vec![5u64, 50, 1000, 100_000]);
    let endings = RefCell::new(BTreeMap::<String, u64>::new());
    loop {
        let config = Config {
            cases: BATCH,
            failure_persistence: None,
            ..Config::default()
        };
        let mut bytes = [0; 32];
        bytes[..8].copy_from_slice(&seed.to_le_bytes());
        let rng = TestRng::from_seed(RngAlgorithm::ChaCha, &bytes);
        let result = TestRunner::new_with_rng(config, rng).run(
            &(program(), budgets.clone()),
            |(program, budget)| {
                *endings
                    .borrow_mut()
                    .entry(check(&program, budget))
                    .or_default() += 1;
                Ok(())
            },
        );
        if let Err(e) = result {
            panic!("seed {seed}: {e}");
        }
        seed += 1;
        if Instant::now() >= deadline {
            break;
        }
    }
    // Shrinking reruns programs, but only after a difference, so these counts are exact.
    let endings = endings.into_inner();
    eprintln!("{} programs, no differences", endings.values().sum::<u64>());
    eprintln!("how they ended: {endings:?}");
}
