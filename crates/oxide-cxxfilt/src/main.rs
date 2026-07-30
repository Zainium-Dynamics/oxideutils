//! oxide-c++filt — demangle C++ (Itanium ABI) and Rust symbol names.
//!
//! Two `[[bin]]` targets share this file: `oxide-c++filt` (GNU-matching
//! name) and `oxide-cxxfilt` (ASCII-safe alias) — same behavior either way.

use clap::Parser;
use oxideutils_core::cli::help::{self, VERSION, print_version};
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-c++filt",
    about = help::cxxfilt::ABOUT,
    long_about = help::cxxfilt::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Don't strip a leading underscore before demangling (GNU compat; the
    /// Itanium/Rust manglings we understand don't use one, so this is a no-op).
    #[arg(short = 'n', long = "no-strip-underscore")]
    no_strip_underscore: bool,

    /// Strip a leading underscore before demangling (GNU compat; no-op here).
    #[arg(long = "strip-underscore", short = '_')]
    strip_underscore: bool,

    /// Omit function parameters from the demangled output (C++ symbols only).
    #[arg(short = 'p', long = "no-params")]
    no_params: bool,

    /// Also try to demangle type names, not just function/data symbols.
    /// GNU compat; our demangler already handles both, so this is a no-op.
    #[arg(short = 't', long = "types")]
    types: bool,

    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Symbols to demangle. If none are given, read lines from stdin and
    /// demangle any mangled-looking identifiers found within each line.
    #[arg(value_name = "SYMBOL")]
    symbols: Vec<String>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("c++filt") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-c++filt");
        return ExitCode::SUCCESS;
    }
    let _ = (args.no_strip_underscore, args.strip_underscore, args.types);

    let opts = cpp_demangle::DemangleOptions::new();
    let opts = if args.no_params {
        opts.no_params()
    } else {
        opts
    };

    if !args.symbols.is_empty() {
        for sym in &args.symbols {
            println!("{}", demangle_one(sym, &opts));
        }
        return ExitCode::SUCCESS;
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let _ = writeln!(out, "{}", demangle_line(&line, &opts));
    }
    ExitCode::SUCCESS
}

/// Try Rust (v0 + legacy) first, then Itanium C++; fall through unchanged.
fn demangle_one(sym: &str, cpp_opts: &cpp_demangle::DemangleOptions) -> String {
    if let Ok(d) = rustc_demangle::try_demangle(sym) {
        return d.to_string();
    }
    if let Ok(parsed) = cpp_demangle::Symbol::new(sym)
        && let Ok(demangled) = parsed.demangle(cpp_opts)
    {
        return demangled;
    }
    sym.to_string()
}

/// GNU c++filt's real behavior: scan a line of arbitrary text (e.g. a
/// linker error) and demangle just the identifier-shaped substrings,
/// passing every other character through untouched.
fn demangle_line(line: &str, cpp_opts: &cpp_demangle::DemangleOptions) -> String {
    let is_ident = |c: char| c.is_alphanumeric() || matches!(c, '_' | '$' | '.');
    let mut out = String::with_capacity(line.len());
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if is_ident(chars[i]) {
            let start = i;
            while i < chars.len() && is_ident(chars[i]) {
                i += 1;
            }
            let token: String = chars[start..i].iter().collect();
            out.push_str(&demangle_one(&token, cpp_opts));
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passes_through_plain_text() {
        let opts = cpp_demangle::DemangleOptions::new();
        assert_eq!(demangle_line("hello world 123", &opts), "hello world 123");
    }

    #[test]
    fn demangles_embedded_itanium_symbol() {
        let opts = cpp_demangle::DemangleOptions::new();
        // _Z3fooi -> foo(int)
        let line = "undefined reference to `_Z3fooi'";
        let out = demangle_line(line, &opts);
        assert!(out.contains("foo(int)"), "{out}");
    }
}
