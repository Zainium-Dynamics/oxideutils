//! Error types — work on both `std` and `no_std` (+ `alloc`).

use core::fmt;

#[cfg(feature = "alloc")]
use alloc::string::String;

// When only `std` is on, `alloc` feature is enabled via Cargo.toml.

/// Primary error type used across tools and the core library.
#[derive(Debug)]
pub enum OxideError {
    /// I/O failure (std only).
    #[cfg(feature = "std")]
    Io {
        path: String,
        source: std::io::Error,
    },

    UnrecognizedFormat {
        path: String,
    },

    Format {
        path: String,
        message: String,
    },

    Object(String),

    NoInputFiles,

    NoDisplaySwitch,

    UnknownOption(String),

    InvalidArgument(String),

    Tool {
        tool: &'static str,
        message: String,
    },

    SectionNotFound(String),

    SymbolNotFound(String),

    NotImplemented(&'static str),

    #[cfg(feature = "std")]
    Other(String),
}

impl fmt::Display for OxideError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            #[cfg(feature = "std")]
            Self::Io { path, source } => write!(f, "oxideutils: '{path}': {source}"),
            Self::UnrecognizedFormat { path } => {
                write!(f, "oxideutils: {path}: file format not recognized")
            }
            Self::Format { path, message } => write!(f, "oxideutils: {path}: {message}"),
            Self::Object(m) => write!(f, "oxideutils: {m}"),
            Self::NoInputFiles => write!(f, "oxideutils: no input files specified"),
            Self::NoDisplaySwitch => {
                write!(
                    f,
                    "oxideutils: at least one of the display switches must be given"
                )
            }
            Self::UnknownOption(o) => write!(f, "oxideutils: unknown option '{o}'"),
            Self::InvalidArgument(m) => write!(f, "oxideutils: invalid argument: {m}"),
            Self::Tool { tool, message } => write!(f, "oxideutils: {tool}: {message}"),
            Self::SectionNotFound(s) => write!(f, "oxideutils: section '{s}' not found"),
            Self::SymbolNotFound(s) => write!(f, "oxideutils: symbol '{s}' not found"),
            Self::NotImplemented(m) => write!(f, "oxideutils: not implemented yet: {m}"),
            #[cfg(feature = "std")]
            Self::Other(m) => write!(f, "oxideutils: {m}"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for OxideError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

impl OxideError {
    #[cfg(feature = "std")]
    pub fn io(path: impl Into<String>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    #[cfg(feature = "std")]
    pub fn io_path(path: &std::path::Path, source: std::io::Error) -> Self {
        Self::Io {
            path: path.display().to_string(),
            source,
        }
    }

    pub fn format(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Format {
            path: path.into(),
            message: message.into(),
        }
    }

    pub fn object(msg: impl Into<String>) -> Self {
        Self::Object(msg.into())
    }

    pub fn tool(tool: &'static str, message: impl Into<String>) -> Self {
        Self::Tool {
            tool,
            message: message.into(),
        }
    }

    /// Map to process exit code (GNU-compatible: 0 ok, 1 soft, 2 hard).
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::NoInputFiles
            | Self::NoDisplaySwitch
            | Self::UnknownOption(_)
            | Self::InvalidArgument(_) => 2,
            _ => 1,
        }
    }
}

pub type Result<T> = core::result::Result<T, OxideError>;

/// Print error to stderr (std only).
#[cfg(feature = "std")]
pub fn eprint_error(err: &OxideError) {
    eprintln!("{err}");
}

#[cfg(feature = "std")]
impl From<anyhow::Error> for OxideError {
    fn from(e: anyhow::Error) -> Self {
        Self::Other(e.to_string())
    }
}

#[cfg(feature = "std")]
impl From<std::io::Error> for OxideError {
    fn from(e: std::io::Error) -> Self {
        Self::Io {
            path: String::from("<io>"),
            source: e,
        }
    }
}
