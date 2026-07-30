//! oxide-strip — discard symbols from object files (GNU strip compatible subset).

use clap::Parser;
use oxideutils_core::cli::help::{self, VERSION, print_version};
use oxideutils_core::cli::utils::Status;
use oxideutils_core::error::{OxideError, Result};
use oxideutils_core::strip::{StripOptions, strip_file};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-strip",
    about = help::strip::ABOUT,
    long_about = help::strip::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Remove all symbols (default if no strip mode is given)
    #[arg(short = 's', long = "strip-all")]
    strip_all: bool,

    /// Remove debugging symbols / sections only
    #[arg(short = 'g', long = "strip-debug", visible_alias = "d")]
    strip_debug: bool,

    /// Remove symbols not needed for relocation
    #[arg(long = "strip-unneeded")]
    strip_unneeded: bool,

    /// Write output to this path (single input only)
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    output: Option<PathBuf>,

    /// Preserve date stamps when possible
    #[arg(short = 'p', long = "preserve-dates")]
    preserve_dates: bool,

    /// Verbose progress
    #[arg(short = 'v', long = "verbose")]
    verbose: bool,

    /// Version banner
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Files to strip
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("strip") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-strip");
        return ExitCode::SUCCESS;
    }
    if args.files.is_empty() {
        eprintln!("oxide-strip: no input files");
        eprintln!("Try:  oxide-strip --help");
        return ExitCode::from(1);
    }
    if args.output.is_some() && args.files.len() != 1 {
        eprintln!("oxide-strip: -o requires exactly one input file");
        return ExitCode::from(1);
    }

    let mut opts = StripOptions {
        strip_all: args.strip_all,
        strip_debug: args.strip_debug,
        strip_unneeded: args.strip_unneeded,
    };
    if !opts.wants_work() {
        opts.strip_all = true;
    }

    let mut status = Status::ok();
    for f in &args.files {
        let out = args.output.as_deref().unwrap_or(f.as_path());
        status.record(do_strip(f, out, opts, args.verbose, args.preserve_dates));
    }
    status.exit_code()
}

fn do_strip(
    input: &Path,
    output: &Path,
    opts: StripOptions,
    verbose: bool,
    preserve_dates: bool,
) -> Result<()> {
    let meta_before = if preserve_dates {
        std::fs::metadata(input).ok()
    } else {
        None
    };

    let in_place = input == output;
    let tmp = if in_place {
        let mut t = output.as_os_str().to_os_string();
        t.push(".oxide-strip.tmp");
        PathBuf::from(t)
    } else {
        output.to_path_buf()
    };

    strip_file(input, &tmp, opts)?;

    if in_place {
        std::fs::rename(&tmp, output).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            OxideError::io_path(output, e)
        })?;
    }

    if let Some(meta) = meta_before
        && let Ok(atime) = meta.accessed()
        && let Ok(mtime) = meta.modified()
    {
        let _ = filetime_set(output, atime, mtime);
    }

    if verbose {
        eprintln!("strip: {input:?} -> {output:?}");
    }
    Ok(())
}

fn filetime_set(
    path: &Path,
    atime: std::time::SystemTime,
    mtime: std::time::SystemTime,
) -> std::io::Result<()> {
    let f = std::fs::File::options().write(true).open(path)?;
    let times = std::fs::FileTimes::new()
        .set_accessed(atime)
        .set_modified(mtime);
    f.set_times(times)
}
