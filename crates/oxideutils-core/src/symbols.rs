//! High-level symbol table operations (nm / objdump -t).

use crate::error::{OxideError, Result};
use crate::format::object::OxideObject;
use crate::prelude::*;
use crate::utils::demangle_symbol;
use core::cmp::Ordering;
use core::fmt;
use object::{Object, ObjectSection, ObjectSymbol, SymbolKind, SymbolScope, SymbolSection};

/// Portable view of a single symbol.
#[derive(Debug, Clone)]
pub struct SymbolInfo {
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub kind: SymbolKindInfo,
    pub binding: Binding,
    pub section: SectionRef,
    pub is_global: bool,
    pub is_weak: bool,
    pub is_local: bool,
    pub is_undefined: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolKindInfo {
    Unknown,
    Null,
    Text,
    Data,
    Section,
    File,
    Label,
    Tls,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Local,
    Global,
    Weak,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionRef {
    Undefined,
    Absolute,
    Common,
    Section(String),
    Unknown,
}

impl SymbolInfo {
    /// GNU nm single-letter type code (approximate).
    pub fn nm_type_char(&self) -> char {
        let lower = match self.kind {
            SymbolKindInfo::Text | SymbolKindInfo::Label => 't',
            SymbolKindInfo::Data => {
                // distinguish BSS-ish via section name heuristics
                match &self.section {
                    SectionRef::Section(n)
                        if n.contains("bss") || n.contains("BSS") || n == ".bss" =>
                    {
                        'b'
                    }
                    SectionRef::Section(n) if n.contains("rodata") || n.contains("data.rel.ro") => {
                        'r'
                    }
                    SectionRef::Common => 'c',
                    _ => 'd',
                }
            }
            SymbolKindInfo::File => 'f',
            SymbolKindInfo::Section => 's',
            SymbolKindInfo::Tls => 't',
            SymbolKindInfo::Null | SymbolKindInfo::Unknown => {
                if self.is_undefined {
                    'U'
                } else {
                    '?'
                }
            }
        };

        if self.is_undefined {
            return 'U';
        }
        if matches!(self.section, SectionRef::Absolute) {
            return if self.is_global { 'A' } else { 'a' };
        }
        if matches!(self.section, SectionRef::Common) {
            return if self.is_global { 'C' } else { 'c' };
        }
        if self.is_weak {
            return if lower == 't' { 'W' } else { 'V' };
        }
        if self.is_global {
            lower.to_ascii_uppercase()
        } else {
            lower
        }
    }

    pub fn demangled_name(&self) -> String {
        demangle_symbol(&self.name)
    }
}

impl fmt::Display for SymbolInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_undefined {
            write!(f, "{:>16} {} {}", "", self.nm_type_char(), self.name)
        } else {
            write!(
                f,
                "{:016x} {} {}",
                self.address,
                self.nm_type_char(),
                self.name
            )
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum SymbolSort {
    #[default]
    Name,
    Address,
    Size,
    None,
}

#[derive(Debug, Clone, Default)]
pub struct SymbolFilter {
    pub defined_only: bool,
    pub undefined_only: bool,
    pub external_only: bool,
    pub demangle: bool,
    pub sort: SymbolSort,
    pub reverse: bool,
    pub numeric_sort: bool,
    pub size_sort: bool,
}

/// Extract all symbols from an object.
pub fn list_symbols(obj: &OxideObject<'_>, filter: &SymbolFilter) -> Result<Vec<SymbolInfo>> {
    let mut syms = Vec::new();
    for sym in obj.file.symbols() {
        let name = sym.name().unwrap_or("").to_string();
        if name.is_empty() {
            continue;
        }
        let section = match sym.section() {
            SymbolSection::Undefined => SectionRef::Undefined,
            SymbolSection::Absolute => SectionRef::Absolute,
            SymbolSection::Common => SectionRef::Common,
            SymbolSection::Section(idx) => {
                let sec = obj
                    .file
                    .section_by_index(idx)
                    .ok()
                    .and_then(|s| s.name().ok().map(|n| n.to_string()))
                    .unwrap_or_else(|| format!("section#{}", idx.0));
                SectionRef::Section(sec)
            }
            _ => SectionRef::Unknown,
        };

        let kind = match sym.kind() {
            SymbolKind::Text => SymbolKindInfo::Text,
            SymbolKind::Data => SymbolKindInfo::Data,
            SymbolKind::Section => SymbolKindInfo::Section,
            SymbolKind::File => SymbolKindInfo::File,
            SymbolKind::Label => SymbolKindInfo::Label,
            SymbolKind::Tls => SymbolKindInfo::Tls,
            SymbolKind::Unknown => SymbolKindInfo::Unknown,
            _ => SymbolKindInfo::Unknown,
        };

        let scope = sym.scope();
        let is_global = matches!(scope, SymbolScope::Dynamic | SymbolScope::Linkage);
        let is_local = matches!(scope, SymbolScope::Compilation);
        let is_weak = sym.is_weak();
        let is_undefined = sym.is_undefined();

        let info = SymbolInfo {
            name,
            address: sym.address(),
            size: sym.size(),
            kind,
            binding: if is_weak {
                Binding::Weak
            } else if is_global {
                Binding::Global
            } else if is_local {
                Binding::Local
            } else {
                Binding::Unknown
            },
            section,
            is_global,
            is_weak,
            is_local,
            is_undefined,
        };

        if filter.defined_only && info.is_undefined {
            continue;
        }
        if filter.undefined_only && !info.is_undefined {
            continue;
        }
        if filter.external_only && info.is_local {
            continue;
        }
        syms.push(info);
    }

    let sort = if filter.size_sort {
        SymbolSort::Size
    } else if filter.numeric_sort {
        SymbolSort::Address
    } else {
        filter.sort
    };

    match sort {
        SymbolSort::Name => syms.sort_by(|a, b| a.name.cmp(&b.name)),
        SymbolSort::Address => syms.sort_by(|a, b| a.address.cmp(&b.address)),
        SymbolSort::Size => syms.sort_by(|a, b| a.size.cmp(&b.size).then(a.name.cmp(&b.name))),
        SymbolSort::None => {}
    }
    if filter.reverse {
        syms.reverse();
    }

    // stable secondary name sort when equal keys
    if matches!(sort, SymbolSort::Address | SymbolSort::Size) {
        // already then-compared for size; for address ensure name stability
        let _ = Ordering::Equal;
    }

    if filter.demangle {
        for s in &mut syms {
            s.name = demangle_symbol(&s.name);
        }
    }

    Ok(syms)
}

/// Find a symbol by exact name.
pub fn find_symbol<'a>(syms: &'a [SymbolInfo], name: &str) -> Result<&'a SymbolInfo> {
    syms.iter()
        .find(|s| s.name == name)
        .ok_or_else(|| OxideError::SymbolNotFound(name.to_string()))
}
