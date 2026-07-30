//! Shared CLI framework (mirrors binutils `bucomm` + getopt patterns).

pub mod aliases;
pub mod command;
pub mod config;
pub mod help;
pub mod multicall;
pub mod parser;
pub mod utils;

pub use aliases::*;
pub use command::*;
pub use config::*;
pub use help::*;
pub use multicall::*;
pub use parser::*;
pub use utils::*;

/// Check whether **this** binary crate was disabled in `oxideutils.toml`.
///
/// Must be a macro so `option_env!` is evaluated in the **tool crate**, not in core.
#[macro_export]
macro_rules! exit_if_tool_disabled {
    ($tool:expr) => {{
        if ::core::option_env!("OXIDE_TOOL_ENABLED") == Some("0") {
            let standalone = ::core::option_env!("OXIDEUTILS_BUILD_STANDALONE") == Some("true");
            let tool = $tool;
            eprintln!("oxide-{tool}: disabled by oxideutils.toml (build-time config)");
            eprintln!();
            if standalone {
                eprintln!("  build.standalone = true  →  one binary mode:");
                eprintln!("    oxideutils {tool} [args…]");
                eprintln!("    ./target/release/oxideutils {tool} …");
            } else {
                eprintln!("  Enable in oxideutils.toml:");
                eprintln!("    [tools]");
                eprintln!("    {tool} = true");
                eprintln!();
                eprintln!("  Then rebuild:  cargo build --release");
            }
            eprintln!();
            eprintln!("  Plan file: target/oxideutils-build-plan.txt");
            true
        } else {
            false
        }
    }};
}
