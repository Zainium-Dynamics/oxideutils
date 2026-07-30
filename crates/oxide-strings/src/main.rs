//! oxide-strings — print printable strings from files (GNU strings compatible).

use clap::Parser;
use oxideutils_core::cli::help::{self, print_version, VERSION};
use oxideutils_core::cli::utils::Status;
use oxideutils_core::error::Result;
use oxideutils_core::utils::read_file;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-strings",
    about = help::strings::ABOUT,
    long_about = help::strings::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Minimum string length (default: 4)
    #[arg(short = 'n', long = "bytes", default_value_t = 4, value_name = "N")]
    min_len: usize,

    /// Print offset before each string: x=hex, o=octal, d=decimal
    #[arg(short = 't', long = "radix", default_value = "", value_name = "x|o|d")]
    radix: String,

    /// Scan the whole file (default behaviour; kept for GNU compatibility)
    #[arg(short = 'a', long = "all")]
    _all: bool,

    /// Version banner
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Files to scan
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("strings") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-strings");
        return ExitCode::SUCCESS;
    }
    if args.files.is_empty() {
        eprintln!("oxide-strings: no input files");
        eprintln!("Try:  oxide-strings --help");
        return ExitCode::from(1);
    }
    let mut status = Status::ok();
    for f in &args.files {
        status.record(scan(f, &args));
    }
    status.exit_code()
}

fn scan(path: &Path, args: &Args) -> Result<()> {
    let data = read_file(path)?;
    let min = args.min_len.max(1);
    let show_off = !args.radix.is_empty();
    let mut i = 0usize;
    while i < data.len() {
        if is_print(data[i]) {
            let start = i;
            while i < data.len() && is_print(data[i]) {
                i += 1;
            }
            if i - start >= min {
                let s = String::from_utf8_lossy(&data[start..i]);
                if show_off {
                    match args.radix.as_str() {
                        "o" => println!("{:o} {s}", start),
                        "x" => println!("{:x} {s}", start),
                        _ => println!("{start} {s}"),
                    }
                } else {
                    println!("{s}");
                }
            }
        } else {
            i += 1;
        }
    }
    Ok(())
}

fn is_print(b: u8) -> bool {
    (0x20..=0x7e).contains(&b) || b == b'\t'
}
