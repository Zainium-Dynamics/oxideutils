//! oxide-objdump — GNU objdump compatible (binutils CLI subset).
//!
//! Flow mirrors `binutils/objdump.c`:
//! 1. Parse switches selecting what to display
//! 2. Process each file (and archive members)
//! 3. Dump headers / sections / symbols / relocs / contents / disassembly

use clap::Parser;
use object::{Object, ObjectSection};
use oxideutils_core::archive::{is_archive, OxideArchive};
use oxideutils_core::cli::config::RuntimeConfig;
use oxideutils_core::cli::help::{self, print_version, VERSION};
use oxideutils_core::cli::utils::Status;
use oxideutils_core::error::{OxideError, Result};
use oxideutils_core::format::elf::ElfFile;
use oxideutils_core::format::object::OxideObject;
use oxideutils_core::symbols::{list_symbols, SymbolFilter};
use oxideutils_core::utils::{hex_dump, map_file, parse_address, read_file};
use object::{ObjectSymbol, RelocationKind, RelocationTarget};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-objdump",
    about = help::objdump::ABOUT,
    long_about = help::objdump::LONG_ABOUT,
    after_long_help = help::objdump::AFTER_HELP,
    version = VERSION,
    disable_version_flag = true,
    // GNU uses -H for help; -h is section headers
    disable_help_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Show this help screen  (note: -h is sections, not help!)
    #[arg(short = 'H', long = "help", action = clap::ArgAction::Help)]
    help: Option<bool>,
    /// List members inside a .a archive
    #[arg(short = 'a', long = "archive-headers")]
    archive_headers: bool,

    /// File header: format, architecture, entry point
    #[arg(short = 'f', long = "file-headers")]
    file_headers: bool,

    /// Section list (.text, .data, …)  — NOT help; use -H for help
    #[arg(short = 'h', long = "section-headers", visible_alias = "headers")]
    section_headers: bool,

    /// Many headers at once (+ ELF dynamic/notes/versions)
    #[arg(short = 'x', long = "all-headers")]
    all_headers: bool,

    /// Disassemble code sections  (optional: --disassemble=SYMBOL)
    #[arg(
        short = 'd',
        long = "disassemble",
        num_args = 0..=1,
        default_missing_value = "",
        require_equals = true
    )]
    disassemble: Option<String>,

    /// Disassemble every section (not only executable ones)
    #[arg(short = 'D', long = "disassemble-all")]
    disassemble_all: bool,

    /// Hex dump of section contents
    #[arg(short = 's', long = "full-contents")]
    full_contents: bool,

    /// Symbol table (names of functions / variables)
    #[arg(short = 't', long = "syms")]
    syms: bool,

    /// Dynamic symbols (shared library exports / imports)
    #[arg(short = 'T', long = "dynamic-syms")]
    dynamic_syms: bool,

    /// Relocations (how the linker patches addresses)
    #[arg(short = 'r', long = "reloc")]
    reloc: bool,

    /// Dynamic relocations
    #[arg(short = 'R', long = "dynamic-reloc")]
    dynamic_reloc: bool,

    /// Only show this section name (repeatable), e.g. -j .text
    #[arg(short = 'j', long = "section", value_name = "NAME")]
    section: Vec<String>,

    /// Demangle C++ / Rust symbol names
    #[arg(short = 'C', long = "demangle")]
    demangle: bool,

    /// Wide output (reserved for GNU compatibility)
    #[arg(short = 'w', long = "wide")]
    wide: bool,

    /// Do not skip long runs of zero bytes while disassembling
    #[arg(short = 'z', long = "disassemble-zeroes")]
    disassemble_zeroes: bool,

    /// Start disassembly / dump at this address (hex ok: 0x401000)
    #[arg(long = "start-address", value_name = "ADDR")]
    start_address: Option<String>,

    /// Stop before this address
    #[arg(long = "stop-address", value_name = "ADDR")]
    stop_address: Option<String>,

    /// Print version banner
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// List supported formats / architectures
    #[arg(short = 'i', long = "info")]
    info: bool,

    /// Print effective TOML config and exit
    #[arg(long = "print-config")]
    print_config: bool,

    /// Dump SFrame stack-trace section (default name: .sframe)
    #[arg(long = "sframe", num_args = 0..=1, default_missing_value = ".sframe", require_equals = true, value_name = "SECTION")]
    sframe: Option<String>,

    /// Object files or archives to inspect
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("objdump") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    let cfg = RuntimeConfig::load();

    if args.version {
        print_version("oxide-objdump");
        return ExitCode::SUCCESS;
    }
    if args.print_config {
        match cfg.toml.to_toml_string() {
            Ok(s) => {
                if let Some(p) = oxideutils_core::cli::config::OxideToml::discover_path() {
                    eprintln!("# loaded from: {}", p.display());
                } else {
                    eprintln!("# loaded from: built-in defaults (+ env)");
                }
                print!("{s}");
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("oxide-objdump: {e}");
                return ExitCode::from(1);
            }
        }
    }
    if args.info {
        println!("oxide-objdump: supported formats: elf, pe, mach-o, wasm, coff, archive");
        println!("architectures: x86-64, i386, aarch64, arm, riscv, ppc, mips, s390x, wasm32, …");
        return ExitCode::SUCCESS;
    }

    let wants_display = args.archive_headers
        || args.file_headers
        || args.section_headers
        || args.all_headers
        || args.disassemble.is_some()
        || args.disassemble_all
        || args.full_contents
        || args.syms
        || args.dynamic_syms
        || args.reloc
        || args.dynamic_reloc
        || args.sframe.is_some();

    if !wants_display {
        eprintln!("oxide-objdump: at least one of the display switches must be given");
        eprintln!("Use --help for a complete list of options.");
        return ExitCode::from(2);
    }

    if args.files.is_empty() {
        eprintln!("oxide-objdump: no input files specified");
        return ExitCode::from(1);
    }

    let mut status = Status::ok();
    for path in &args.files {
        status.record(process_path(path, &args, &cfg));
    }
    status.exit_code()
}

fn process_path(path: &Path, args: &Args, cfg: &RuntimeConfig) -> Result<()> {
    // Prefer mmap; fall back to read for special files
    let owned;
    let data: &[u8] = match map_file(path) {
        Ok(m) => {
            // leak map into process for simple lifetime — use owned instead
            owned = m.to_vec();
            &owned
        }
        Err(_) => {
            owned = read_file(path)?;
            &owned
        }
    };

    if is_archive(data) {
        return process_archive(path, data, args, cfg);
    }
    process_object(path, data, args, cfg)
}

fn process_archive(path: &Path, data: &[u8], args: &Args, cfg: &RuntimeConfig) -> Result<()> {
    let arch = OxideArchive::parse(path.display(), data)?;
    if args.archive_headers || args.all_headers {
        println!("In archive {}:", path.display());
        for m in &arch.members {
            println!(
                "  {} offset={} size={}",
                m.name, m.offset, m.size
            );
        }
    }
    for m in &arch.members {
        let md = arch.member_data(m);
        let member_path = path.join(&m.name);
        // display as archive(member)
        let label = format!("{}({})", path.display(), m.name);
        if let Err(e) = process_object(Path::new(&label), md, args, cfg) {
            eprintln!("{e}");
        }
        let _ = member_path;
    }
    Ok(())
}

fn process_object(path: &Path, data: &[u8], args: &Args, cfg: &RuntimeConfig) -> Result<()> {
    let obj = OxideObject::parse(path.display(), data)?;

    let dump_file = args.file_headers || args.all_headers;
    let dump_sec = args.section_headers || args.all_headers;
    let dump_syms = args.syms || args.all_headers;
    let dump_dyn = args.dynamic_syms || args.all_headers;
    let dis = args.disassemble.is_some() || args.disassemble_all;
    let contents = args.full_contents;

    if dump_file {
        print!("{}", obj.format_file_header());
    }
    if dump_sec {
        print!("{}", obj.format_section_headers()?);
    }
    if dump_syms || dump_dyn {
        let filter = SymbolFilter {
            demangle: args.demangle || cfg.demangle || cfg.toml.objdump.demangle,
            ..Default::default()
        };
        let syms = list_symbols(&obj, &filter)?;
        println!("\nSYMBOL TABLE:");
        for s in syms {
            if dump_dyn && !s.is_global && !s.is_undefined {
                // still print all for now; dynamic filter refined later
            }
            if s.is_undefined {
                println!("0000000000000000 {} {}", s.nm_type_char(), s.name);
            } else {
                println!("{:016x} {} {}", s.address, s.nm_type_char(), s.name);
            }
        }
    }
    if args.reloc || args.dynamic_reloc {
        dump_relocations(&obj)?;
    }

    if let Some(sec) = &args.sframe {
        let name = if sec.is_empty() { ".sframe" } else { sec.as_str() };
        if let Ok(elf) = ElfFile::parse(path.display(), data) {
            print!("{}", elf.format_sframe(Some(name)));
        } else {
            eprintln!("oxide-objdump: --sframe requires ELF input");
        }
    }

    // Richer -x: also show dynamic + notes for ELF when all-headers
    if args.all_headers
        && let Ok(elf) = ElfFile::parse(path.display(), data)
    {
        print!("{}", elf.format_dynamic());
        print!("{}", elf.format_notes());
        print!("{}", elf.format_version_info());
    }

    if contents {
        dump_contents(&obj, args)?;
    }

    if dis {
        dump_disassembly(&obj, args, cfg)?;
    }

    Ok(())
}

fn dump_relocations(obj: &OxideObject<'_>) -> Result<()> {
    use object::ObjectSection;
    let mut any_sec = false;
    for sec in obj.file.sections() {
        let name = sec.name().unwrap_or("?");
        let mut any = false;
        for (offset, rel) in sec.relocations() {
            if !any {
                println!("\nRELOCATION RECORDS FOR [{name}]:");
                println!("{:18} {:16} VALUE", "OFFSET", "TYPE");
                any = true;
                any_sec = true;
            }
            let kind = format!("{:?}", rel.kind());
            let target = match rel.target() {
                RelocationTarget::Symbol(idx) => obj
                    .file
                    .symbol_by_index(idx)
                    .ok()
                    .and_then(|s| s.name().ok().map(|n| n.to_string()))
                    .unwrap_or_else(|| format!("sym#{}", idx.0)),
                RelocationTarget::Section(idx) => format!("section#{}", idx.0),
                RelocationTarget::Absolute => "*ABS*".into(),
                _ => "?".into(),
            };
            let addend = rel.addend();
            let addend_s = if addend != 0 {
                format!("+{addend:#x}")
            } else {
                String::new()
            };
            let _ = RelocationKind::Absolute; // keep import warm if kind is unused path
            println!("{offset:016x}  {kind:<14} {target}{addend_s}");
        }
    }
    if !any_sec {
        println!("\nRELOCATION RECORDS: (none)");
    }
    Ok(())
}

fn dump_contents(obj: &OxideObject<'_>, args: &Args) -> Result<()> {
    let start = args
        .start_address
        .as_ref()
        .map(|s| parse_address(s))
        .transpose()?;
    let stop = args
        .stop_address
        .as_ref()
        .map(|s| parse_address(s))
        .transpose()?;

    for sec in obj.file.sections() {
        let name = sec.name().unwrap_or("<no-name>");
        if !args.section.is_empty() && !args.section.iter().any(|s| s == name) {
            continue;
        }
        let data = match sec.uncompressed_data() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if data.is_empty() {
            continue;
        }
        let addr = sec.address();
        println!("\nContents of section {name}:");
        let mut slice = data.as_ref();
        let mut base = addr;
        if let Some(sa) = start {
            if addr + slice.len() as u64 <= sa {
                continue;
            }
            if sa > addr {
                let skip = (sa - addr) as usize;
                if skip < slice.len() {
                    slice = &slice[skip..];
                    base = sa;
                }
            }
        }
        if let Some(ea) = stop {
            let end = base + slice.len() as u64;
            if base >= ea {
                continue;
            }
            if end > ea {
                let keep = (ea - base) as usize;
                slice = &slice[..keep.min(slice.len())];
            }
        }
        print!("{}", hex_dump(base, slice, 16));
    }
    Ok(())
}

fn dump_disassembly(obj: &OxideObject<'_>, args: &Args, cfg: &RuntimeConfig) -> Result<()> {
    use object::ObjectSymbol;
    use oxideutils_core::disasm::{format_disassembly_with_labels, DisasmOptions};

    let all = args.disassemble_all;
    let only_sym = args
        .disassemble
        .as_ref()
        .filter(|s| !s.is_empty())
        .cloned();

    let opts = DisasmOptions {
        show_raw_insn: cfg.toml.disasm.show_raw_insn,
        disassemble_zeroes: args.disassemble_zeroes || cfg.toml.disasm.disassemble_zeroes,
        start_address: args
            .start_address
            .as_ref()
            .map(|s| parse_address(s))
            .transpose()?,
        stop_address: args
            .stop_address
            .as_ref()
            .map(|s| parse_address(s))
            .transpose()?,
        insn_width: 7,
        allow_hex_fallback: true,
    };

    let mut syms: Vec<(u64, String)> = obj
        .file
        .symbols()
        .filter(|s| !s.is_undefined())
        .filter_map(|s| {
            let n = s.name().ok()?;
            Some((s.address(), n.to_string()))
        })
        .collect();
    syms.sort_by_key(|(a, _)| *a);

    // --disassemble=SYMBOL → restrict to that symbol's range
    let sym_range: Option<(u64, u64)> = if let Some(ref name) = only_sym {
        let mut found = None;
        for (i, (addr, n)) in syms.iter().enumerate() {
            if n == name || n.strip_prefix('_') == Some(name.as_str()) {
                let end = syms
                    .get(i + 1)
                    .map(|(a, _)| *a)
                    .unwrap_or(u64::MAX);
                found = Some((*addr, end));
                break;
            }
        }
        if found.is_none() {
            return Err(OxideError::SymbolNotFound(name.clone()));
        }
        found
    } else {
        None
    };

    let arch = obj.file.architecture();

    for sec in obj.file.sections() {
        let name = sec.name().unwrap_or("?");
        let is_text = matches!(sec.kind(), object::SectionKind::Text);
        if !all && !is_text {
            continue;
        }
        if !args.section.is_empty() && !args.section.iter().any(|s| s == name) {
            continue;
        }
        let data = match sec.uncompressed_data() {
            Ok(d) => d,
            Err(_) => continue,
        };
        if data.is_empty() {
            continue;
        }

        let mut base = sec.address();
        let mut slice = data.as_ref();
        let mut local_opts = opts.clone();

        if let Some((sa, ea)) = sym_range {
            let sec_end = base + slice.len() as u64;
            if ea <= base || sa >= sec_end {
                continue;
            }
            let start_off = sa.saturating_sub(base) as usize;
            let end_off = (ea.min(sec_end) - base) as usize;
            if start_off >= slice.len() {
                continue;
            }
            slice = &slice[start_off..end_off.min(slice.len())];
            base = sa;
            local_opts.start_address = Some(sa);
            local_opts.stop_address = Some(ea);
        }

        let text = format_disassembly_with_labels(name, arch, base, slice, &syms, &local_opts)?;
        print!("{text}");
    }
    Ok(())
}

#[allow(dead_code)]
fn _unused_err() -> OxideError {
    OxideError::NoDisplaySwitch
}
