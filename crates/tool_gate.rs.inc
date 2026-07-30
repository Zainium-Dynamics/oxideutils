// Included at top of each tool binary (after any module docs).
// When oxideutils.toml disables this tool (or standalone=true), exit with help.

#[cfg(oxide_tool_disabled)]
fn main() {
    let name = env!("CARGO_PKG_NAME");
    eprintln!("{name}: disabled by oxideutils.toml");
    eprintln!();
    eprintln!("  • standalone mode →  oxideutils <tool> [args…]");
    eprintln!("  • or set [tools] <name> = true, then:  cargo build --release");
    eprintln!();
    eprintln!("  See oxideutils.toml and target/oxideutils-build-plan.txt");
    std::process::exit(2);
}
