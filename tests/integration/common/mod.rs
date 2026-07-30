//! Shared helpers for integration / GNU differential tests.
//!
//! Set `OXIDE_COMPARE_GNU=1` to require system GNU tools for comparisons.

use std::path::{Path, PathBuf};
use std::process::Command;

pub fn oxide_bin(name: &str) -> PathBuf {
    // Prefer cargo-built debug binaries
    let mut p = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    p.push("target");
    p.push("debug");
    p.push(name);
    if p.exists() {
        return p;
    }
    // Fallback: PATH
    PathBuf::from(name)
}

pub fn gnu_available(tool: &str) -> bool {
    Command::new(tool)
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn require_gnu() -> bool {
    std::env::var("OXIDE_COMPARE_GNU").ok().as_deref() == Some("1")
}

pub fn run_capture(bin: &Path, args: &[&str]) -> (i32, String, String) {
    let out = Command::new(bin)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));
    let code = out.status.code().unwrap_or(1);
    let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
    (code, stdout, stderr)
}

/// Compare structural shapes (line counts / non-empty), not byte-identical output.
pub fn lines_nonempty(s: &str) -> usize {
    s.lines().filter(|l| !l.trim().is_empty()).count()
}
