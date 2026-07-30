//! Format-layer helpers.

use crate::prelude::*;
/// Human-readable byte size.
pub fn human_size(n: u64) -> String {
    const UNITS: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut v = n as f64;
    let mut i = 0;
    while v >= 1024.0 && i < UNITS.len() - 1 {
        v /= 1024.0;
        i += 1;
    }
    if i == 0 {
        format!("{n}{}", UNITS[i])
    } else {
        format!("{v:.1}{}", UNITS[i])
    }
}

/// ELF machine id to rough name (common subset).
pub fn elf_machine_name(m: u16) -> &'static str {
    match m {
        3 => "i386",
        8 => "mips",
        20 => "ppc",
        21 => "ppc64",
        40 => "arm",
        50 => "ia64",
        62 => "x86-64",
        183 => "aarch64",
        243 => "riscv",
        0 => "none",
        _ => "unknown",
    }
}
