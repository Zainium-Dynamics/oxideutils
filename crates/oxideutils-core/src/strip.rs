//! Symbol / debug stripping (GNU strip subset).
//!
//! - **ET_REL** relocatable objects: rebuild via `object::write`.
//! - **ET_EXEC / ET_DYN** (ELF32 + ELF64): drop non-allocated symbol/debug sections
//!   while preserving program headers and loadable content.
//!
//! Never silently returns the original bytes when strip work was requested and
//! sections could not be processed — callers get an error instead.

use crate::error::{OxideError, Result};
use crate::utils::atomic_write;
use object::write::{Object as WriteObject, Symbol as WriteSymbol, SymbolSection};
use object::{
    Object, ObjectSection, ObjectSymbol, SectionKind as ReadSectionKind, SymbolFlags, SymbolKind,
    SymbolScope,
};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Copy, Default)]
pub struct StripOptions {
    /// Remove all symbols (GNU -s / --strip-all).
    pub strip_all: bool,
    /// Remove debugging sections (GNU -g / --strip-debug).
    pub strip_debug: bool,
    /// Remove symbols not needed for relocation (GNU --strip-unneeded).
    pub strip_unneeded: bool,
}

impl StripOptions {
    pub fn wants_work(self) -> bool {
        self.strip_all || self.strip_debug || self.strip_unneeded
    }

    fn normalized(self) -> Self {
        if self.wants_work() {
            self
        } else {
            Self {
                strip_all: true,
                ..self
            }
        }
    }
}

fn is_debug_section(name: &str) -> bool {
    name.starts_with(".debug")
        || name.starts_with(".zdebug")
        || name == ".gdb_index"
        || name == ".stab"
        || name == ".stabstr"
        || name == ".comment"
}

fn drop_section(name: &str, sh_type: u32, flags: u64, opts: StripOptions) -> bool {
    // Never drop allocated sections (runtime / loader needs them).
    if flags & SHF_ALLOC != 0 {
        return false;
    }
    if opts.strip_all || opts.strip_unneeded {
        if sh_type == SHT_SYMTAB || name == ".symtab" || name == ".strtab" {
            return true;
        }
        // Non-allocated relocation tables for static linking
        if sh_type == SHT_REL || sh_type == SHT_RELA {
            return true;
        }
        if name.starts_with(".rel") || name.starts_with(".rela") {
            return true;
        }
    }
    if (opts.strip_debug || opts.strip_all) && is_debug_section(name) {
        return true;
    }
    false
}

/// Strip `input` → `output` (paths may be equal for in-place).
/// Uses atomic replace and preserves mode bits from the input when possible.
pub fn strip_file(input: &Path, output: &Path, opts: StripOptions) -> Result<()> {
    let opts = opts.normalized();
    let data = fs::read(input).map_err(|e| OxideError::io_path(input, e))?;
    let stripped = strip_bytes(&data, opts)
        .map_err(|e| OxideError::format(input.display().to_string(), e.to_string()))?;
    atomic_write(output, &stripped, Some(input))?;
    Ok(())
}

pub fn strip_bytes(data: &[u8], opts: StripOptions) -> anyhow::Result<Vec<u8>> {
    let opts = opts.normalized();
    if data.len() >= 4 && data[0..4] == [0x7f, b'E', b'L', b'F'] {
        let out = strip_elf(data, opts)?;
        verify_still_object(&out)?;
        return Ok(out);
    }
    let out = strip_relocatable_object(data, opts)?;
    verify_still_object(&out)?;
    Ok(out)
}

fn verify_still_object(data: &[u8]) -> anyhow::Result<()> {
    object::File::parse(data).map_err(|e| anyhow::anyhow!("strip produced unreadable object: {e}"))?;
    Ok(())
}

fn strip_relocatable_object(data: &[u8], opts: StripOptions) -> anyhow::Result<Vec<u8>> {
    let in_obj = object::File::parse(data)?;
    let mut out = WriteObject::new(
        in_obj.format(),
        in_obj.architecture(),
        in_obj.endianness(),
    );

    let mut section_map: Vec<Option<object::write::SectionId>> = Vec::new();

    for section in in_obj.sections() {
        let name = section.name().unwrap_or("");
        // object crate path has no raw sh_type easily; use name heuristics
        if drop_section(name, 0, 0, opts)
            || ((opts.strip_all || opts.strip_unneeded)
                && (name == ".symtab" || name == ".strtab"))
            || ((opts.strip_debug || opts.strip_all) && is_debug_section(name))
        {
            // For relocatable write path, SHF_ALLOC is not available the same way;
            // only drop known debug/symtab names.
            if is_debug_section(name)
                || name == ".symtab"
                || name == ".strtab"
                || ((opts.strip_all || opts.strip_unneeded)
                    && (name.starts_with(".rel") || name.starts_with(".rela")))
            {
                section_map.push(None);
                continue;
            }
        }
        if (opts.strip_all || opts.strip_unneeded) && (name == ".symtab" || name == ".strtab") {
            section_map.push(None);
            continue;
        }
        if (opts.strip_debug || opts.strip_all) && is_debug_section(name) {
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

    if !opts.strip_all {
        for sym in in_obj.symbols() {
            let name = sym.name().unwrap_or("");
            if name.is_empty() {
                continue;
            }
            if opts.strip_debug && matches!(sym.kind(), SymbolKind::File) {
                continue;
            }
            if opts.strip_unneeded && sym.is_local() && !sym.is_undefined() {
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
    }

    let mut buf = Vec::new();
    out.emit(&mut buf)?;
    Ok(buf)
}

fn strip_elf(data: &[u8], opts: StripOptions) -> anyhow::Result<Vec<u8>> {
    if data.len() < 16 {
        anyhow::bail!("ELF truncated");
    }
    let is_64 = data[4] == 2;
    let le = data[5] == 1;
    let class_ok = data[4] == 1 || data[4] == 2;
    if !class_ok {
        anyhow::bail!("unsupported ELF class");
    }
    strip_elf_common(data, is_64, le, opts)
}

const SHT_NULL: u32 = 0;
const SHT_SYMTAB: u32 = 2;
const SHT_RELA: u32 = 4;
const SHT_NOBITS: u32 = 8;
const SHT_REL: u32 = 9;
const SHT_GROUP: u32 = 17;
const SHF_ALLOC: u64 = 0x2;

#[derive(Clone)]
struct Shdr {
    name_off: u32,
    sh_type: u32,
    flags: u64,
    addr: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    addralign: u64,
    entsize: u64,
    name: String,
}

fn r_u16(data: &[u8], off: usize, le: bool) -> u16 {
    let b = [data[off], data[off + 1]];
    if le {
        u16::from_le_bytes(b)
    } else {
        u16::from_be_bytes(b)
    }
}
fn r_u32(data: &[u8], off: usize, le: bool) -> u32 {
    let b = [data[off], data[off + 1], data[off + 2], data[off + 3]];
    if le {
        u32::from_le_bytes(b)
    } else {
        u32::from_be_bytes(b)
    }
}
fn r_u64(data: &[u8], off: usize, le: bool) -> u64 {
    let mut b = [0u8; 8];
    b.copy_from_slice(&data[off..off + 8]);
    if le {
        u64::from_le_bytes(b)
    } else {
        u64::from_be_bytes(b)
    }
}
fn w_u16(buf: &mut [u8], off: usize, v: u16, le: bool) {
    let b = if le {
        v.to_le_bytes()
    } else {
        v.to_be_bytes()
    };
    buf[off..off + 2].copy_from_slice(&b);
}
fn w_u32(buf: &mut [u8], off: usize, v: u32, le: bool) {
    let b = if le {
        v.to_le_bytes()
    } else {
        v.to_be_bytes()
    };
    buf[off..off + 4].copy_from_slice(&b);
}
fn w_u64(buf: &mut [u8], off: usize, v: u64, le: bool) {
    let b = if le {
        v.to_le_bytes()
    } else {
        v.to_be_bytes()
    };
    buf[off..off + 8].copy_from_slice(&b);
}

fn strip_elf_common(data: &[u8], is_64: bool, le: bool, opts: StripOptions) -> anyhow::Result<Vec<u8>> {
    let e_ehsize = if is_64 { 64 } else { 52 };
    if data.len() < e_ehsize {
        anyhow::bail!("ELF truncated");
    }

    let (e_phoff, e_shoff, e_phentsize, e_phnum, e_shentsize, e_shnum, e_shstrndx) = if is_64 {
        (
            r_u64(data, 32, le) as usize,
            r_u64(data, 40, le) as usize,
            r_u16(data, 54, le) as usize,
            r_u16(data, 56, le) as usize,
            r_u16(data, 58, le) as usize,
            r_u16(data, 60, le) as usize,
            r_u16(data, 62, le) as usize,
        )
    } else {
        (
            r_u32(data, 28, le) as usize,
            r_u32(data, 32, le) as usize,
            r_u16(data, 42, le) as usize,
            r_u16(data, 44, le) as usize,
            r_u16(data, 46, le) as usize,
            r_u16(data, 48, le) as usize,
            r_u16(data, 50, le) as usize,
        )
    };

    let expect_shentsize = if is_64 { 64 } else { 40 };
    if e_shentsize != expect_shentsize {
        anyhow::bail!(
            "unsupported section header size {e_shentsize} (want {expect_shentsize})"
        );
    }
    if e_shnum == 0 {
        anyhow::bail!("ELF has no section headers; cannot strip safely");
    }
    if e_shoff.saturating_add(e_shnum.saturating_mul(e_shentsize)) > data.len() {
        anyhow::bail!("section headers out of range");
    }

    let mut shdrs: Vec<Shdr> = (0..e_shnum)
        .map(|i| parse_shdr(data, e_shoff + i * e_shentsize, is_64, le))
        .collect();

    // Resolve names from shstrtab
    if e_shstrndx < shdrs.len() {
        let str_off = shdrs[e_shstrndx].offset as usize;
        let str_sz = shdrs[e_shstrndx].size as usize;
        if str_off.saturating_add(str_sz) <= data.len() {
            let tab = &data[str_off..str_off + str_sz];
            for sh in &mut shdrs {
                let no = sh.name_off as usize;
                if no < tab.len() {
                    let end = tab[no..]
                        .iter()
                        .position(|&b| b == 0)
                        .map(|p| no + p)
                        .unwrap_or(tab.len());
                    sh.name = String::from_utf8_lossy(&tab[no..end]).into_owned();
                }
            }
        }
    }

    let mut keep = vec![true; shdrs.len()];
    keep[0] = true;
    for (i, sh) in shdrs.iter().enumerate().skip(1) {
        if i == e_shstrndx {
            keep[i] = true;
            continue;
        }
        if sh.flags & SHF_ALLOC != 0 {
            keep[i] = true;
            continue;
        }
        keep[i] = !drop_section(&sh.name, sh.sh_type, sh.flags, opts);
    }

    // Nothing to remove: still OK (already stripped)
    if keep.iter().all(|&k| k) {
        return Ok(data.to_vec());
    }

    let mut new_index = vec![None; shdrs.len()];
    let mut kept: Vec<usize> = Vec::new();
    for (i, &k) in keep.iter().enumerate() {
        if k {
            new_index[i] = Some(kept.len());
            kept.push(i);
        }
    }

    let new_shstrndx = new_index
        .get(e_shstrndx)
        .copied()
        .flatten()
        .ok_or_else(|| anyhow::anyhow!("shstrtab section was dropped"))?;

    // End of all program header file ranges
    let mut segment_end = e_ehsize.max(e_phoff + e_phnum * e_phentsize);
    for i in 0..e_phnum {
        let poff = e_phoff + i * e_phentsize;
        if is_64 {
            if poff + 56 > data.len() {
                break;
            }
            let p_offset = r_u64(data, poff + 8, le) as usize;
            let p_filesz = r_u64(data, poff + 32, le) as usize;
            segment_end = segment_end.max(p_offset.saturating_add(p_filesz));
        } else {
            if poff + 32 > data.len() {
                break;
            }
            let p_offset = r_u32(data, poff + 4, le) as usize;
            let p_filesz = r_u32(data, poff + 16, le) as usize;
            segment_end = segment_end.max(p_offset.saturating_add(p_filesz));
        }
    }

    let mut out = data[..segment_end.min(data.len())].to_vec();
    let align_pad = if is_64 { 8 } else { 4 };
    while out.len() % align_pad != 0 {
        out.push(0);
    }

    let mut new_shdrs: Vec<Shdr> = Vec::with_capacity(kept.len());
    for &old_i in &kept {
        let mut sh = shdrs[old_i].clone();
        if sh.sh_type == SHT_NULL {
            new_shdrs.push(sh);
            continue;
        }

        // Remap sh_link always (section index)
        if sh.link as usize > 0 {
            if let Some(Some(ni)) = new_index.get(sh.link as usize) {
                sh.link = *ni as u32;
            } else {
                sh.link = 0;
            }
        }
        // Remap sh_info when it is a section index
        if matches!(sh.sh_type, SHT_REL | SHT_RELA | SHT_GROUP) && sh.info as usize > 0 {
            if let Some(Some(ni)) = new_index.get(sh.info as usize) {
                sh.info = *ni as u32;
            } else {
                sh.info = 0;
            }
        }

        if sh.flags & SHF_ALLOC != 0 {
            // Loadable content stays at original file offsets inside segments
            new_shdrs.push(sh);
            continue;
        }

        // Non-alloc: repack (skip NOBITS file payload)
        let old_off = sh.offset as usize;
        let old_sz = sh.size as usize;
        if sh.sh_type == SHT_NOBITS || old_sz == 0 {
            sh.offset = out.len() as u64;
            new_shdrs.push(sh);
            continue;
        }
        if old_off.saturating_add(old_sz) > data.len() {
            anyhow::bail!(
                "section '{}' data out of range (offset {old_off}+{old_sz})",
                sh.name
            );
        }
        let bytes = &data[old_off..old_off + old_sz];
        let align = (sh.addralign.max(1) as usize).clamp(1, 4096);
        while out.len() % align != 0 {
            out.push(0);
        }
        sh.offset = out.len() as u64;
        out.extend_from_slice(bytes);
        new_shdrs.push(sh);
    }

    while out.len() % align_pad != 0 {
        out.push(0);
    }
    let new_shoff = out.len();

    for sh in &new_shdrs {
        if is_64 {
            let mut raw = [0u8; 64];
            w_u32(&mut raw, 0, sh.name_off, le);
            w_u32(&mut raw, 4, sh.sh_type, le);
            w_u64(&mut raw, 8, sh.flags, le);
            w_u64(&mut raw, 16, sh.addr, le);
            w_u64(&mut raw, 24, sh.offset, le);
            w_u64(&mut raw, 32, sh.size, le);
            w_u32(&mut raw, 40, sh.link, le);
            w_u32(&mut raw, 44, sh.info, le);
            w_u64(&mut raw, 48, sh.addralign, le);
            w_u64(&mut raw, 56, sh.entsize, le);
            out.extend_from_slice(&raw);
        } else {
            let mut raw = [0u8; 40];
            w_u32(&mut raw, 0, sh.name_off, le);
            w_u32(&mut raw, 4, sh.sh_type, le);
            w_u32(&mut raw, 8, sh.flags as u32, le);
            w_u32(&mut raw, 12, sh.addr as u32, le);
            w_u32(&mut raw, 16, sh.offset as u32, le);
            w_u32(&mut raw, 20, sh.size as u32, le);
            w_u32(&mut raw, 24, sh.link, le);
            w_u32(&mut raw, 28, sh.info, le);
            w_u32(&mut raw, 32, sh.addralign as u32, le);
            w_u32(&mut raw, 36, sh.entsize as u32, le);
            out.extend_from_slice(&raw);
        }
    }

    // Patch ELF header
    if is_64 {
        w_u64(&mut out, 40, new_shoff as u64, le);
        w_u16(&mut out, 60, new_shdrs.len() as u16, le);
        w_u16(&mut out, 62, new_shstrndx as u16, le);
    } else {
        w_u32(&mut out, 32, new_shoff as u32, le);
        w_u16(&mut out, 48, new_shdrs.len() as u16, le);
        w_u16(&mut out, 50, new_shstrndx as u16, le);
    }

    Ok(out)
}

fn parse_shdr(data: &[u8], off: usize, is_64: bool, le: bool) -> Shdr {
    if is_64 {
        Shdr {
            name_off: r_u32(data, off, le),
            sh_type: r_u32(data, off + 4, le),
            flags: r_u64(data, off + 8, le),
            addr: r_u64(data, off + 16, le),
            offset: r_u64(data, off + 24, le),
            size: r_u64(data, off + 32, le),
            link: r_u32(data, off + 40, le),
            info: r_u32(data, off + 44, le),
            addralign: r_u64(data, off + 48, le),
            entsize: r_u64(data, off + 56, le),
            name: String::new(),
        }
    } else {
        Shdr {
            name_off: r_u32(data, off, le),
            sh_type: r_u32(data, off + 4, le),
            flags: r_u32(data, off + 8, le) as u64,
            addr: r_u32(data, off + 12, le) as u64,
            offset: r_u32(data, off + 16, le) as u64,
            size: r_u32(data, off + 20, le) as u64,
            link: r_u32(data, off + 24, le),
            info: r_u32(data, off + 28, le),
            addralign: r_u32(data, off + 32, le) as u64,
            entsize: r_u32(data, off + 36, le) as u64,
            name: String::new(),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_identity_when_nothing_to_drop_on_minimal() {
        // Invalid tiny buffer must error, not panic
        let err = strip_bytes(&[0x7f, b'E', b'L', b'F', 2, 1], StripOptions::default());
        assert!(err.is_err());
    }
}
