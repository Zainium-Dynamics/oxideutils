//! `-lNAME` / `-LDIR` library search, GNU ld `GROUP()` script following, and
//! shared-object dynamic-symbol export scanning.
//!
//! Mirrors `ld/ldfile.c` (`ldfile_open_library_file`) at a reduced scale.
//!
//! Deliberately carries **no hardcoded FHS/glibc paths** (no `/usr/lib64`,
//! no `/lib/x86_64-linux-gnu`, ...) — those are glibc/Debian-multilib
//! assumptions that are simply wrong for a musl target (musl doesn't split
//! `lib`/`lib64`) and wrong for a from-scratch OS with its own sysroot
//! layout (e.g. ZainiumOS's own tree isn't under `/usr` at all). Every
//! search directory comes from an explicit `-L`, or from `--sysroot`
//! (`{sysroot}/lib`, `{sysroot}/usr/lib`) — the same mechanism real
//! cross-toolchains use instead of baking in a host's library layout. With
//! neither, resolution only considers `-L` dirs; it will not silently fall
//! back to the *build host's* libraries, which would be a correctness trap
//! when cross-linking for a different libc/OS.

use anyhow::{bail, Result};
use object::read::{Object, ObjectSection, ObjectSymbol};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub enum ResolvedInput {
    /// A relocatable object or a static archive — inspected structurally.
    ObjectOrArchive(PathBuf),
    /// A real ELF shared object — recorded for `DT_NEEDED`, not linked in.
    SharedObject(PathBuf),
}

/// Resolve `-lNAME` to a file on disk: `-L` dirs first (in given order),
/// then `{sysroot}/lib` and `{sysroot}/usr/lib` if a sysroot was given.
/// Prefers `.so` unless `static_only`.
pub fn find_library(
    name: &str,
    search_dirs: &[PathBuf],
    static_only: bool,
    sysroot: Option<&Path>,
) -> Result<PathBuf> {
    let mut dirs: Vec<PathBuf> = search_dirs.to_vec();
    if let Some(root) = sysroot {
        dirs.push(root.join("lib"));
        dirs.push(root.join("usr/lib"));
    }

    for dir in &dirs {
        if !static_only {
            let so = dir.join(format!("lib{name}.so"));
            if so.is_file() {
                return Ok(so);
            }
        }
        let a = dir.join(format!("lib{name}.a"));
        if a.is_file() {
            return Ok(a);
        }
    }
    bail!("cannot find -l{name}");
}

/// Detect and expand a GNU-ld `GROUP(...)` linker script such as glibc's
/// `libc.so` (`/* GNU ld script */ OUTPUT_FORMAT(...) GROUP ( a b c )`).
/// Returns `None` if `path` is a real ELF file rather than a text script.
pub fn expand_group_script(
    path: &Path,
    sysroot: Option<&Path>,
) -> Result<Option<Vec<ResolvedInput>>> {
    let bytes = fs::read(path)?;
    if bytes.first() == Some(&0x7f) && bytes.get(1..4) == Some(b"ELF") {
        return Ok(None);
    }
    let Ok(text) = std::str::from_utf8(&bytes) else {
        return Ok(None);
    };
    let Some(group_start) = text.find("GROUP") else {
        return Ok(None);
    };
    let Some(open_rel) = text[group_start..].find('(') else {
        return Ok(None);
    };
    let open = group_start + open_rel;
    let close = find_matching_paren(text, open)?;
    let inner = &text[open + 1..close];
    // `AS_NEEDED ( ... )` is a nested grouping we don't distinguish from a
    // plain member list (we don't implement "only record DT_NEEDED if
    // actually referenced" — everything named ends up as either a merged
    // object/archive or a recorded `DT_NEEDED`, which is a safe superset).
    let inner = inner.replace("AS_NEEDED", " ").replace(['(', ')'], " ");

    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    let mut out = Vec::new();
    for tok in inner.split_whitespace() {
        let resolved = if let Some(lib) = tok.strip_prefix("-l") {
            find_library(lib, &[], false, sysroot)?
        } else if tok.starts_with('/') {
            PathBuf::from(tok)
        } else {
            base_dir.join(tok)
        };
        if !resolved.is_file() {
            continue;
        }
        if is_elf_shared_object(&resolved)? {
            out.push(ResolvedInput::SharedObject(resolved));
        } else {
            out.push(ResolvedInput::ObjectOrArchive(resolved));
        }
    }
    Ok(Some(out))
}

/// Find the `)` matching the `(` at byte offset `open`, accounting for
/// nesting (glibc's `libc.so` script nests `AS_NEEDED ( ... )` inside the
/// outer `GROUP ( ... )`).
fn find_matching_paren(text: &str, open: usize) -> Result<usize> {
    let mut depth = 0i32;
    for (i, c) in text[open..].char_indices() {
        match c {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(open + i);
                }
            }
            _ => {}
        }
    }
    bail!("unbalanced parentheses in linker script");
}

pub fn is_elf_shared_object(path: &Path) -> Result<bool> {
    let bytes = fs::read(path)?;
    let Ok(file) = object::File::parse(&*bytes) else {
        return Ok(false);
    };
    Ok(file.kind() == object::ObjectKind::Dynamic)
}

/// Exported (defined, non-local) dynamic symbol names of a real `.so`, plus
/// its `DT_SONAME` if present (falling back to the file's own basename).
pub struct SharedLibInfo {
    pub soname: String,
    pub exports: BTreeSet<String>,
}

pub fn scan_shared_object(path: &Path) -> Result<SharedLibInfo> {
    let bytes = fs::read(path)?;
    let file = object::File::parse(&*bytes)
        .map_err(|e| anyhow::anyhow!("{}: not an ELF shared object: {e}", path.display()))?;

    let mut exports = BTreeSet::new();
    for sym in file.dynamic_symbols() {
        if sym.is_undefined() {
            continue;
        }
        if let Ok(name) = sym.name() {
            if !name.is_empty() {
                exports.insert(name.to_string());
            }
        }
    }

    let soname = read_soname(&file, &bytes).unwrap_or_else(|| {
        path.file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string())
    });

    Ok(SharedLibInfo { soname, exports })
}

/// Manually walk `.dynamic` + `.dynstr` for `DT_SONAME` (tag 14); the `object`
/// crate's read API doesn't expose dynamic-section tags directly.
fn read_soname(file: &object::File, _bytes: &[u8]) -> Option<String> {
    use object::{elf, Endian, Endianness};

    let dynamic = file.sections().find(|s| s.name() == Ok(".dynamic"))?;
    let dynstr = file.sections().find(|s| s.name() == Ok(".dynstr"))?;
    let dyn_data = dynamic.data().ok()?;
    let str_data = dynstr.data().ok()?;
    let endian = Endianness::Little;
    let entsize = 16usize; // Elf64_Dyn { d_tag: u64, d_val: u64 }
    let mut off = 0;
    while off + entsize <= dyn_data.len() {
        let tag = endian.read_u64_bytes(dyn_data[off..off + 8].try_into().ok()?);
        let val = endian.read_u64_bytes(dyn_data[off + 8..off + 16].try_into().ok()?);
        if tag == elf::DT_NULL as u64 {
            break;
        }
        if tag == elf::DT_SONAME as u64 {
            let start = val as usize;
            let end = str_data[start..].iter().position(|&b| b == 0)? + start;
            return std::str::from_utf8(&str_data[start..end])
                .ok()
                .map(|s| s.to_string());
        }
        off += entsize;
    }
    None
}
