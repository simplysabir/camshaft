//! Function names from an ELF's `.symtab`, for the profiler. This is debug information: a
//! stripped or odd file just has no names, and loading never looks at it.

use super::parse::{EHDR_SIZE, Header, SHDR_SIZE, SYM_SIZE, Shdr, Sym, string_in};
use crate::insn::INSN_SIZE;
use std::collections::BTreeMap;

/// Longest name read. Rust names with generics can be long.
const NAME_MAX: usize = 4096;

/// Each function's name, by the pc where it starts.
pub struct FunctionNames(BTreeMap<usize, String>);

impl FunctionNames {
    /// Demangled names from `.symtab`. A stripped file, like what `cargo build-sbf` puts in
    /// `target/deploy`, has none; the unstripped copy is in `target/<target>/release/`.
    /// The entry is called `entrypoint` if no symbol names it.
    pub fn read(bytes: &[u8], entry_pc: usize) -> Self {
        let mut names = names(bytes).unwrap_or_default();
        names.entry(entry_pc).or_insert_with(|| "entrypoint".into());
        Self(names)
    }

    /// The function starting at `pc`: its symbol, or `function_<pc>`.
    pub fn of(&self, pc: usize) -> String {
        self.0
            .get(&pc)
            .cloned()
            .unwrap_or_else(|| format!("function_{pc}"))
    }
}

fn names(bytes: &[u8]) -> Option<BTreeMap<usize, String>> {
    let header = Header::read(bytes.get(..EHDR_SIZE)?);
    let start = usize::try_from(header.shoff).ok()?;
    let table = bytes.get(start..start.checked_add(SHDR_SIZE * header.shnum as usize)?)?;
    let (table, _) = table.as_chunks::<SHDR_SIZE>();
    let sections: Vec<Shdr> = table.iter().map(|s| Shdr::read(s)).collect();
    let section_names = sections.get(header.shstrndx as usize)?;
    let named = |name: &[u8]| {
        sections
            .iter()
            .find(|s| string_in(bytes, section_names, s.name, NAME_MAX).is_ok_and(|n| n == name))
    };
    let (text, symtab, strtab) = (named(b".text")?, named(b".symtab")?, named(b".strtab")?);

    let mut names = BTreeMap::new();
    let (symbols, _) = bytes.get(symtab.file_range())?.as_chunks::<SYM_SIZE>();
    for symbol in symbols.iter().map(|s| Sym::read(s)) {
        if !symbol.is_function() || !text.vm_range().contains(&symbol.value) {
            continue;
        }
        let Ok(name) = string_in(bytes, strtab, symbol.name, NAME_MAX) else {
            continue;
        };
        let pc = ((symbol.value - text.addr) / INSN_SIZE as u64) as usize;
        let name = format!(
            "{:#}",
            rustc_demangle::demangle(&String::from_utf8_lossy(name))
        );
        // Two names for one pc: keep the first.
        names.entry(pc).or_insert(name);
    }
    Some(names)
}
