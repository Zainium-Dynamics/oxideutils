//! oxide-nm — list symbols from object files (GNU nm compatible).

use clap::Parser;
use oxideutils_core::archive::{is_archive, OxideArchive};
use oxideutils_core::cli::help::{self, print_version, VERSION};
use oxideutils_core::cli::utils::Status;
use oxideutils_core::error::Result;
use oxideutils_core::format::object::OxideObject;
use oxideutils_core::symbols::{list_symbols, SymbolFilter, SymbolSort};
use oxideutils_core::utils::{map_file, read_file};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-nm",
    about = help::nm::ABOUT,
    long_about = help::nm::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Only external / global symbols
    #[arg(short = 'g', long = "extern-only")]
    extern_only: bool,

    /// Only undefined symbols (need to be resolved by the linker)
    #[arg(short = 'u', long = "undefined-only")]
    undefined_only: bool,

    /// Only defined symbols
    #[arg(short = 'U', long = "defined-only")]
    defined_only: bool,

    /// Demangle C++ / Rust names
    #[arg(short = 'C', long = "demangle")]
    demangle: bool,

    /// Sort by address (numeric)
    #[arg(short = 'n', long = "numeric-sort", visible_alias = "v")]
    numeric_sort: bool,

    /// Sort by size (Oxide/GNU long option; NOT the same as -S)
    #[arg(long = "size-sort")]
    size_sort: bool,

    /// Do not sort; keep symbol-table order
    #[arg(short = 'p', long = "no-sort")]
    no_sort: bool,

    /// Reverse the sort order
    #[arg(short = 'r', long = "reverse-sort")]
    reverse: bool,

    /// Print size of defined symbols (GNU -S / --print-size)
    #[arg(short = 'S', long = "print-size")]
    print_size: bool,

    /// Print the input file name before every symbol
    #[arg(short = 'A', long = "print-file-name", visible_alias = "o")]
    print_file_name: bool,

    /// Version banner
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Object files or archives
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("nm") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-nm");
        return ExitCode::SUCCESS;
    }
    if args.files.is_empty() {
        eprintln!("oxide-nm: no input files");
        eprintln!("Try:  oxide-nm --help");
        return ExitCode::from(1);
    }
    let mut status = Status::ok();
    let multi = args.files.len() > 1 || args.print_file_name;
    for f in &args.files {
        status.record(process(f, &args, multi));
    }
    status.exit_code()
}

fn process(path: &Path, args: &Args, multi: bool) -> Result<()> {
    let data = map_file(path)
        .map(|m| m.to_vec())
        .or_else(|_| read_file(path))?;
    if is_archive(&data) {
        let arch = OxideArchive::parse(path.display(), &data)?;
        for m in &arch.members {
            if m.name == "/" || m.name == "//" || m.name == "/SYM64/" {
                continue;
            }
            let label = format!("{}:{}", path.display(), m.name);
            if let Ok(obj) = OxideObject::parse(&label, arch.member_data(m)) {
                print_syms(&obj, args, true)?;
            }
        }
        return Ok(());
    }
    let obj = OxideObject::parse(path.display(), &data)?;
    print_syms(&obj, args, multi)
}

fn print_syms(obj: &OxideObject<'_>, args: &Args, multi: bool) -> Result<()> {
    let filter = SymbolFilter {
        defined_only: args.defined_only,
        undefined_only: args.undefined_only,
        external_only: args.extern_only,
        demangle: args.demangle,
        sort: if args.no_sort {
            SymbolSort::None
        } else {
            SymbolSort::Name
        },
        reverse: args.reverse,
        numeric_sort: args.numeric_sort,
        size_sort: args.size_sort,
    };
    let syms = list_symbols(obj, &filter)?;
    if multi {
        println!("\n{}:", obj.path);
    }
    for s in syms {
        let prefix = if args.print_file_name {
            format!("{}:", obj.path)
        } else {
            String::new()
        };
        if s.is_undefined {
            if args.print_size {
                println!(
                    "{prefix}{:16} {:8} {} {}",
                    "", "", s.nm_type_char(), s.name
                );
            } else {
                println!("{prefix}{:16} {} {}", "", s.nm_type_char(), s.name);
            }
        } else if args.print_size {
            println!(
                "{prefix}{:016x} {:8x} {} {}",
                s.address,
                s.size,
                s.nm_type_char(),
                s.name
            );
        } else {
            println!("{prefix}{:016x} {} {}", s.address, s.nm_type_char(), s.name);
        }
    }
    Ok(())
}
