//! Command identity and dispatch metadata.

/// Known OxideUtils tools (GNU binutils counterparts).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ToolId {
    Objdump,
    Nm,
    Readelf,
    Strip,
    Objcopy,
    Ar,
    Ranlib,
    Size,
    Addr2line,
    Strings,
    Cxxfilt,
    Elfedit,
    As,
    Ld,
    Multicall,
    Unknown,
}

impl ToolId {
    pub fn name(self) -> &'static str {
        match self {
            Self::Objdump => "objdump",
            Self::Nm => "nm",
            Self::Readelf => "readelf",
            Self::Strip => "strip",
            Self::Objcopy => "objcopy",
            Self::Ar => "ar",
            Self::Ranlib => "ranlib",
            Self::Size => "size",
            Self::Addr2line => "addr2line",
            Self::Strings => "strings",
            Self::Cxxfilt => "c++filt",
            Self::Elfedit => "elfedit",
            Self::As => "as",
            Self::Ld => "ld",
            Self::Multicall => "oxideutils",
            Self::Unknown => "unknown",
        }
    }

    /// Name of the installed binary that implements this tool.
    ///
    /// `ranlib` is a real GNU binutils convention: it's the *same program*
    /// as `ar` (upstream builds ar.c twice with `-DIS_RANLIB`), just under a
    /// different argv0 that makes it behave like `ar -s`. `oxide-ranlib` is
    /// a second `[[bin]]` in the `oxide-ar` crate sharing `src/main.rs`,
    /// which branches on argv0 the same way (see `aliases::is_ranlib_alias`).
    pub fn binary_crate(self) -> &'static str {
        match self {
            Self::Objdump => "oxide-objdump",
            Self::Nm => "oxide-nm",
            Self::Readelf => "oxide-readelf",
            Self::Strip => "oxide-strip",
            Self::Objcopy => "oxide-objcopy",
            Self::Ar => "oxide-ar",
            Self::Ranlib => "oxide-ranlib",
            Self::Size => "oxide-size",
            Self::Addr2line => "oxide-addr2line",
            Self::Strings => "oxide-strings",
            // Real installed binary — `+` isn't a valid rustc crate name, so
            // Cargo can't build a bin literally named `oxide-c++filt`; an
            // install-time symlink provides that exact GNU-matching name.
            Self::Cxxfilt => "oxide-cxxfilt",
            Self::Elfedit => "oxide-elfedit",
            Self::As => "oxide-as",
            Self::Ld => "oxide-ld",
            Self::Multicall => "oxideutils",
            Self::Unknown => "unknown",
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "objdump" | "oxide-objdump" | "ox-objdump" => Self::Objdump,
            "nm" | "oxide-nm" | "ox-nm" => Self::Nm,
            "readelf" | "oxide-readelf" | "ox-readelf" => Self::Readelf,
            "strip" | "oxide-strip" | "ox-strip" => Self::Strip,
            "objcopy" | "oxide-objcopy" | "ox-objcopy" => Self::Objcopy,
            "ar" | "oxide-ar" | "ox-ar" => Self::Ar,
            "ranlib" | "oxide-ranlib" | "ox-ranlib" => Self::Ranlib,
            "size" | "oxide-size" | "ox-size" => Self::Size,
            "addr2line" | "oxide-addr2line" | "ox-addr2line" => Self::Addr2line,
            "strings" | "oxide-strings" | "ox-strings" => Self::Strings,
            "c++filt" | "cxxfilt" | "oxide-c++filt" | "oxide-cxxfilt" | "ox-cxxfilt" => {
                Self::Cxxfilt
            }
            "elfedit" | "oxide-elfedit" | "ox-elfedit" => Self::Elfedit,
            "as" | "oxide-as" | "ox-as" => Self::As,
            "ld" | "oxide-ld" | "ox-ld" => Self::Ld,
            "oxideutils" | "ox" => Self::Multicall,
            _ => Self::Unknown,
        }
    }
}
