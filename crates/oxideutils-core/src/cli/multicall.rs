//! Multicall / busybox-style dispatch (`oxideutils objdump ...` or argv0).

use crate::cli::command::ToolId;
use crate::cli::help;
use crate::utils::{program_name, tool_name_from_argv0};

/// Resolve which tool to run from argv.
pub fn resolve_tool(argv: &[String]) -> (ToolId, Vec<String>) {
    let argv0 = argv.first().map(|s| s.as_str()).unwrap_or("oxideutils");
    let base = program_name(argv0);
    let as_tool = ToolId::from_name(tool_name_from_argv0(argv0));

    // If invoked as multicall binary, first arg may be subcommand.
    if matches!(as_tool, ToolId::Multicall | ToolId::Unknown)
        && (base == "oxideutils" || base == "ox")
    {
        if let Some(sub) = argv.get(1) {
            let id = ToolId::from_name(sub);
            if id != ToolId::Unknown {
                let rest = std::iter::once(format!("oxide-{}", id.name()))
                    .chain(argv.iter().skip(2).cloned())
                    .collect();
                return (id, rest);
            }
        }
        return (ToolId::Multicall, argv.to_vec());
    }

    (as_tool, argv.to_vec())
}

pub fn multicall_usage() -> String {
    help::multicall::usage()
}
