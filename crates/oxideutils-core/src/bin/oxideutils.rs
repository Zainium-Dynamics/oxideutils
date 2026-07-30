//! Multicall entry: `oxideutils <tool> [args...]`

use oxideutils_core::cli::command::ToolId;
use oxideutils_core::cli::help::print_version;
use oxideutils_core::cli::multicall::{multicall_usage, resolve_tool};
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let argv: Vec<String> = std::env::args().collect();
    let (tool, rest) = resolve_tool(&argv);

    if matches!(tool, ToolId::Multicall | ToolId::Unknown) {
        if rest.len() <= 1
            || rest.get(1).map(|s| s.as_str()) == Some("--help")
            || rest.get(1).map(|s| s.as_str()) == Some("-h")
        {
            print!("{}", multicall_usage());
            return ExitCode::SUCCESS;
        }
        if rest.get(1).map(|s| s.as_str()) == Some("--version")
            || rest.get(1).map(|s| s.as_str()) == Some("-V")
            || rest.get(1).map(|s| s.as_str()) == Some("-v")
        {
            print_version("oxideutils");
            return ExitCode::SUCCESS;
        }
        if rest.len() > 1 {
            eprintln!(
                "oxideutils: unknown tool '{}'\n\
                 Try:  oxideutils --help\n",
                rest[1]
            );
        } else {
            eprintln!("oxideutils: missing tool name. Try:  oxideutils --help\n");
        }
        print!("{}", multicall_usage());
        return ExitCode::from(2);
    }

    // Prefer re-exec of sibling binaries on PATH / same directory.
    let bin = tool.binary_crate();
    let args: Vec<&str> = rest.iter().skip(1).map(|s| s.as_str()).collect();
    match Command::new(bin).args(&args).status() {
        Ok(st) => {
            if let Some(code) = st.code() {
                ExitCode::from(code as u8)
            } else {
                ExitCode::FAILURE
            }
        }
        Err(e) => {
            eprintln!(
                "oxideutils: failed to execute {bin}: {e}\n\
                 Build and install oxide-* tools, or run them directly."
            );
            ExitCode::from(127)
        }
    }
}
