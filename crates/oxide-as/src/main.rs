//! oxide-as — GNU gas-compatible assembler (binutils 2.46.1 subset).
//!
//! - Directives: gas/read.c potable
//! - Encoding: tc-i386 AT&T subset with ELF relocations
//! - Output: ET_REL ELF64 x86_64 (like gas default)

mod encode;
mod macros;
mod parser;

use anyhow::{Context, Result};
use clap::Parser;
use encode::{encode_insn, PendingReloc, RelocKind};
use object::write::{Object, Relocation, StandardSection, Symbol, SymbolSection};
use object::{
    Architecture, BinaryFormat, Endianness, RelocationEncoding, RelocationFlags, RelocationKind,
    SectionKind, SymbolFlags, SymbolKind, SymbolScope,
};
use parser::{parse_assembly, Directive, SectionKind as AsmSection, Statement};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "oxide-as",
    about = "GNU gas-compatible assembler (x86_64 ELF) for ZainiumOS",
    version
)]
struct Cli {
    #[arg(value_name = "INPUT")]
    input: Option<PathBuf>,
    #[arg(short = 'o', value_name = "OUTPUT", default_value = "a.out")]
    output: PathBuf,
    #[arg(long = "64")]
    target_64: bool,
    #[arg(long = "32")]
    target_32: bool,
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if cli.verbose {
        println!("[*] oxide-as — gas 2.46.1 subset (relocs + multi-section)");
        let _ = (cli.target_32, cli.target_64);
    }

    let input_path = cli.input.unwrap_or_else(|| PathBuf::from("-"));
    let source_text = if input_path == PathBuf::from("-") {
        use std::io::Read;
        let mut buffer = String::new();
        std::io::stdin()
            .read_to_string(&mut buffer)
            .context("Failed to read assembly from stdin")?;
        buffer
    } else {
        fs::read_to_string(&input_path)
            .with_context(|| format!("Failed to read assembly file: {}", input_path.display()))?
    };

    let object_bytes = assemble_x86_64(&source_text, cli.verbose)?;
    if let Some(parent) = cli.output.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    fs::write(&cli.output, &object_bytes)
        .with_context(|| format!("Failed to write object file: {}", cli.output.display()))?;
    if cli.verbose {
        println!(
            "[OK] {} -> {} ({} bytes)",
            input_path.display(),
            cli.output.display(),
            object_bytes.len()
        );
    }
    Ok(())
}

#[derive(Clone)]
struct SecSym {
    name: String,
    offset: u64,
    global: bool,
    weak: bool,
}

struct SecRel {
    offset: u64,
    symbol: String,
    kind: RelocKind,
    addend: i64,
}

struct SecBuf {
    data: Vec<u8>,
    symbols: Vec<SecSym>,
    relocs: Vec<SecRel>,
    /// For .bss — size only (no data bytes written until link).
    bss_size: u64,
}

impl SecBuf {
    fn new() -> Self {
        Self {
            data: Vec::new(),
            symbols: Vec::new(),
            relocs: Vec::new(),
            bss_size: 0,
        }
    }

    fn len(&self) -> u64 {
        if self.bss_size > 0 {
            self.bss_size
        } else {
            self.data.len() as u64
        }
    }

    fn align_to(&mut self, align: u64, fill: u8) {
        if align <= 1 {
            return;
        }
        let mask = align - 1;
        if self.bss_size > 0 {
            while self.bss_size & mask != 0 {
                self.bss_size += 1;
            }
        } else {
            while (self.data.len() as u64) & mask != 0 {
                self.data.push(fill);
            }
        }
    }
}

/// Assemble source → relocatable ELF64 (ET_REL).
pub fn assemble_x86_64(source: &str, verbose: bool) -> Result<Vec<u8>> {
    let expanded = macros::preprocess(source);
    let statements = parse_assembly(&expanded);

    let mut sections: BTreeMap<String, SecBuf> = BTreeMap::new();
    for n in [".text", ".data", ".rodata", ".bss"] {
        sections.insert(n.to_string(), SecBuf::new());
    }

    let mut current = ".text".to_string();
    let mut pending_global: BTreeMap<String, bool> = BTreeMap::new();
    let mut pending_weak: BTreeMap<String, bool> = BTreeMap::new();
    // .set / .equ name, value
    let mut absolutes: BTreeMap<String, i64> = BTreeMap::new();

    for stmt in statements {
        match stmt {
            Statement::Directive(Directive::Section(kind)) => {
                current = match kind {
                    AsmSection::Text => ".text".into(),
                    AsmSection::Data => ".data".into(),
                    AsmSection::RoData => ".rodata".into(),
                    AsmSection::Bss => ".bss".into(),
                    AsmSection::Named(n) => {
                        if !sections.contains_key(&n) {
                            sections.insert(n.clone(), SecBuf::new());
                        }
                        n
                    }
                };
                if !sections.contains_key(&current) {
                    sections.insert(current.clone(), SecBuf::new());
                }
            }
            Statement::Directive(Directive::Global(name)) => {
                pending_global.insert(name.clone(), true);
                if let Some(sec) = sections.get_mut(&current) {
                    for s in &mut sec.symbols {
                        if s.name == name {
                            s.global = true;
                        }
                    }
                }
            }
            Statement::Directive(Directive::AlignP2(p2)) => {
                let align = 1u64 << p2.min(12);
                let fill = if current == ".text" { 0x90 } else { 0 };
                sections.get_mut(&current).unwrap().align_to(align, fill);
            }
            Statement::Directive(Directive::AlignBytes(n)) => {
                let fill = if current == ".text" { 0x90 } else { 0 };
                sections.get_mut(&current).unwrap().align_to(n.max(1), fill);
            }
            Statement::Directive(Directive::Zero(n)) => {
                let sec = sections.get_mut(&current).unwrap();
                if current == ".bss" {
                    sec.bss_size += n;
                } else {
                    sec.data.resize(sec.data.len() + n as usize, 0);
                }
            }
            Statement::Directive(Directive::Ascii(bytes)) => {
                sections
                    .get_mut(&current)
                    .unwrap()
                    .data
                    .extend_from_slice(&bytes);
            }
            Statement::Directive(Directive::Asciz(bytes)) => {
                let sec = sections.get_mut(&current).unwrap();
                sec.data.extend_from_slice(&bytes);
                sec.data.push(0);
            }
            Statement::Directive(Directive::Byte(vals)) => {
                sections
                    .get_mut(&current)
                    .unwrap()
                    .data
                    .extend_from_slice(&vals);
            }
            Statement::Directive(Directive::Word(vals)) => {
                let sec = sections.get_mut(&current).unwrap();
                for v in vals {
                    sec.data.extend_from_slice(&v.to_le_bytes());
                }
            }
            Statement::Directive(Directive::Long(vals)) => {
                let sec = sections.get_mut(&current).unwrap();
                for v in vals {
                    sec.data.extend_from_slice(&v.to_le_bytes());
                }
            }
            Statement::Directive(Directive::Quad(items)) => {
                let sec = sections.get_mut(&current).unwrap();
                for item in items {
                    match item {
                        parser::QuadItem::Int(v) => sec.data.extend_from_slice(&v.to_le_bytes()),
                        parser::QuadItem::Sym(symbol, addend) => {
                            let offset = sec.data.len() as u64;
                            sec.data.extend_from_slice(&0u64.to_le_bytes());
                            sec.relocs.push(SecRel {
                                offset,
                                symbol,
                                kind: RelocKind::Abs64,
                                addend,
                            });
                        }
                        // `.` resolves immediately to the running length of
                        // the *current* section at this point — a plain
                        // integer, no relocation needed.
                        parser::QuadItem::Here(addend) => {
                            let here = sec.data.len() as i64 + addend;
                            sec.data.extend_from_slice(&(here as u64).to_le_bytes());
                        }
                    }
                }
            }
            Statement::Directive(Directive::CfiProc) => {}
            Statement::Directive(Directive::Set(name, value)) => {
                absolutes.insert(name, value);
            }
            Statement::Label(name) => {
                let sec = sections.get_mut(&current).unwrap();
                let off = sec.len();
                let global = pending_global.remove(&name).unwrap_or(false);
                let weak = pending_weak.remove(&name).unwrap_or(false);
                sec.symbols.push(SecSym {
                    name: name.clone(),
                    offset: off,
                    global,
                    weak,
                });
                if verbose {
                    println!("[*] {name} @ {current}+{off:#x}");
                }
            }
            Statement::Instruction { mnemonic, operands } => {
                // Resolve local absolute equates in operands (simple).
                let ops: Vec<String> = operands
                    .iter()
                    .map(|o| {
                        if let Some(v) = absolutes.get(o.trim_start_matches('$')) {
                            format!("${v}")
                        } else {
                            o.clone()
                        }
                    })
                    .collect();
                let enc = encode_insn(&mnemonic, &ops)?;
                let sec = sections.get_mut(&current).unwrap();
                let base = sec.data.len() as u64;
                for r in enc.relocs {
                    sec.relocs.push(SecRel {
                        offset: base + r.offset as u64,
                        symbol: r.symbol,
                        kind: r.kind,
                        addend: r.addend,
                    });
                }
                if verbose {
                    println!("[*] {mnemonic} {ops:?} -> {:02x?}", enc.bytes);
                }
                sec.data.extend_from_slice(&enc.bytes);
            }
        }
    }

    build_elf_object(&sections, verbose)
}

fn build_elf_object(sections: &BTreeMap<String, SecBuf>, verbose: bool) -> Result<Vec<u8>> {
    let mut obj = Object::new(
        BinaryFormat::Elf,
        Architecture::X86_64,
        Endianness::Little,
    );

    let mut section_ids = BTreeMap::new();
    let mut symbol_ids: BTreeMap<String, object::write::SymbolId> = BTreeMap::new();

    // Create sections + defined symbols first.
    for (name, buf) in sections {
        if buf.data.is_empty() && buf.symbols.is_empty() && buf.relocs.is_empty() && buf.bss_size == 0
        {
            continue;
        }
        let id = match name.as_str() {
            ".text" => obj.section_id(StandardSection::Text),
            ".data" => obj.section_id(StandardSection::Data),
            ".rodata" => obj.section_id(StandardSection::ReadOnlyData),
            ".bss" => obj.section_id(StandardSection::UninitializedData),
            other => {
                let kind = if other.contains("text") {
                    SectionKind::Text
                } else if other.contains("bss") {
                    SectionKind::UninitializedData
                } else if other.contains("rodata") || other.contains("data.rel.ro") {
                    SectionKind::ReadOnlyData
                } else {
                    SectionKind::Data
                };
                obj.add_section(Vec::new(), other.as_bytes().to_vec(), kind)
            }
        };

        if name == ".bss" {
            let sz = if buf.bss_size > 0 {
                buf.bss_size
            } else {
                buf.data.len() as u64
            };
            if sz > 0 {
                obj.append_section_bss(id, sz, 1);
            }
        } else if !buf.data.is_empty() {
            let align = if name == ".text" { 16 } else { 8 };
            obj.append_section_data(id, &buf.data, align);
        } else {
            obj.append_section_data(id, &[], 1);
        }
        section_ids.insert(name.clone(), id);

        for sym in &buf.symbols {
            let kind = if name == ".text" {
                SymbolKind::Text
            } else {
                SymbolKind::Data
            };
            let scope = if sym.global || sym.weak {
                SymbolScope::Linkage
            } else {
                SymbolScope::Compilation
            };
            let sid = obj.add_symbol(Symbol {
                name: sym.name.as_bytes().to_vec(),
                value: sym.offset,
                size: 0,
                kind,
                scope,
                weak: sym.weak,
                section: SymbolSection::Section(id),
                flags: SymbolFlags::None,
            });
            symbol_ids.insert(sym.name.clone(), sid);
        }
    }

    if section_ids.is_empty() {
        let id = obj.section_id(StandardSection::Text);
        obj.append_section_data(id, &[0xc3], 16);
        section_ids.insert(".text".into(), id);
    }

    // Collect all reloc symbol names → ensure undefined symbols exist.
    for buf in sections.values() {
        for r in &buf.relocs {
            if !symbol_ids.contains_key(&r.symbol) {
                let sid = obj.add_symbol(Symbol {
                    name: r.symbol.as_bytes().to_vec(),
                    value: 0,
                    size: 0,
                    kind: SymbolKind::Unknown,
                    scope: SymbolScope::Linkage,
                    weak: false,
                    section: SymbolSection::Undefined,
                    flags: SymbolFlags::None,
                });
                symbol_ids.insert(r.symbol.clone(), sid);
                if verbose {
                    println!("[*] undefined symbol: {}", r.symbol);
                }
            }
        }
    }

    // Emit relocations (must be after symbols are defined).
    for (name, buf) in sections {
        let Some(&sec_id) = section_ids.get(name) else {
            continue;
        };
        for r in &buf.relocs {
            let Some(&sym_id) = symbol_ids.get(&r.symbol) else {
                continue;
            };
            // TLS relocations have no `RelocationKind`/`RelocationEncoding`
            // pair in the object crate's generic model — go straight to the
            // raw ELF r_type instead of routing through `RelocationFlags::Generic`.
            if let Some(r_type) = match r.kind {
                RelocKind::TpOff32 => Some(object::elf::R_X86_64_TPOFF32),
                RelocKind::GotTpOff => Some(object::elf::R_X86_64_GOTTPOFF),
                _ => None,
            } {
                obj.add_relocation(
                    sec_id,
                    Relocation {
                        offset: r.offset,
                        symbol: sym_id,
                        addend: r.addend,
                        flags: RelocationFlags::Elf { r_type },
                    },
                )
                .map_err(|e| anyhow::anyhow!("reloc {}: {e}", r.symbol))?;
                continue;
            }
            let (kind, encoding, size) = match r.kind {
                RelocKind::Pc32 => (
                    RelocationKind::Relative,
                    RelocationEncoding::X86RipRelative,
                    32u8,
                ),
                RelocKind::Plt32 => (
                    RelocationKind::PltRelative,
                    RelocationEncoding::X86Branch,
                    32,
                ),
                RelocKind::Abs32 => (RelocationKind::Absolute, RelocationEncoding::Generic, 32),
                RelocKind::Abs32S => (RelocationKind::Absolute, RelocationEncoding::X86Signed, 32),
                RelocKind::Abs64 => (RelocationKind::Absolute, RelocationEncoding::Generic, 64),
                RelocKind::TpOff32 | RelocKind::GotTpOff => {
                    unreachable!("handled via RelocationFlags::Elf above")
                }
            };
            // Write implicit addend into section data if non-zero and field is zero.
            // object::add_relocation may also write addend depending on format.
            obj.add_relocation(
                sec_id,
                Relocation {
                    offset: r.offset,
                    symbol: sym_id,
                    addend: r.addend,
                    flags: RelocationFlags::Generic {
                        kind,
                        encoding,
                        size,
                    },
                },
            )
            .map_err(|e| anyhow::anyhow!("reloc {}: {e}", r.symbol))?;
        }
    }

    // Silence unused helper type from encode
    let _ = std::mem::size_of::<PendingReloc>();

    obj.write()
        .map_err(|e| anyhow::anyhow!("ELF write error: {e:?}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use object::{Object as _, ObjectSection, ObjectSymbol};

    #[test]
    fn assembles_minimal_function() {
        let src = r#"
            .text
            .globl main
            main:
                pushq %rbp
                movq %rsp, %rbp
                xorl %eax, %eax
                popq %rbp
                ret
        "#;
        let bytes = assemble_x86_64(src, false).unwrap();
        let f = object::File::parse(&*bytes).unwrap();
        let text = f.sections().find(|s| s.name() == Ok(".text")).unwrap();
        assert_eq!(
            text.data().unwrap(),
            &[0x55, 0x48, 0x89, 0xe5, 0x31, 0xc0, 0x5d, 0xc3]
        );
        let main = f.symbols().find(|s| s.name() == Ok("main")).unwrap();
        assert!(main.is_global());
    }

    #[test]
    fn assembles_call_with_reloc() {
        let src = r#"
            .text
            .globl _start
            _start:
                call puts
                ret
        "#;
        let bytes = assemble_x86_64(src, false).unwrap();
        let f = object::File::parse(&*bytes).unwrap();
        let text = f.sections().find(|s| s.name() == Ok(".text")).unwrap();
        let relocs: Vec<_> = text.relocations().collect();
        assert!(
            !relocs.is_empty(),
            "expected relocation for call puts, got none"
        );
        // undefined puts must exist
        assert!(f.symbols().any(|s| s.name() == Ok("puts") && s.is_undefined()));
    }

    #[test]
    fn assembles_data_and_lea() {
        let src = r#"
            .text
            .globl _start
            _start:
                leaq msg(%rip), %rdi
                ret
            .section .rodata
            msg:
                .asciz "hi"
        "#;
        let bytes = assemble_x86_64(src, false).unwrap();
        let f = object::File::parse(&*bytes).unwrap();
        assert!(f.symbols().any(|s| s.name() == Ok("msg")));
        let text = f.sections().find(|s| s.name() == Ok(".text")).unwrap();
        assert!(!text.relocations().collect::<Vec<_>>().is_empty());
    }

    #[test]
    fn equ_substitutes_into_immediate_operand() {
        let src = r#"
            .equ FOO, 5
            .text
            .globl _start
            _start:
                movl $FOO, %eax
                ret
        "#;
        let bytes = assemble_x86_64(src, false).unwrap();
        let f = object::File::parse(&*bytes).unwrap();
        let text = f.sections().find(|s| s.name() == Ok(".text")).unwrap();
        // movl $5, %eax → b8 05 00 00 00 ; then ret → c3
        assert_eq!(
            text.data().unwrap(),
            &[0xb8, 0x05, 0x00, 0x00, 0x00, 0xc3]
        );
    }
}
