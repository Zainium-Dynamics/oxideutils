//! Shared utilities — pure helpers work on `no_std`+`alloc`; file I/O is `std`.

use crate::error::{OxideError, Result};
use alloc::string::String;
#[cfg(feature = "std")]
use alloc::vec::Vec;

#[cfg(feature = "std")]
use std::fs::{self, File};
#[cfg(feature = "std")]
use std::io::Read;
#[cfg(feature = "std")]
use std::path::{Path, PathBuf};

/// Read entire file into memory (`std` only).
#[cfg(feature = "std")]
pub fn read_file(path: &Path) -> Result<Vec<u8>> {
    let mut f = File::open(path).map_err(|e| OxideError::io_path(path, e))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)
        .map_err(|e| OxideError::io_path(path, e))?;
    Ok(buf)
}

/// Memory-map a file when possible (`std` only).
#[cfg(feature = "std")]
pub fn map_file(path: &Path) -> Result<memmap2::Mmap> {
    let f = File::open(path).map_err(|e| OxideError::io_path(path, e))?;
    // SAFETY: file is open read-only; we do not mutate while mapped.
    unsafe { memmap2::Mmap::map(&f) }.map_err(|e| OxideError::io_path(path, e))
}

/// Parse an address string (`0x...`, decimal, or octal with leading 0).
pub fn parse_address(s: &str) -> Result<u64> {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16)
            .map_err(|_| OxideError::InvalidArgument(alloc::format!("invalid address: {s}")))
    } else if s.starts_with('0') && s.len() > 1 && s.chars().all(|c| c.is_digit(8)) {
        u64::from_str_radix(s, 8)
            .map_err(|_| OxideError::InvalidArgument(alloc::format!("invalid address: {s}")))
    } else {
        s.parse::<u64>()
            .map_err(|_| OxideError::InvalidArgument(alloc::format!("invalid address: {s}")))
    }
}

/// Demangle Rust (and C++ on std builds) symbol names when possible.
pub fn demangle_symbol(name: &str) -> String {
    if let Ok(sym) = rustc_demangle::try_demangle(name) {
        return alloc::format!("{sym:#}");
    }
    #[cfg(feature = "std")]
    {
        if name.starts_with("_Z") || name.starts_with("__Z") {
            if let Ok(sym) = cpp_demangle::Symbol::new(name) {
                return sym.to_string();
            }
        }
    }
    String::from(name)
}

/// Format bytes as classic hex dump (objdump -s style).
pub fn hex_dump(addr: u64, data: &[u8], width: usize) -> String {
    let width = width.max(1);
    let mut out = String::new();
    for (i, chunk) in data.chunks(width).enumerate() {
        let line_addr = addr + (i * width) as u64;
        use core::fmt::Write;
        let _ = write!(out, " {line_addr:04x} ");
        for (j, b) in chunk.iter().enumerate() {
            let _ = write!(out, "{b:02x}");
            if j % 4 == 3 {
                out.push(' ');
            }
        }
        let used = chunk.len();
        let pad = width - used;
        for j in 0..pad {
            out.push_str("  ");
            if (used + j) % 4 == 3 {
                out.push(' ');
            }
        }
        if !out.ends_with(' ') {
            out.push(' ');
        }
        out.push(' ');
        for b in chunk {
            let c = *b as char;
            if c.is_ascii_graphic() || c == ' ' {
                out.push(c);
            } else {
                out.push('.');
            }
        }
        out.push('\n');
    }
    out
}

/// Expand `@file` option files (GNU style) — `std` only.
#[cfg(feature = "std")]
pub fn expand_at_args(args: Vec<String>) -> Result<Vec<String>> {
    let mut out = Vec::new();
    for a in args {
        if let Some(path) = a.strip_prefix('@') {
            let content = read_file(Path::new(path))?;
            let text = String::from_utf8_lossy(&content);
            for token in text.split_whitespace() {
                out.push(token.to_string());
            }
        } else {
            out.push(a);
        }
    }
    Ok(out)
}

/// Collect input paths.
#[cfg(feature = "std")]
pub fn collect_inputs(paths: &[PathBuf]) -> Result<Vec<PathBuf>> {
    if paths.is_empty() {
        return Err(OxideError::NoInputFiles);
    }
    Ok(paths.to_vec())
}

/// Atomically write `data` to `path` (temp file in same directory + rename).
///
/// When `mode_from` is `Some`, copies permission bits from that path (best-effort).
#[cfg(feature = "std")]
pub fn atomic_write(path: &Path, data: &[u8], mode_from: Option<&Path>) -> Result<()> {
    use std::io::Write;

    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| OxideError::io_path(path, e))?;
        }
    }

    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let dir = parent.unwrap_or_else(|| Path::new("."));

    let mut tmp_name = path
        .file_name()
        .map(|s| s.to_os_string())
        .unwrap_or_else(|| "oxideutils.out".into());
    tmp_name.push(".tmp.");
    tmp_name.push(std::process::id().to_string());
    let tmp_path = dir.join(tmp_name);

    {
        let mut f = fs::File::create(&tmp_path).map_err(|e| OxideError::io_path(&tmp_path, e))?;
        f.write_all(data)
            .map_err(|e| OxideError::io_path(&tmp_path, e))?;
        f.sync_all()
            .map_err(|e| OxideError::io_path(&tmp_path, e))?;
    }

    if let Some(src) = mode_from {
        if let Ok(meta) = fs::metadata(src) {
            let _ = fs::set_permissions(&tmp_path, meta.permissions());
        }
    } else if let Ok(meta) = fs::metadata(path) {
        let _ = fs::set_permissions(&tmp_path, meta.permissions());
    }

    fs::rename(&tmp_path, path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        OxideError::io_path(path, e)
    })?;
    Ok(())
}

/// Program name from argv0 (for multicall) — `std` only.
#[cfg(feature = "std")]
pub fn program_name(argv0: &str) -> &str {
    Path::new(argv0)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or(argv0)
}

/// Strip tool prefixes for multicall dispatch — pure string (no_std OK).
pub fn tool_name_from_argv0(argv0: &str) -> &str {
    let base = {
        #[cfg(feature = "std")]
        {
            program_name(argv0)
        }
        #[cfg(not(feature = "std"))]
        {
            argv0.rsplit('/').next().unwrap_or(argv0)
        }
    };
    for prefix in ["oxide-", "ox-", "llvm-", "g"] {
        if let Some(rest) = base.strip_prefix(prefix) {
            if !rest.is_empty() {
                return rest;
            }
        }
    }
    base
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_address() {
        assert_eq!(parse_address("0x1000").unwrap(), 0x1000);
        assert_eq!(parse_address("4096").unwrap(), 4096);
    }

    #[test]
    fn demangle_rustish() {
        assert_eq!(demangle_symbol("main"), "main");
    }
}
