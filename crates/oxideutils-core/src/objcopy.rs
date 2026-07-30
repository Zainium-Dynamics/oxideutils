//! Object copy / transform (GNU objcopy subset).
//!
//! Supported operations:
//! - copy file → output (identity or transform)
//! - `--strip-all` / `--strip-debug` / `--strip-unneeded` (delegates to strip)
//! - `-R` / `--remove-section=NAME`
//! - `-j` / `--only-section=NAME`
//! - binary extract of a single section (`-O binary -j .text` style simplified)

use crate::error::{OxideError, Result};
use crate::strip::{StripOptions, strip_bytes};
use crate::utils::atomic_write;
use object::write::{Object as WriteObject, Symbol as WriteSymbol, SymbolSection};
use object::{
    Object, ObjectSection, ObjectSymbol, SectionKind as ReadSectionKind, SymbolFlags, SymbolKind,
    SymbolScope,
};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct ObjcopyOptions {
    pub strip_all: bool,
    pub strip_debug: bool,
    pub strip_unneeded: bool,
    /// Keep only these sections (plus NULL / shstrtab plumbing as needed).
    pub only_sections: Vec<String>,
    /// Drop these sections by name.
    pub remove_sections: Vec<String>,
    /// Output target name: "elf", "binary", or empty = same as input.
    pub output_target: Option<String>,
}

impl ObjcopyOptions {
    fn wants_strip(&self) -> bool {
        self.strip_all || self.strip_debug || self.strip_unneeded
    }

    fn filter_active(&self) -> bool {
        !self.only_sections.is_empty() || !self.remove_sections.is_empty()
    }

    fn keep_section(&self, name: &str) -> bool {
        if self.remove_sections.iter().any(|r| r == name) {
            return false;
        }
        if !self.only_sections.is_empty() {
            return self.only_sections.iter().any(|o| o == name)
                || name.is_empty()
                || name == ".shstrtab";
        }
        true
    }
}

/// Copy/transform `input` into `output`.
pub fn objcopy_file(input: &Path, output: &Path, opts: &ObjcopyOptions) -> Result<()> {
    let data = fs::read(input).map_err(|e| OxideError::io_path(input, e))?;
    let out = objcopy_bytes(&data, opts)
        .map_err(|e| OxideError::format(input.display().to_string(), e.to_string()))?;
    // Preserve mode from input when overwriting a different path is still fine.
    atomic_write(output, &out, Some(input))?;
    Ok(())
}

pub fn objcopy_bytes(data: &[u8], opts: &ObjcopyOptions) -> anyhow::Result<Vec<u8>> {
    // binary extract of only-section(s)
    if opts
        .output_target
        .as_deref()
        .is_some_and(|t| t.eq_ignore_ascii_case("binary") || t == "bin")
    {
        return extract_binary(data, opts);
    }

    let mut bytes = data.to_vec();

    // Section filtering first (rebuild relocatable or ELF-aware filter)
    if opts.filter_active() {
        bytes = filter_sections(&bytes, opts)?;
    }

    if opts.wants_strip() {
        bytes = strip_bytes(
            &bytes,
            StripOptions {
                strip_all: opts.strip_all,
                strip_debug: opts.strip_debug,
                strip_unneeded: opts.strip_unneeded,
            },
        )?;
    }

    Ok(bytes)
}

fn extract_binary(data: &[u8], opts: &ObjcopyOptions) -> anyhow::Result<Vec<u8>> {
    let obj = object::File::parse(data)?;
    let mut out = Vec::new();
    let names: Vec<&str> = if opts.only_sections.is_empty() {
        // all allocated PROGBITS-like
        Vec::new()
    } else {
        opts.only_sections.iter().map(|s| s.as_str()).collect()
    };

    for sec in obj.sections() {
        let name = sec.name().unwrap_or("");
        if !names.is_empty() {
            if !names.contains(&name) {
                continue;
            }
        } else {
            // default: only first .text if nothing specified
            if name != ".text" {
                continue;
            }
        }
        if opts.remove_sections.iter().any(|r| r == name) {
            continue;
        }
        let d = sec.uncompressed_data()?;
        out.extend_from_slice(&d);
    }
    if out.is_empty() && !names.is_empty() {
        anyhow::bail!(
            "no matching sections for binary extract: {}",
            names.join(", ")
        );
    }
    Ok(out)
}

fn filter_sections(data: &[u8], opts: &ObjcopyOptions) -> anyhow::Result<Vec<u8>> {
    // Prefer relocatable rebuild; for ELF exec/shared with only remove/only, use strip-like ELF path
    if data.len() >= 4 && data[0..4] == [0x7f, b'E', b'L', b'F'] {
        // Try relocatable path first if it works and file has few program headers
        if let Ok(v) = filter_via_object_write(data, opts) {
            return Ok(v);
        }
        return filter_elf_sections(data, opts);
    }
    filter_via_object_write(data, opts)
}

fn filter_via_object_write(data: &[u8], opts: &ObjcopyOptions) -> anyhow::Result<Vec<u8>> {
    let in_obj = object::File::parse(data)?;
    let mut out = WriteObject::new(in_obj.format(), in_obj.architecture(), in_obj.endianness());

    let mut section_map: Vec<Option<object::write::SectionId>> = Vec::new();

    for section in in_obj.sections() {
        let name = section.name().unwrap_or("");
        if !opts.keep_section(name) {
            section_map.push(None);
            continue;
        }
        let kind = map_section_kind(section.kind());
        let id = out.add_section(Vec::new(), name.as_bytes().to_vec(), kind);
        let align = section.align().max(1);
        if section.kind().is_bss() {
            out.append_section_bss(id, section.size(), align);
        } else {
            let bytes = section.uncompressed_data()?;
            out.set_section_data(id, bytes.as_ref().to_vec(), align);
        }
        section_map.push(Some(id));
    }

    for sym in in_obj.symbols() {
        let name = sym.name().unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let section = match sym.section() {
            object::SymbolSection::Undefined => SymbolSection::Undefined,
            object::SymbolSection::Absolute => SymbolSection::Absolute,
            object::SymbolSection::Common => SymbolSection::Common,
            object::SymbolSection::Section(idx) => {
                match section_map.get(idx.0).copied().flatten() {
                    Some(id) => SymbolSection::Section(id),
                    None => continue,
                }
            }
            _ => SymbolSection::Undefined,
        };
        let _ = SymbolKind::Text;
        out.add_symbol(WriteSymbol {
            name: name.as_bytes().to_vec(),
            value: sym.address(),
            size: sym.size(),
            kind: sym.kind(),
            scope: if sym.is_local() {
                SymbolScope::Compilation
            } else {
                sym.scope()
            },
            weak: sym.is_weak(),
            section,
            flags: SymbolFlags::None,
        });
    }

    let mut buf = Vec::new();
    out.emit(&mut buf)?;
    Ok(buf)
}

/// ELF: drop non-allocated sections matching filter; keep PHDRs intact.
fn filter_elf_sections(data: &[u8], opts: &ObjcopyOptions) -> anyhow::Result<Vec<u8>> {
    // Reuse strip machinery by mapping filter → strip-like drop list is hard;
    // implement via temporary "remove" as strip_debug style using custom keep predicate.
    // For only_sections on executables, extract is more common with -O binary.
    if !opts.only_sections.is_empty() {
        // If only-sections and not binary target, still try object write
        return filter_via_object_write(data, opts);
    }
    // remove-sections only: strip path with names
    let mut bytes = crate::strip::strip_bytes(
        data,
        StripOptions {
            strip_all: false,
            strip_debug: opts.remove_sections.iter().any(|s| s.starts_with(".debug")),
            strip_unneeded: false,
        },
    )?;
    // If user asked to remove specific non-debug sections, second pass via write
    if opts
        .remove_sections
        .iter()
        .any(|s| !s.starts_with(".debug") && s != ".comment")
    {
        bytes = filter_via_object_write(&bytes, opts).unwrap_or(bytes);
    }
    Ok(bytes)
}

fn map_section_kind(k: ReadSectionKind) -> object::SectionKind {
    match k {
        ReadSectionKind::Text => object::SectionKind::Text,
        ReadSectionKind::Data => object::SectionKind::Data,
        ReadSectionKind::ReadOnlyData => object::SectionKind::ReadOnlyData,
        ReadSectionKind::ReadOnlyString => object::SectionKind::ReadOnlyString,
        ReadSectionKind::ReadOnlyDataWithRel => object::SectionKind::ReadOnlyDataWithRel,
        ReadSectionKind::UninitializedData => object::SectionKind::UninitializedData,
        ReadSectionKind::Common => object::SectionKind::Common,
        ReadSectionKind::Tls => object::SectionKind::Tls,
        ReadSectionKind::UninitializedTls => object::SectionKind::UninitializedTls,
        ReadSectionKind::TlsVariables => object::SectionKind::TlsVariables,
        ReadSectionKind::Debug => object::SectionKind::Debug,
        ReadSectionKind::Other => object::SectionKind::Other,
        ReadSectionKind::Metadata => object::SectionKind::Metadata,
        ReadSectionKind::Linker => object::SectionKind::Linker,
        ReadSectionKind::Note => object::SectionKind::Note,
        ReadSectionKind::Elf(x) => object::SectionKind::Elf(x),
        _ => object::SectionKind::Unknown,
    }
}
