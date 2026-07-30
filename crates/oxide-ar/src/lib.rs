//! oxide-ar library — shared entry points for `oxide-ar` and `oxide-ranlib`.
//!
//! Mirrors GNU binutils 2.46.1 layout:
//! - `binutils/ar.c` — single implementation
//! - `binutils/is-ranlib.c`  — `#define is_ranlib 1` + include ar.c  → ranlib binary
//! - `binutils/not-ranlib.c` — `#define is_ranlib 0` + include ar.c  → ar binary
//! - `binutils/maybe-ranlib.c` — `is_ranlib = -1` runtime argv0 dispatch
//!
//! Our bins:
//! - `oxide-ar`     → `maybe_main` (argv0 may still be ranlib via hardlink)
//! - `oxide-ranlib` → `ranlib_main` (like is-ranlib.c)

use oxideutils_core::archive_write::{ArOperation, run_ar};
use oxideutils_core::cli::aliases::is_ranlib_alias;
use oxideutils_core::cli::help::{self, VERSION, print_version};
use oxideutils_core::error::{OxideError, Result};
use oxideutils_core::utils::program_name;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Parser;

// ---------------------------------------------------------------------------
// ar CLI  (binutils/ar.c — decode_options + main non-ranlib path)
// ---------------------------------------------------------------------------

#[derive(Debug, Parser)]
#[command(
    name = "oxide-ar",
    about = help::ar::ABOUT,
    long_about = help::ar::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Version banner
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// GNU-style: KEY archive [members…]
    /// Example: rcs lib.a a.o b.o   |   t lib.a   |   x lib.a
    #[arg(value_name = "KEY_OR_FILE")]
    positional: Vec<String>,
}

// ---------------------------------------------------------------------------
// ranlib CLI  (binutils/ar.c — ranlib_main, flags DhHUvVt)
// ---------------------------------------------------------------------------

/// `ranlib [-DhHvVt] archive...` — equivalent to `ar -s` on each archive.
/// See binutils/doc/ranlib.1 and ar.c:ranlib_main.
#[derive(Debug, Default, Parser)]
#[command(
    name = "oxide-ranlib",
    about = help::ranlib::ABOUT,
    long_about = help::ranlib::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct RanlibArgs {
    /// Deterministic mode: zero uid/gid/timestamp in the symbol map (`-D`).
    #[arg(short = 'D')]
    deterministic: bool,

    /// Inverse of -D: real uid/gid/timestamp (default) (`-U`).
    #[arg(short = 'U')]
    non_deterministic: bool,

    /// Update the timestamp of the symbol map of an archive (`-t`).
    #[arg(short = 't')]
    touch: bool,

    /// Verbose (`-v`).
    #[arg(short = 'v')]
    verbose: bool,

    #[arg(short = 'V', long = "version")]
    version: bool,

    #[arg(value_name = "ARCHIVE")]
    archives: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Entry points
// ---------------------------------------------------------------------------

/// like binutils `maybe-ranlib.c`: decide at runtime from argv0.
pub fn maybe_main() -> ExitCode {
    let argv0 = std::env::args().next().unwrap_or_default();
    if is_ranlib_alias(program_name(&argv0)) {
        return ranlib_main();
    }
    ar_main()
}

/// like binutils `not-ranlib.c` / ar path of main().
pub fn ar_main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("ar") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-ar");
        return ExitCode::SUCCESS;
    }

    if args.positional.is_empty() {
        use clap::CommandFactory;
        let mut cmd = Args::command();
        let _ = cmd.print_help();
        println!();
        return ExitCode::from(2);
    }

    match gnu_style(&args.positional) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            eprintln!("Try:  oxide-ar --help");
            ExitCode::from(e.exit_code() as u8)
        }
    }
}

/// like binutils `is-ranlib.c` / ar.c:ranlib_main.
pub fn ranlib_main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("ranlib") {
        return ExitCode::from(2);
    }
    let args = RanlibArgs::parse();
    if args.version {
        print_version("oxide-ranlib");
        return ExitCode::SUCCESS;
    }
    if args.archives.is_empty() {
        use clap::CommandFactory;
        let mut cmd = RanlibArgs::command();
        let _ = cmd.print_help();
        println!();
        return ExitCode::from(2);
    }

    // ar.c:ranlib_main — `-t` calls ranlib_touch; we always rebuild the index
    // (superset of touch). Deterministic follows `-D` / `-U` like ar.c.
    let _ = args.touch;
    let op = ArOperation {
        delete: false,
        print: false,
        quick_append: false,
        replace: false,
        table: false,
        extract: false,
        symbol_index: true,
        create: false,
        verbose: args.verbose,
        deterministic: args.deterministic && !args.non_deterministic,
    };

    for archive in &args.archives {
        if args.verbose {
            eprintln!("oxide-ranlib: {}", archive.display());
        }
        if let Err(e) = run_ar(&op, archive, &[], &[]) {
            eprintln!("{e}");
            eprintln!("Try:  oxide-ranlib --help");
            return ExitCode::from(e.exit_code() as u8);
        }
    }
    ExitCode::SUCCESS
}

/// GNU-style key+archive parsing (ar.c decode_options + operation dispatch).
fn gnu_style(pos: &[String]) -> Result<()> {
    if pos.is_empty() {
        return Err(OxideError::tool(
            "ar",
            "usage: oxide-ar [-]{dmpqrstx}[cDsuv] archive-file [member...]",
        ));
    }
    let key = pos[0].trim_start_matches('-');
    if key == "V" || key == "version" {
        print_version("oxide-ar");
        return Ok(());
    }

    let op = ArOperation::parse_key(key)?;
    let archive = pos
        .get(1)
        .ok_or_else(|| OxideError::tool("ar", "archive file required (see oxide-ar --help)"))?;
    let rest: Vec<PathBuf> = pos[2..].iter().map(PathBuf::from).collect();

    let member_names: Vec<String> = if op.delete || op.extract || op.print {
        rest.iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    } else {
        Vec::new()
    };
    let files: Vec<PathBuf> = if op.replace || op.quick_append {
        rest
    } else if op.delete {
        Vec::new()
    } else {
        rest
    };

    run_ar(&op, &PathBuf::from(archive), &files, &member_names)
}
