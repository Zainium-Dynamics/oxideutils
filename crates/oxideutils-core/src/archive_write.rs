//! GNU `ar` archive create / modify / symbol-index (ranlib).
//!
//! Supports the classic SVR4/GNU thin-headerless archive:
//! - magic `!<arch>\n`
//! - 60-byte member headers + even-padded payloads
//! - long names via `//` string table (`/N` name field)
//! - symbol map member `/` (32-bit BE offsets) when requested

use crate::archive::{is_archive, OxideArchive};
use crate::error::{OxideError, Result};
use crate::utils::atomic_write;
use object::{Object, ObjectSymbol};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAGIC: &[u8] = b"!<arch>\n";
const TERMINATOR: &[u8] = b"`\n";

/// One real object/data member (excludes `/`, `//`, `/SYM64/`).
#[derive(Debug, Clone)]
pub struct ArchiveMemberData {
    pub name: String,
    pub data: Vec<u8>,
    pub mtime: u64,
    pub mode: u32,
}

/// In-memory archive builder / mutator.
#[derive(Debug, Clone, Default)]
pub struct ArchiveBuilder {
    pub members: Vec<ArchiveMemberData>,
    /// When true, use mtime=0 uid=0 gid=0 (GNU `D` / deterministic).
    pub deterministic: bool,
    /// Emit GNU symbol index (`/` member) when writing.
    pub write_symbol_index: bool,
}

impl ArchiveBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn deterministic(mut self, on: bool) -> Self {
        self.deterministic = on;
        self
    }

    pub fn with_symbol_index(mut self, on: bool) -> Self {
        self.write_symbol_index = on;
        self
    }

    /// Parse an existing GNU/BSD archive into editable members.
    pub fn from_bytes(path: impl AsRef<str>, data: &[u8]) -> Result<Self> {
        if !is_archive(data) {
            return Err(OxideError::format(
                path.as_ref(),
                "not a recognised ar archive",
            ));
        }
        if data.starts_with(b"!<thin>\n") {
            return Err(OxideError::tool(
                "ar",
                "thin archives are read-only in this version; cannot rewrite",
            ));
        }
        let arch = OxideArchive::parse(path.as_ref(), data)?;
        let mut members = Vec::new();
        for m in &arch.members {
            // OxideArchive already skips symbol/string tables when using object crate?
            // object::read::archive includes all; filter pseudo-members.
            if m.name == "/" || m.name == "//" || m.name == "/SYM64/" || m.name.is_empty() {
                continue;
            }
            members.push(ArchiveMemberData {
                name: m.name.clone(),
                data: arch.member_data(m).to_vec(),
                mtime: 0,
                mode: 0o644,
            });
        }
        Ok(Self {
            members,
            deterministic: false,
            write_symbol_index: false,
        })
    }

    pub fn from_file(path: &Path) -> Result<Self> {
        let data = fs::read(path).map_err(|e| OxideError::io_path(path, e))?;
        Self::from_bytes(path.display().to_string(), &data)
    }

    /// Replace member with same basename, or append.
    pub fn replace_or_add(&mut self, name: String, data: Vec<u8>) {
        let base = member_basename(&name);
        if let Some(slot) = self.members.iter_mut().find(|m| member_basename(&m.name) == base)
        {
            slot.name = base.to_string();
            slot.data = data;
            slot.mtime = now_epoch();
            slot.mode = 0o644;
        } else {
            self.members.push(ArchiveMemberData {
                name: base.to_string(),
                data,
                mtime: now_epoch(),
                mode: 0o644,
            });
        }
    }

    pub fn add_file(&mut self, path: &Path) -> Result<()> {
        let data = fs::read(path).map_err(|e| OxideError::io_path(path, e))?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| OxideError::tool("ar", "invalid member path"))?
            .to_string();
        self.replace_or_add(name, data);
        Ok(())
    }

    /// Quick-append without replacing existing names.
    pub fn append_file(&mut self, path: &Path) -> Result<()> {
        let data = fs::read(path).map_err(|e| OxideError::io_path(path, e))?;
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(|| OxideError::tool("ar", "invalid member path"))?
            .to_string();
        self.members.push(ArchiveMemberData {
            name,
            data,
            mtime: now_epoch(),
            mode: 0o644,
        });
        Ok(())
    }

    pub fn delete(&mut self, names: &[String]) -> usize {
        let want: Vec<String> = names.iter().map(|n| member_basename(n).to_string()).collect();
        let before = self.members.len();
        self.members
            .retain(|m| !want.iter().any(|n| n == member_basename(&m.name)));
        before - self.members.len()
    }

    /// Serialise to GNU ar bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        // Build long-name string table if needed
        let mut strtab = Vec::new();
        let mut name_fields: Vec<NameField> = Vec::with_capacity(self.members.len());

        for m in &self.members {
            let nm = member_basename(&m.name);
            // GNU short form: name + '/' fits in 16 bytes
            if nm.len() < 16 {
                let mut field = format!("{nm}/");
                while field.len() < 16 {
                    field.push(' ');
                }
                name_fields.push(NameField::Inline(field));
            } else {
                let off = strtab.len();
                strtab.extend_from_slice(nm.as_bytes());
                strtab.push(b'/');
                strtab.push(b'\n');
                name_fields.push(NameField::Long(off));
            }
        }

        // First pass: layout without symbol index to compute member file offsets
        // We need offsets of each real member for the symbol map; offsets include
        // optional `/` and `//` members that precede them.

        // Pre-build symbol index body if requested
        let sym_body = if self.write_symbol_index {
            Some(build_gnu_symbol_map(&self.members)?)
        } else {
            None
        };

        let mut out = Vec::new();
        out.extend_from_slice(MAGIC);

        // Placeholder plan: write / then // then members; track member offsets
        let mut member_offsets: Vec<u64> = Vec::with_capacity(self.members.len());

        // We'll do two-phase for symbol map: first layout sizes, then write.
        // Simpler approach: write without index first into a temp buffer of members only,
        // then prepend index with correct offsets.

        let mut body = Vec::new();
        if !strtab.is_empty() {
            write_member_header(
                &mut body,
                b"//              ",
                0,
                0,
                0,
                0,
                strtab.len() as u64,
            );
            body.extend_from_slice(&strtab);
            pad_even(&mut body);
        }

        for (i, m) in self.members.iter().enumerate() {
            // offset of this member header relative to start of archive file
            // will be adjusted if symbol index is prepended
            member_offsets.push(body.len() as u64);
            let name_field = match &name_fields[i] {
                NameField::Inline(s) => {
                    let mut b = [b' '; 16];
                    let raw = s.as_bytes();
                    b[..raw.len().min(16)].copy_from_slice(&raw[..raw.len().min(16)]);
                    b
                }
                NameField::Long(off) => {
                    let s = format!("/{off}");
                    let mut b = [b' '; 16];
                    let raw = s.as_bytes();
                    b[..raw.len().min(16)].copy_from_slice(&raw[..raw.len().min(16)]);
                    b
                }
            };
            let mtime = if self.deterministic { 0 } else { m.mtime };
            let mode = if self.deterministic { 0o644 } else { m.mode };
            write_member_header_raw(
                &mut body,
                &name_field,
                mtime,
                0,
                0,
                mode,
                m.data.len() as u64,
            );
            body.extend_from_slice(&m.data);
            pad_even(&mut body);
        }

        // Prepend symbol index with corrected absolute member offsets.
        // `member_offsets` are relative to start of `body`. Final layout:
        // MAGIC + [optional `/` symbol member] + body
        if let Some(sym_entries) = sym_body {
            let names_blob: Vec<u8> = {
                let mut n = Vec::new();
                for (sym, _) in &sym_entries {
                    n.extend_from_slice(sym.as_bytes());
                    n.push(0);
                }
                n
            };
            let count = sym_entries.len() as u32;
            let payload_len = 4 + count as usize * 4 + names_blob.len();
            let payload_padded = payload_len + (payload_len % 2);
            let sym_member_total = 60 + payload_padded;
            let base = MAGIC.len() + sym_member_total;

            let mut payload = Vec::with_capacity(payload_len);
            payload.extend_from_slice(&count.to_be_bytes());
            for (_sym, mem_idx) in &sym_entries {
                let abs = (base as u64 + member_offsets[*mem_idx]) as u32;
                payload.extend_from_slice(&abs.to_be_bytes());
            }
            payload.extend_from_slice(&names_blob);

            let mut prefix = Vec::new();
            write_member_header(
                &mut prefix,
                b"/               ",
                0,
                0,
                0,
                0,
                payload.len() as u64,
            );
            prefix.extend_from_slice(&payload);
            pad_even(&mut prefix);

            out.extend_from_slice(&prefix);
            out.extend_from_slice(&body);
        } else {
            out.extend_from_slice(&body);
        }

        Ok(out)
    }

    pub fn write_to(&self, path: &Path) -> Result<()> {
        let bytes = self.to_bytes()?;
        atomic_write(path, &bytes, None)
    }
}

enum NameField {
    Inline(String),
    Long(usize),
}

fn member_basename(name: &str) -> &str {
    Path::new(name)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(name)
}

fn now_epoch() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn pad_even(buf: &mut Vec<u8>) {
    if buf.len() % 2 != 0 {
        buf.push(b'\n');
    }
}

fn write_member_header(
    out: &mut Vec<u8>,
    name16: &[u8],
    mtime: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    size: u64,
) {
    let mut name = [b' '; 16];
    let n = name16.len().min(16);
    name[..n].copy_from_slice(&name16[..n]);
    write_member_header_raw(out, &name, mtime, uid, gid, mode, size);
}

fn write_member_header_raw(
    out: &mut Vec<u8>,
    name: &[u8; 16],
    mtime: u64,
    uid: u32,
    gid: u32,
    mode: u32,
    size: u64,
) {
    out.extend_from_slice(name);
    // date 12, uid 6, gid 6, mode 8, size 10, terminator 2
    out.extend_from_slice(format!("{mtime:<12}").as_bytes());
    out.extend_from_slice(format!("{uid:<6}").as_bytes());
    out.extend_from_slice(format!("{gid:<6}").as_bytes());
    out.extend_from_slice(format!("{mode:<8o}").as_bytes());
    out.extend_from_slice(format!("{size:<10}").as_bytes());
    out.extend_from_slice(TERMINATOR);
}

/// Collect (symbol_name, member_index) for defined global/weak symbols.
fn build_gnu_symbol_map(members: &[ArchiveMemberData]) -> Result<Vec<(String, usize)>> {
    // BTreeMap keeps sorted unique symbols (first member wins)
    let mut map: BTreeMap<String, usize> = BTreeMap::new();
    for (idx, m) in members.iter().enumerate() {
        let Ok(obj) = object::File::parse(m.data.as_slice()) else {
            continue;
        };
        for sym in obj.symbols() {
            if sym.is_undefined() || sym.is_local() {
                continue;
            }
            let Ok(name) = sym.name() else { continue };
            if name.is_empty() || name.starts_with(".L") {
                continue;
            }
            // Prefer first definition
            map.entry(name.to_string()).or_insert(idx);
        }
    }
    Ok(map.into_iter().collect())
}

/// High-level archive operations matching GNU letter keys.
#[derive(Debug, Clone)]
pub struct ArOperation {
    pub delete: bool,
    pub print: bool,
    pub quick_append: bool,
    pub replace: bool,
    pub table: bool,
    pub extract: bool,
    pub symbol_index: bool,
    pub create: bool,
    pub verbose: bool,
    pub deterministic: bool,
}

impl ArOperation {
    pub fn parse_key(key: &str) -> Result<Self> {
        let key = key.trim_start_matches('-');
        if key.is_empty() {
            return Err(OxideError::tool("ar", "missing operation key"));
        }
        let mut op = Self {
            delete: false,
            print: false,
            quick_append: false,
            replace: false,
            table: false,
            extract: false,
            symbol_index: false,
            create: false,
            verbose: false,
            deterministic: false,
        };
        let mut has_op = false;
        for c in key.chars() {
            match c {
                'd' => {
                    op.delete = true;
                    has_op = true;
                }
                'p' => {
                    op.print = true;
                    has_op = true;
                }
                'q' => {
                    op.quick_append = true;
                    has_op = true;
                }
                'r' => {
                    op.replace = true;
                    has_op = true;
                }
                't' => {
                    op.table = true;
                    has_op = true;
                }
                'x' => {
                    op.extract = true;
                    has_op = true;
                }
                's' => {
                    op.symbol_index = true;
                    // `s` alone is ranlib-like
                    has_op = true;
                }
                'c' => op.create = true,
                'v' => op.verbose = true,
                'D' => op.deterministic = true,
                'u' | 'a' | 'b' | 'i' | 'N' | 'o' | 'O' | 'P' | 'S' | 'T' | 'f' | 'l' | 'M' => {
                    // accepted, ignored for now
                }
                'V' => {}
                other => {
                    return Err(OxideError::tool(
                        "ar",
                        format!("unknown modifier/operation '{other}'"),
                    ));
                }
            }
        }
        if !has_op {
            return Err(OxideError::tool(
                "ar",
                "one of d, p, q, r, t, x, s required",
            ));
        }
        Ok(op)
    }
}

/// Run a GNU-style ar operation.
pub fn run_ar(
    op: &ArOperation,
    archive: &Path,
    files: &[PathBuf],
    member_names: &[String],
) -> Result<()> {
    let exists = archive.exists();
    let read_only = (op.table || op.print || op.extract)
        && !op.replace
        && !op.quick_append
        && !op.delete
        && !(op.symbol_index && !op.table && !op.print && !op.extract);

    // Pure read ops (t/p/x) — and t/p/x with modifiers that don't mutate.
    if op.table || op.print || op.extract {
        if !exists {
            return Err(OxideError::io_path(
                archive,
                std::io::Error::new(std::io::ErrorKind::NotFound, "No such file"),
            ));
        }
        let data = fs::read(archive).map_err(|e| OxideError::io_path(archive, e))?;
        let arch = OxideArchive::parse(archive.display().to_string(), &data)?;
        if op.table {
            for m in &arch.members {
                if is_special_member(&m.name) {
                    continue;
                }
                if op.verbose {
                    println!("{:>10} {}", m.size, m.name);
                } else {
                    println!("{}", m.name);
                }
            }
        }
        if op.print {
            use std::io::Write;
            for m in &arch.members {
                if is_special_member(&m.name) {
                    continue;
                }
                if !member_filter_ok(member_names, &m.name) {
                    continue;
                }
                let _ = std::io::stdout().write_all(arch.member_data(m));
            }
        }
        if op.extract {
            for m in &arch.members {
                if is_special_member(&m.name) {
                    continue;
                }
                if !member_filter_ok(member_names, &m.name) {
                    continue;
                }
                let name = PathBuf::from(&m.name)
                    .file_name()
                    .map(|s| s.to_os_string())
                    .unwrap_or_else(|| m.name.clone().into());
                fs::write(&name, arch.member_data(m))
                    .map_err(|e| OxideError::io(name.to_string_lossy(), e))?;
                if op.verbose {
                    eprintln!("x - {}", name.to_string_lossy());
                }
            }
        }
        if read_only {
            return Ok(());
        }
        // Fall through only if combined with mutating ops (unusual).
    }

    // Mutating: r / q / d / s (ranlib)
    if !(exists || op.replace || op.quick_append) {
        return Err(OxideError::tool(
            "ar",
            format!("{}: No such file (use r/q to create)", archive.display()),
        ));
    }

    let mut builder = if exists {
        ArchiveBuilder::from_file(archive)?
    } else {
        ArchiveBuilder::new()
    };
    builder.deterministic = op.deterministic;
    // Emit symbol index for static-lib workflows and explicit `s`.
    builder.write_symbol_index = op.symbol_index || op.replace || op.quick_append;

    if op.delete {
        let names: Vec<String> = if !member_names.is_empty() {
            member_names.to_vec()
        } else {
            files
                .iter()
                .filter_map(|p| p.file_name().and_then(|s| s.to_str()).map(|s| s.to_string()))
                .collect()
        };
        let n = builder.delete(&names);
        if op.verbose {
            eprintln!("d - removed {n} member(s)");
        }
        // Preserve index presence after delete when it was requested historically
        builder.write_symbol_index = true;
    }

    if op.replace {
        for f in files {
            if op.verbose {
                eprintln!("r - {}", f.display());
            }
            builder.add_file(f)?;
        }
    }

    if op.quick_append {
        for f in files {
            if op.verbose {
                eprintln!("q - {}", f.display());
            }
            builder.append_file(f)?;
        }
    }

    if op.symbol_index {
        builder.write_symbol_index = true;
    }

    if op.replace || op.quick_append || op.symbol_index || op.delete {
        builder.write_to(archive)?;
    }

    Ok(())
}

fn is_special_member(name: &str) -> bool {
    name == "/" || name == "//" || name == "/SYM64/" || name.is_empty()
}

fn member_filter_ok(filter: &[String], name: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    filter
        .iter()
        .any(|n| member_basename(n) == member_basename(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_emptyish_create() {
        let mut b = ArchiveBuilder::new().deterministic(true).with_symbol_index(false);
        b.replace_or_add("foo.o".into(), b"hello world".to_vec());
        let bytes = b.to_bytes().unwrap();
        assert!(bytes.starts_with(MAGIC));
        let parsed = ArchiveBuilder::from_bytes("t.a", &bytes).unwrap();
        assert_eq!(parsed.members.len(), 1);
        assert_eq!(parsed.members[0].name, "foo.o");
        assert_eq!(parsed.members[0].data, b"hello world");
    }

    #[test]
    fn parse_key_rcs() {
        let op = ArOperation::parse_key("rcs").unwrap();
        assert!(op.replace && op.create && op.symbol_index);
    }
}
