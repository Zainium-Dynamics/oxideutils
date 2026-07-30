//! GNU-compatible short/long option aliases.

/// Map common GNU aliases to canonical long names where tools share meaning.
pub fn normalize_alias(opt: &str) -> &str {
    match opt {
        "-h" => "--section-headers", // context-dependent; tools override
        "-H" => "--help",
        "-v" | "-V" => "--version",
        other => other,
    }
}

/// Tools that are hardlinks / argv0 aliases of each other in GNU.
pub fn is_ranlib_alias(name: &str) -> bool {
    matches!(name, "ranlib" | "oxide-ranlib" | "ox-ranlib")
}
