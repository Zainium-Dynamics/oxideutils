//! oxide-objcopy — copy and transform object files (GNU objcopy subset).

use clap::Parser;
use oxideutils_core::cli::help::{self, print_version, VERSION};
use oxideutils_core::objcopy::{objcopy_file, ObjcopyOptions};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-objcopy",
    about = help::objcopy::ABOUT,
    long_about = help::objcopy::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Remove all symbols in the output
    #[arg(long = "strip-all", visible_short_alias = 'S')]
    strip_all: bool,

    /// Remove debugging symbols / sections
    #[arg(long = "strip-debug", visible_alias = "strip-dwo")]
    strip_debug: bool,

    /// Remove symbols not needed for relocation
    #[arg(long = "strip-unneeded")]
    strip_unneeded: bool,

    /// Keep only this section (repeatable), e.g. -j .text
    #[arg(short = 'j', long = "only-section", value_name = "NAME")]
    only_section: Vec<String>,

    /// Remove this section (repeatable)
    #[arg(short = 'R', long = "remove-section", value_name = "NAME")]
    remove_section: Vec<String>,

    /// Output target: elf (default) or binary
    #[arg(short = 'O', long = "output-target", value_name = "TARGET")]
    output_target: Option<String>,

    /// Input target (accepted for GNU CLI compatibility; currently ignored)
    #[arg(short = 'I', long = "input-target", value_name = "TARGET")]
    _input_target: Option<String>,

    /// Verbose
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Version banner
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Input file
    #[arg(value_name = "INFILE")]
    infile: Option<PathBuf>,

    /// Output file
    #[arg(value_name = "OUTFILE")]
    outfile: Option<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("objcopy") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-objcopy");
        return ExitCode::SUCCESS;
    }

    let infile = match args.infile {
        Some(p) => p,
        None => {
            eprintln!("oxide-objcopy: no input file specified");
            eprintln!("Usage: oxide-objcopy [options] INFILE OUTFILE");
            eprintln!("Try:   oxide-objcopy --help");
            return ExitCode::from(1);
        }
    };
    let outfile = match args.outfile {
        Some(p) => p,
        None => {
            eprintln!("oxide-objcopy: no output file specified (need: objcopy IN OUT)");
            eprintln!("Try:  oxide-objcopy --help");
            return ExitCode::from(1);
        }
    };

    let opts = ObjcopyOptions {
        strip_all: args.strip_all,
        strip_debug: args.strip_debug,
        strip_unneeded: args.strip_unneeded,
        only_sections: args.only_section,
        remove_sections: args.remove_section,
        output_target: args.output_target,
    };

    match objcopy_file(&infile, &outfile, &opts) {
        Ok(()) => {
            if args.verbose {
                eprintln!("objcopy: {} -> {}", infile.display(), outfile.display());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}
