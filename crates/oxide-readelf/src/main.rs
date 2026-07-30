//! oxide-readelf — display ELF file information (GNU readelf compatible subset).

use clap::Parser;
use oxideutils_core::cli::help::{self, print_version, VERSION};
use oxideutils_core::cli::utils::Status;
use oxideutils_core::error::Result;
use oxideutils_core::format::elf::ElfFile;
use oxideutils_core::utils::{map_file, read_file};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-readelf",
    about = help::readelf::ABOUT,
    long_about = help::readelf::LONG_ABOUT,
    after_long_help = help::readelf::AFTER_HELP,
    version = VERSION,
    disable_version_flag = true,
    // GNU readelf: -h = file header, -H = help
    disable_help_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Show this help screen  (note: -h is the ELF header!)
    #[arg(short = 'H', long = "help", action = clap::ArgAction::Help)]
    help: Option<bool>,

    /// ELF file header (type, machine, entry)  — NOT help; use -H
    #[arg(short = 'h', long = "file-header")]
    file_header: bool,

    /// Section headers (.text, .data, …)
    #[arg(short = 'S', long = "section-headers", visible_alias = "sections")]
    section_headers: bool,

    /// Program headers / segments (what the loader maps)
    #[arg(short = 'l', long = "program-headers", visible_alias = "segments")]
    program_headers: bool,

    /// Symbol tables
    #[arg(short = 's', long = "symbols", visible_alias = "syms")]
    symbols: bool,

    /// Dynamic section (shared libraries, rpath, …)
    #[arg(short = 'd', long = "dynamic")]
    dynamic: bool,

    /// Relocations
    #[arg(short = 'r', long = "relocs")]
    relocs: bool,

    /// Notes (build-id, ABI tag, …)
    #[arg(short = 'n', long = "notes")]
    notes: bool,

    /// Symbol versioning (versym / verneed / verdef)
    #[arg(short = 'V', long = "version-info")]
    version_info: bool,

    /// Dump Global Offset Table contents (GNU binutils 2.46)
    #[arg(long = "got-contents")]
    got_contents: bool,

    /// SFrame stack-trace dump (default section: .sframe)
    #[arg(long = "sframe", num_args = 0..=1, default_missing_value = ".sframe", require_equals = true, value_name = "SECTION")]
    sframe: Option<String>,

    /// Unwind section summary (.eh_frame)
    #[arg(short = 'u', long = "unwind")]
    unwind: bool,

    /// Almost everything: -h -l -S -s -r -d -n -V -u --got-contents
    #[arg(short = 'a', long = "all")]
    all: bool,

    /// Program version banner
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// ELF files to inspect
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("readelf") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-readelf");
        return ExitCode::SUCCESS;
    }
    if args.files.is_empty() {
        eprintln!("oxide-readelf: no input files");
        eprintln!("Try:  oxide-readelf --help");
        return ExitCode::from(1);
    }
    let show_any = args.file_header
        || args.section_headers
        || args.program_headers
        || args.symbols
        || args.dynamic
        || args.relocs
        || args.notes
        || args.version_info
        || args.got_contents
        || args.sframe.is_some()
        || args.unwind
        || args.all;
    if !show_any {
        eprintln!("oxide-readelf: Warning: Nothing to do.");
        eprintln!("(input file: {})", args.files[0].display());
        eprintln!("Tip: pass -h, -S, -a, … or run:  oxide-readelf --help");
        return ExitCode::from(1);
    }
    let mut status = Status::ok();
    for f in &args.files {
        status.record(process(f, &args));
    }
    status.exit_code()
}

fn process(path: &Path, args: &Args) -> Result<()> {
    let data = map_file(path)
        .map(|m| m.to_vec())
        .or_else(|_| read_file(path))?;
    let elf = ElfFile::parse(path.display(), &data)?;

    if args.file_header || args.all {
        print!("{}", elf.format_elf_header());
    }
    if args.section_headers || args.all {
        print!("{}", elf.format_section_headers());
    }
    if args.program_headers || args.all {
        print!("{}", elf.format_program_headers());
    }
    if args.dynamic || args.all {
        print!("{}", elf.format_dynamic());
    }
    if args.relocs || args.all {
        print!("{}", elf.format_relocs());
    }
    if args.symbols || args.all {
        print!("{}", elf.format_symbols());
    }
    if args.notes || args.all {
        print!("{}", elf.format_notes());
    }
    if args.version_info || args.all {
        print!("{}", elf.format_version_info());
    }
    if args.got_contents || args.all {
        print!("{}", elf.format_got_contents());
    }
    if args.unwind || args.all {
        print!("{}", elf.format_unwind());
    }
    if let Some(sec) = &args.sframe {
        let name = if sec.is_empty() { ".sframe" } else { sec.as_str() };
        print!("{}", elf.format_sframe(Some(name)));
    }

    Ok(())
}
