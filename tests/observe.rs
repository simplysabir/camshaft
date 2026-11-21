//! Tracing and profiling real programs. Watching a run must not change it, and the profile
//! must add up to the compute units the run was charged.

use camshaft::elf::FunctionNames;
use camshaft::executable::Executable;
use camshaft::fixture::{self, Parsed};
use camshaft::profile::Profiler;
use camshaft::runtime::{Outcome, process_instruction, process_instruction_observed};
use camshaft::trace::Tracer;
use std::fs;

fn load(name: &str) -> (Executable, FunctionNames) {
    let bytes = fs::read(format!("tests/programs/{name}.so")).unwrap();
    let exe = Executable::from_elf(&bytes).unwrap();
    let names = FunctionNames::read(&bytes, exe.entry_pc);
    (exe, names)
}

fn tour(scenario: u8) -> Parsed {
    let mut parsed =
        fixture::parse(&fs::read_to_string("tests/programs/tour.json").unwrap()).unwrap();
    parsed.instruction.data = vec![scenario];
    parsed
}

fn run(exe: &Executable, parsed: &Parsed) -> Outcome {
    process_instruction(exe, &parsed.instruction, &parsed.accounts, &parsed.env)
}

#[test]
fn watching_does_not_change_the_run() {
    for program in ["tour_unstripped", "tour_v3_unstripped"] {
        let (exe, names) = load(program);
        // Scenario 12 needs a different fixture; the others cover every way a run ends.
        for scenario in (0..=11).chain([13]) {
            let parsed = tour(scenario);
            let plain = format!("{:?}", run(&exe, &parsed));
            let (ix, accounts, env) = (&parsed.instruction, &parsed.accounts, &parsed.env);

            let mut tracer = Tracer::new(Vec::new(), &exe, &names);
            let traced = process_instruction_observed(&exe, ix, accounts, env, &mut tracer);
            let lines = tracer
                .finish()
                .unwrap()
                .iter()
                .filter(|&&b| b == b'\n')
                .count();
            assert_eq!(format!("{traced:?}"), plain, "{program} {scenario}");
            assert_eq!(lines as u64, traced.insn_count, "one line per instruction");

            let mut profiler = Profiler::new(exe.entry_pc);
            let profiled = process_instruction_observed(&exe, ix, accounts, env, &mut profiler);
            assert_eq!(format!("{profiled:?}"), plain, "{program} {scenario}");
            let spent: u64 = profiler.table(&names).iter().map(|r| r.self_units).sum();
            // Either the profile has every unit, or a crash outside a syscall charged the
            // whole budget.
            let charged_all = profiled.compute_units == env.compute_units;
            assert!(
                spent == profiled.compute_units || charged_all && spent < env.compute_units,
                "{program} {scenario}: profiled {spent} of {}",
                profiled.compute_units
            );
        }
    }
}

/// The milestone's goal: point it at a real program and see what burns the most compute.
#[test]
fn profile_names_the_most_expensive_functions() {
    for program in ["tour_unstripped", "tour_v3_unstripped"] {
        let (exe, names) = load(program);
        let parsed = tour(0);
        let mut profiler = Profiler::new(exe.entry_pc);
        let (ix, accounts, env) = (&parsed.instruction, &parsed.accounts, &parsed.env);
        let outcome = process_instruction_observed(&exe, ix, accounts, env, &mut profiler);
        let rows = profiler.table(&names);
        // 255 is on the curve, so finding the PDA costs two tries of 1,500 plus the call.
        assert_eq!(rows[0].name, "sol_try_find_program_address");
        assert_eq!(rows[0].self_units, 3001);
        let entry = rows.iter().find(|r| r.name == "entrypoint").unwrap();
        assert_eq!(entry.total_units, outcome.compute_units);
        assert!(
            rows.iter()
                .any(|r| r.name == "solana_program_entrypoint::deserialize"),
            "{program} has its symbols"
        );
    }
}

#[test]
fn stripped_programs_get_pc_names() {
    let (exe, names) = load("tour");
    assert_eq!(names.of(exe.entry_pc), "entrypoint");
    assert_eq!(names.of(980), "function_980");
}
