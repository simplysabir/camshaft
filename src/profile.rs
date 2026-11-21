//! `camshaft profile`: compute units per function.
//!
//! The profiler keeps its own copy of the call stack. Each instruction's cost goes to the
//! function on top; a syscall's cost goes to a frame named after the syscall, one level down.

use crate::elf::FunctionNames;
use crate::interp::{Observer, Step, Vm};
use crate::syscalls;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Frame {
    /// A function, by the pc where it starts.
    Function(usize),
    Syscall(&'static str),
}

pub struct Profiler {
    stack: Vec<Frame>,
    /// Compute units spent with exactly this stack, innermost frame last.
    spent: BTreeMap<Vec<Frame>, u64>,
    calls: BTreeMap<Frame, u64>,
}

/// One line of the table.
#[derive(Debug, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    /// Spent in the function itself.
    pub self_units: u64,
    /// Spent in the function and everything it called.
    pub total_units: u64,
    pub calls: u64,
}

impl Observer for Profiler {
    fn after(&mut self, vm: &Vm, step: &Step) {
        let mut stack = self.stack.clone();
        if let Some(name) = step.insn.syscall_key().and_then(syscalls::name) {
            stack.push(Frame::Syscall(name));
            *self.calls.entry(Frame::Syscall(name)).or_default() += 1;
        }
        *self.spent.entry(stack).or_default() += step.cost;

        // A call moved into a function at the new pc; an exit returned from one.
        if vm.depth() > step.depth {
            self.stack.push(Frame::Function(vm.pc));
            *self.calls.entry(Frame::Function(vm.pc)).or_default() += 1;
        } else if vm.depth() < step.depth {
            self.stack.pop();
        }
    }
}

impl Profiler {
    pub fn new(entry_pc: usize) -> Self {
        let entry = Frame::Function(entry_pc);
        Self {
            stack: vec![entry],
            spent: BTreeMap::new(),
            calls: BTreeMap::from([(entry, 1)]),
        }
    }

    /// Every function and syscall that ran, most self compute units first.
    pub fn table(&self, names: &FunctionNames) -> Vec<Row> {
        let mut self_units = BTreeMap::<Frame, u64>::new();
        let mut total_units = BTreeMap::<Frame, u64>::new();
        for (stack, &units) in &self.spent {
            *self_units.entry(*stack.last().unwrap()).or_default() += units;
            // A recursive function counts once per stack.
            for frame in stack.iter().collect::<BTreeSet<_>>() {
                *total_units.entry(*frame).or_default() += units;
            }
        }
        let mut rows: Vec<Row> = total_units
            .into_iter()
            .map(|(frame, total_units)| Row {
                name: name(frame, names),
                self_units: self_units.get(&frame).copied().unwrap_or(0),
                total_units,
                calls: self.calls.get(&frame).copied().unwrap_or(0),
            })
            .collect();
        rows.sort_by(|a, b| b.self_units.cmp(&a.self_units).then(a.name.cmp(&b.name)));
        rows
    }

    /// The table as text, plus how much of `charged` it does not explain: a crash outside a
    /// syscall also charges the rest of the budget.
    pub fn report(&self, names: &FunctionNames, charged: u64) -> String {
        let rows = self.table(names);
        let mut out = String::from("  self CU  total CU   calls  function\n");
        for row in &rows {
            out += &format!(
                "{:>9} {:>9} {:>7}  {}\n",
                row.self_units, row.total_units, row.calls, row.name
            );
        }
        let spent: u64 = rows.iter().map(|row| row.self_units).sum();
        if charged > spent {
            let rest = charged - spent;
            out += &format!("plus {rest} CU the runtime charged because the program failed\n");
        }
        out
    }

    /// Folded stacks, one `outer;inner units` line per stack. `inferno-flamegraph` and
    /// `flamegraph.pl` turn this into a flame graph.
    pub fn folded(&self, names: &FunctionNames) -> String {
        let mut out = String::new();
        for (stack, units) in &self.spent {
            let path: Vec<String> = stack.iter().map(|&frame| name(frame, names)).collect();
            out += &format!("{} {units}\n", path.join(";"));
        }
        out
    }
}

fn name(frame: Frame, names: &FunctionNames) -> String {
    match frame {
        Frame::Function(pc) => names.of(pc),
        Frame::Syscall(name) => name.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executable::Executable;
    use crate::version::Version;

    #[test]
    fn charges_each_function_for_its_own_instructions() {
        #[rustfmt::skip]
        let program = [
            0x85, 0x10, 0, 0, 2, 0, 0, 0, // 0: call pc 3
            0x85, 0x10, 0, 0, 1, 0, 0, 0, // 1: call pc 3
            0x95, 0, 0, 0, 0, 0, 0, 0,    // 2: exit
            0xb7, 0, 0, 0, 1, 0, 0, 0,    // 3: mov64 r0, 1
            0x95, 0, 0, 0, 0, 0, 0, 0,    // 4: exit
        ];
        let exe = Executable::from_text(&program, Default::default(), Version::V3);
        let mut profiler = Profiler::new(exe.entry_pc);
        Vm::new(&exe).run_observed(100, &mut profiler).unwrap();

        let names = FunctionNames::read(&[], exe.entry_pc);
        let row = |name: &str, self_units, total_units, calls| Row {
            name: name.into(),
            self_units,
            total_units,
            calls,
        };
        assert_eq!(
            profiler.table(&names),
            [row("function_3", 4, 4, 2), row("entrypoint", 3, 7, 1)]
        );
        assert_eq!(
            profiler.folded(&names),
            "entrypoint 3\nentrypoint;function_3 4\n"
        );
    }
}
