//! Relocatable-object (`.o`) loading — sections, defined symbols, relocations.

use crate::reloc::r_type_from_object;
use anyhow::{Context, Result};
use object::read::{Object, ObjectSection, ObjectSymbol, RelocationTarget};
use object::{File, RelocationFlags, SectionKind};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Clone)]
pub struct InputSection {
    pub name: String,
    pub data: Vec<u8>,
    /// relocs: (offset, r_type, addend, symbol_name)
    pub relocs: Vec<(u64, u32, i64, String)>,
    pub align: u64,
}

#[derive(Clone)]
pub struct DefinedSymbol {
    pub name: String,
    /// Which input section (pre-merge name, e.g. `.text` / `.text.foo`).
    pub section: String,
    /// Offset within that input section.
    pub offset: u64,
    pub global: bool,
    pub weak: bool,
}

pub struct LoadedObject {
    pub sections: Vec<InputSection>,
    pub symbols: Vec<DefinedSymbol>,
}

impl LoadedObject {
    /// Every symbol name referenced by a relocation in this object (potential
    /// external dependency — may turn out to be satisfied by our own
    /// `symbols` for intra-object references, the caller filters that out).
    pub fn referenced_symbols(&self) -> impl Iterator<Item = &str> {
        self.sections
            .iter()
            .flat_map(|s| s.relocs.iter())
            .map(|(_, _, _, name)| name.as_str())
    }
}

pub fn parse_one_object(bytes: &[u8], path: &Path) -> Result<LoadedObject> {
    let file = File::parse(bytes).with_context(|| format!("{}: not an object", path.display()))?;

    let mut sections = Vec::new();
    // object::SectionIndex has no Ord impl; key on the raw index instead.
    let mut sec_index_names: BTreeMap<usize, String> = BTreeMap::new();

    for sec in file.sections() {
        let name = sec.name().unwrap_or("").to_string();
        if name.is_empty()
            || name.starts_with(".rela")
            || name.starts_with(".rel")
            || name == ".symtab"
            || name == ".strtab"
            || name == ".shstrtab"
            || name.starts_with(".note")
            || name.starts_with(".comment")
            || name.starts_with(".debug")
            || name.starts_with(".eh_frame")
        {
            continue;
        }
        sec_index_names.insert(sec.index().0, name.clone());
        let data = if sec.kind() == SectionKind::UninitializedData {
            vec![0u8; sec.size() as usize]
        } else {
            sec.data().unwrap_or(&[]).to_vec()
        };
        let mut relocs = Vec::new();
        for (offset, reloc) in sec.relocations() {
            let (r_type, addend) = match reloc.flags() {
                RelocationFlags::Elf { r_type } => (r_type, reloc.addend()),
                RelocationFlags::Generic {
                    kind,
                    encoding,
                    size,
                } => (r_type_from_object(kind, size, encoding), reloc.addend()),
                _ => continue,
            };
            let sym_name = match reloc.target() {
                RelocationTarget::Symbol(idx) => file
                    .symbol_by_index(idx)
                    .ok()
                    .and_then(|s| s.name().ok().map(|n| n.to_string()))
                    .unwrap_or_default(),
                RelocationTarget::Section(idx) => {
                    sec_index_names.get(&idx.0).cloned().unwrap_or_default()
                }
                _ => String::new(),
            };
            if sym_name.is_empty() {
                continue;
            }
            relocs.push((offset, r_type, addend, sym_name));
        }
        let align = sec.align().max(1);
        sections.push(InputSection {
            name,
            data,
            relocs,
            align,
        });
    }

    let mut symbols = Vec::new();
    for sym in file.symbols() {
        let name = match sym.name() {
            Ok(n) if !n.is_empty() => n.to_string(),
            _ => continue,
        };
        if sym.kind() == object::SymbolKind::File {
            continue;
        }
        if sym.is_undefined() {
            continue;
        }
        let sec_name = match sym.section() {
            object::SymbolSection::Section(idx) => sec_index_names
                .get(&idx.0)
                .cloned()
                .unwrap_or_else(|| ".text".into()),
            object::SymbolSection::Absolute | object::SymbolSection::Common => "COMMON".into(),
            _ => continue,
        };
        symbols.push(DefinedSymbol {
            name,
            section: if sec_name == "COMMON" {
                ".bss".into()
            } else {
                sec_name
            },
            offset: sym.address(),
            global: sym.is_global(),
            weak: sym.is_weak(),
        });
    }

    Ok(LoadedObject { sections, symbols })
}
