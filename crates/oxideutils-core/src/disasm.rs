//! Disassembly backends (GNU opcodes analogue).
//!
//! | Arch | Backend | Feature |
//! |------|---------|---------|
//! | x86 / x86_64 | **iced-x86** (gas) | `disasm` |
//! | AArch64 | **bad64** (pure Rust, `no_std`) | `disasm` |
//! | other | hex / `.byte` fallback | always |
//!
//! Set [`DisasmOptions::allow_hex_fallback`] to `false` to error on unsupported
//! arches instead of emitting hex (Phase E honesty).

use crate::error::{OxideError, Result};
use crate::prelude::*;
use core::fmt::Write as _;
use object::Architecture;

#[derive(Debug, Clone)]
pub struct DisasmOptions {
    /// Print raw instruction bytes next to mnemonics (GNU --show-raw-insn).
    pub show_raw_insn: bool,
    /// Do not skip long zero runs (GNU -z / --disassemble-zeroes).
    pub disassemble_zeroes: bool,
    /// Only disassemble addresses in [start, stop).
    pub start_address: Option<u64>,
    pub stop_address: Option<u64>,
    /// Max instruction width hint for formatting.
    pub insn_width: usize,
    /// If false, unsupported arches return an error instead of hex dump.
    pub allow_hex_fallback: bool,
}

impl Default for DisasmOptions {
    fn default() -> Self {
        Self {
            show_raw_insn: true,
            disassemble_zeroes: false,
            start_address: None,
            stop_address: None,
            insn_width: 7,
            allow_hex_fallback: true,
        }
    }
}

/// True when we have a real decoder for `arch` under current features.
pub fn has_real_backend(arch: Architecture) -> bool {
    if matches!(arch, Architecture::X86_64 | Architecture::I386) {
        return cfg!(feature = "disasm");
    }
    if matches!(arch, Architecture::Aarch64) {
        return cfg!(feature = "disasm-aarch64");
    }
    false
}

#[derive(Debug, Clone)]
pub struct DisassembledInsn {
    pub address: u64,
    pub bytes: Vec<u8>,
    pub text: String,
}

/// Disassemble a code buffer at `base_addr` for the given architecture.
pub fn disassemble(
    arch: Architecture,
    base_addr: u64,
    data: &[u8],
    opts: &DisasmOptions,
) -> Result<Vec<DisassembledInsn>> {
    #[cfg(feature = "disasm")]
    {
        if matches!(arch, Architecture::X86_64 | Architecture::I386) {
            return disassemble_x86(arch, base_addr, data, opts);
        }
    }
    #[cfg(feature = "disasm-aarch64")]
    {
        if matches!(arch, Architecture::Aarch64) {
            return disassemble_aarch64(base_addr, data, opts);
        }
    }
    if !opts.allow_hex_fallback {
        return Err(OxideError::NotImplemented(
            "disassembly backend not available for this architecture (set allow_hex_fallback or enable disasm)",
        ));
    }
    let _ = arch;
    Ok(disassemble_hex_fallback(base_addr, data, opts))
}

/// Format GNU objdump-ish lines for a section.
pub fn format_disassembly(
    section_name: &str,
    arch: Architecture,
    base_addr: u64,
    data: &[u8],
    symbols: &[(u64, String)],
    opts: &DisasmOptions,
) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "\nDisassembly of section {section_name}:").ok();

    let insns = disassemble(arch, base_addr, data, opts)?;
    let end = base_addr + data.len() as u64;

    // symbols in this section, sorted
    let mut syms: Vec<&(u64, String)> = symbols
        .iter()
        .filter(|(a, _)| *a >= base_addr && *a < end)
        .collect();
    syms.sort_by_key(|(a, _)| *a);

    let mut sym_i = 0usize;
    for insn in &insns {
        while sym_i < syms.len() && syms[sym_i].0 <= insn.address {
            let (a, n) = syms[sym_i];
            if *a == insn.address || (sym_i + 1 < syms.len() && syms[sym_i + 1].0 > insn.address) {
                // print label when we hit its address
            }
            if *a == insn.address {
                writeln!(out, "\n{:016x} <{n}>:", a).ok();
            }
            if *a < insn.address {
                sym_i += 1;
                continue;
            }
            if *a == insn.address {
                sym_i += 1;
            }
            break;
        }
        // also emit symbols that land exactly on this address if missed
        for (a, n) in symbols {
            if *a == insn.address {
                // may duplicate; only if not just printed — keep simple
                let marker = format!("<{n}>");
                if !out.ends_with(&format!("{marker}:\n")) {
                    // skip heavy dedup
                }
            }
        }

        if opts.show_raw_insn {
            let hex: String = insn
                .bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            // pad raw bytes column
            let pad = opts.insn_width.saturating_mul(3).saturating_sub(1);
            let raw = format!("{hex:<pad$}");
            writeln!(out, "  {:x}:\t{raw}\t{}", insn.address, insn.text).ok();
        } else {
            writeln!(out, "  {:x}:\t{}", insn.address, insn.text).ok();
        }
    }
    Ok(out)
}

/// Emit symbol labels then instructions with cleaner GNU-like flow.
pub fn format_disassembly_with_labels(
    section_name: &str,
    arch: Architecture,
    base_addr: u64,
    data: &[u8],
    symbols: &[(u64, String)],
    opts: &DisasmOptions,
) -> Result<String> {
    let mut out = String::new();
    writeln!(out, "\nDisassembly of section {section_name}:").ok();

    let insns = disassemble(arch, base_addr, data, opts)?;
    let end = base_addr.saturating_add(data.len() as u64);

    let mut labels: Vec<(u64, &str)> = symbols
        .iter()
        .filter(|(a, _)| *a >= base_addr && *a < end)
        .map(|(a, n)| (*a, n.as_str()))
        .collect();
    labels.sort_by_key(|(a, _)| *a);
    labels.dedup_by_key(|(a, _)| *a);

    let mut li = 0usize;
    for insn in &insns {
        while li < labels.len() && labels[li].0 < insn.address {
            li += 1;
        }
        if li < labels.len() && labels[li].0 == insn.address {
            writeln!(out, "\n{:016x} <{}>:", labels[li].0, labels[li].1).ok();
            li += 1;
        }

        if opts.show_raw_insn {
            let hex: String = insn
                .bytes
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<Vec<_>>()
                .join(" ");
            let col = 21usize;
            writeln!(
                out,
                "  {:x}:\t{:<width$}\t{}",
                insn.address,
                hex,
                insn.text,
                width = col
            )
            .ok();
        } else {
            writeln!(out, "  {:x}:\t{}", insn.address, insn.text).ok();
        }
    }
    Ok(out)
}

#[cfg(feature = "disasm")]
fn disassemble_x86(
    arch: Architecture,
    base_addr: u64,
    data: &[u8],
    opts: &DisasmOptions,
) -> Result<Vec<DisassembledInsn>> {
    use iced_x86::{Decoder, DecoderOptions, Formatter, GasFormatter, Instruction};

    let bitness = match arch {
        Architecture::X86_64 => 64,
        Architecture::I386 => 32,
        _ => 64,
    };

    let mut decoder = Decoder::with_ip(bitness, data, base_addr, DecoderOptions::NONE);
    let mut formatter = GasFormatter::new();
    // AT&T / gas style closer to GNU objdump default on Linux
    formatter.options_mut().set_uppercase_hex(false);
    formatter
        .options_mut()
        .set_gas_show_mnemonic_size_suffix(true);
    formatter.options_mut().set_first_operand_char_index(8);

    let mut out = Vec::new();
    let mut instruction = Instruction::default();
    let mut zero_run = 0usize;

    while decoder.can_decode() {
        decoder.decode_out(&mut instruction);
        let ip = instruction.ip();
        let len = instruction.len();
        if len == 0 {
            break;
        }

        if let Some(sa) = opts.start_address {
            if ip + len as u64 <= sa {
                continue;
            }
        }
        if let Some(ea) = opts.stop_address {
            if ip >= ea {
                break;
            }
        }

        let offset = (ip - base_addr) as usize;
        let end = (offset + len).min(data.len());
        if offset >= data.len() {
            break;
        }
        let bytes = data[offset..end].to_vec();

        if !opts.disassemble_zeroes && bytes.iter().all(|b| *b == 0) {
            zero_run += bytes.len();
            if zero_run >= 16 {
                // skip remaining zeros in one go
                continue;
            }
        } else {
            zero_run = 0;
        }

        let mut text = String::new();
        formatter.format(&instruction, &mut text);

        out.push(DisassembledInsn {
            address: ip,
            bytes,
            text,
        });
    }

    // collapse leading skipped zeros: insert a single "..." marker if we jumped
    if !opts.disassemble_zeroes {
        out = collapse_zero_skips(out, base_addr, data, opts);
    }

    Ok(out)
}

#[cfg(any(feature = "disasm", feature = "disasm-aarch64"))]
fn collapse_zero_skips(
    insns: Vec<DisassembledInsn>,
    _base: u64,
    _data: &[u8],
    _opts: &DisasmOptions,
) -> Vec<DisassembledInsn> {
    // Already skipped during decode for long zero runs partially;
    // filter pure-nop zero groups of 16+ consecutive zero bytes shown as db
    let mut out = Vec::with_capacity(insns.len());
    let mut i = 0;
    while i < insns.len() {
        let insn = &insns[i];
        if insn.bytes.iter().all(|b| *b == 0) {
            let start = i;
            let mut total = 0usize;
            while i < insns.len() && insns[i].bytes.iter().all(|b| *b == 0) {
                total += insns[i].bytes.len();
                i += 1;
            }
            if total >= 16 {
                out.push(DisassembledInsn {
                    address: insns[start].address,
                    bytes: vec![],
                    text: "...".into(),
                });
            } else {
                out.extend(insns[start..i].iter().cloned());
            }
        } else {
            out.push(insn.clone());
            i += 1;
        }
    }
    out
}

#[cfg(feature = "disasm-aarch64")]
fn disassemble_aarch64(
    base_addr: u64,
    data: &[u8],
    opts: &DisasmOptions,
) -> Result<Vec<DisassembledInsn>> {
    // AArch64 instructions are 4-byte fixed size (ignoring SVE stream for dump).
    let mut out = Vec::new();
    let mut off = 0usize;
    let mut zero_run = 0usize;

    while off + 4 <= data.len() {
        let ip = base_addr + off as u64;
        if let Some(sa) = opts.start_address {
            if ip + 4 <= sa {
                off += 4;
                continue;
            }
        }
        if let Some(ea) = opts.stop_address {
            if ip >= ea {
                break;
            }
        }

        let word_bytes = &data[off..off + 4];
        if !opts.disassemble_zeroes && word_bytes.iter().all(|b| *b == 0) {
            zero_run += 4;
            if zero_run >= 16 {
                off += 4;
                continue;
            }
        } else {
            zero_run = 0;
        }

        let insn_word =
            u32::from_le_bytes([word_bytes[0], word_bytes[1], word_bytes[2], word_bytes[3]]);
        let text = match bad64::decode(insn_word, ip) {
            Ok(decoded) => format!("{decoded}"),
            Err(_) => format!(".inst 0x{insn_word:08x} ; undefined"),
        };

        out.push(DisassembledInsn {
            address: ip,
            bytes: word_bytes.to_vec(),
            text,
        });
        off += 4;
    }

    // trailing unaligned tail
    if off < data.len() {
        let rest = &data[off..];
        out.push(DisassembledInsn {
            address: base_addr + off as u64,
            bytes: rest.to_vec(),
            text: format!(
                ".byte {}",
                rest.iter()
                    .map(|b| format!("0x{b:02x}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        });
    }

    if !opts.disassemble_zeroes {
        out = collapse_zero_skips(out, base_addr, data, opts);
    }
    Ok(out)
}

fn disassemble_hex_fallback(
    base_addr: u64,
    data: &[u8],
    opts: &DisasmOptions,
) -> Vec<DisassembledInsn> {
    let mut out = Vec::new();
    let mut off = 0usize;
    while off < data.len() {
        let addr = base_addr + off as u64;
        if let Some(ea) = opts.stop_address {
            if addr >= ea {
                break;
            }
        }
        if let Some(sa) = opts.start_address {
            if addr < sa {
                off += 1;
                continue;
            }
        }
        let take = 4.min(data.len() - off);
        let bytes = data[off..off + take].to_vec();
        if !opts.disassemble_zeroes && bytes.iter().all(|b| *b == 0) {
            let mut z = off;
            while z < data.len() && data[z] == 0 {
                z += 1;
            }
            if z - off >= 16 {
                out.push(DisassembledInsn {
                    address: addr,
                    bytes: vec![],
                    text: "...".into(),
                });
                off = z;
                continue;
            }
        }
        let text = format!(
            ".byte {}",
            bytes
                .iter()
                .map(|b| format!("0x{b:02x}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
        out.push(DisassembledInsn {
            address: addr,
            bytes,
            text,
        });
        off += take;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "disasm")]
    #[test]
    fn decode_nop_ret_x64() {
        // nop; ret
        let data = [0x90, 0xc3];
        let insns = disassemble(
            Architecture::X86_64,
            0x1000,
            &data,
            &DisasmOptions::default(),
        )
        .unwrap();
        assert!(insns.len() >= 2);
        assert!(insns[0].text.contains("nop") || insns[0].text.contains("xchg"));
        assert!(insns[1].text.contains("ret"));
    }

    #[cfg(feature = "disasm-aarch64")]
    #[test]
    fn decode_nop_aarch64() {
        // nop = 0xd503201f little-endian bytes
        let data = [0x1f, 0x20, 0x03, 0xd5];
        let insns = disassemble(
            Architecture::Aarch64,
            0x1000,
            &data,
            &DisasmOptions::default(),
        )
        .unwrap();
        assert_eq!(insns.len(), 1);
        assert!(
            insns[0].text.to_ascii_lowercase().contains("nop"),
            "got {}",
            insns[0].text
        );
    }

    #[test]
    fn has_backend_flags() {
        assert!(has_real_backend(Architecture::X86_64) || !cfg!(feature = "disasm"));
    }
}
