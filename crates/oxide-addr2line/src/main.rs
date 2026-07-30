//! oxide-addr2line — convert addresses into file names and line numbers (GNU compatible).

use clap::Parser;
use oxideutils_core::addr2line_util::{Addr2LineContext, Addr2LineOptions};
use oxideutils_core::cli::help::{self, print_version, VERSION};
use oxideutils_core::error::{OxideError, Result};
use oxideutils_core::utils::parse_address;
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-addr2line",
    about = help::addr2line::ABOUT,
    long_about = help::addr2line::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Executable or shared object that has debug info
    #[arg(short = 'e', long = "exe", default_value = "a.out", value_name = "FILE")]
    exe: PathBuf,

    /// Show function names
    #[arg(short = 'f', long = "functions")]
    functions: bool,

    /// Demangle function names
    #[arg(short = 'C', long = "demangle")]
    demangle: bool,

    /// Pretty print (one line per address)
    #[arg(short = 'p', long = "pretty-print")]
    pretty: bool,

    /// Show only basenames of source paths
    #[arg(short = 's', long = "basenames")]
    basenames: bool,

    /// Unwind inlined functions
    #[arg(short = 'i', long = "inlines")]
    inlines: bool,

    /// Print the address before each result
    #[arg(short = 'a', long = "addresses")]
    show_addresses: bool,

    /// Version banner
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Addresses (hex or decimal). If empty, read from stdin.
    #[arg(value_name = "ADDR")]
    addrs: Vec<String>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("addr2line") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-addr2line");
        return ExitCode::SUCCESS;
    }

    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

fn run(args: Args) -> Result<()> {
    if !args.exe.exists() {
        return Err(OxideError::io_path(
            &args.exe,
            std::io::Error::new(std::io::ErrorKind::NotFound, "No such file"),
        ));
    }

    let opts = Addr2LineOptions {
        demangle: args.demangle,
        functions: args.functions,
        pretty: args.pretty,
        basenames: args.basenames,
        inlines: args.inlines,
    };
    let ctx = Addr2LineContext::open(&args.exe)?;

    let print_one = |addr: u64| -> Result<()> {
        if args.show_addresses {
            println!("{addr:#x}");
        }
        let resolved = ctx.resolve(addr, &opts)?;
        print!("{}", resolved.format_gnu(&opts));
        let _ = io::stdout().flush();
        Ok(())
    };

    if args.addrs.is_empty() {
        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = line.map_err(|e| OxideError::io("<stdin>", e))?;
            for tok in line.split_whitespace() {
                let addr = parse_address(tok)?;
                print_one(addr)?;
            }
        }
    } else {
        for a in &args.addrs {
            let addr = parse_address(a)?;
            print_one(addr)?;
        }
    }
    Ok(())
}
