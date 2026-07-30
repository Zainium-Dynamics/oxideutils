//! Address → source location (GNU addr2line / DWARF via `addr2line` + gimli).

use crate::error::{OxideError, Result};
use crate::utils::demangle_symbol;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Addr2LineOptions {
    pub demangle: bool,
    pub functions: bool,
    pub pretty: bool,
    pub basenames: bool,
    pub inlines: bool,
}

#[derive(Debug, Clone)]
pub struct ResolvedAddress {
    pub address: u64,
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub column: Option<u32>,
    pub inlined: Vec<InlineFrame>,
}

#[derive(Debug, Clone)]
pub struct InlineFrame {
    pub function: Option<String>,
    pub file: Option<String>,
    pub line: Option<u32>,
}

/// DWARF context for repeated queries on one executable.
pub struct Addr2LineContext {
    path: PathBuf,
    #[cfg(feature = "dwarf")]
    loader: addr2line::Loader,
}

impl Addr2LineContext {
    pub fn open(path: &Path) -> Result<Self> {
        #[cfg(feature = "dwarf")]
        {
            let loader = addr2line::Loader::new(path).map_err(|e| {
                OxideError::format(
                    path.display().to_string(),
                    format!("failed to load debug info: {e}"),
                )
            })?;
            Ok(Self {
                path: path.to_path_buf(),
                loader,
            })
        }
        #[cfg(not(feature = "dwarf"))]
        {
            let _ = path;
            Err(OxideError::NotImplemented(
                "addr2line: build with `dwarf` feature (default)",
            ))
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn resolve(&self, addr: u64, opts: &Addr2LineOptions) -> Result<ResolvedAddress> {
        #[cfg(feature = "dwarf")]
        {
            resolve_with_loader(&self.loader, addr, opts)
                .map_err(|e| OxideError::format(self.path.display().to_string(), e))
        }
        #[cfg(not(feature = "dwarf"))]
        {
            let _ = (addr, opts);
            Err(OxideError::NotImplemented("dwarf feature disabled"))
        }
    }
}

#[cfg(feature = "dwarf")]
fn resolve_with_loader(
    loader: &addr2line::Loader,
    addr: u64,
    opts: &Addr2LineOptions,
) -> std::result::Result<ResolvedAddress, String> {
    let mut function = None;
    let mut file_name = None;
    let mut line = None;
    let mut column = None;
    let mut inlined = Vec::new();

    if let Ok(Some(loc)) = loader.find_location(addr) {
        file_name = loc.file.map(|p| display_path(p, opts.basenames));
        line = loc.line;
        column = loc.column;
    }

    if opts.functions {
        if let Some(sym) = loader.find_symbol(addr) {
            let s = sym.to_string();
            function = Some(if opts.demangle {
                demangle_symbol(&s)
            } else {
                s
            });
        }
    }

    if opts.inlines || opts.functions {
        if let Ok(mut iter) = loader.find_frames(addr) {
            let mut first = true;
            loop {
                match iter.next() {
                    Ok(Some(frame)) => {
                        let func = frame.function.as_ref().and_then(|f| {
                            let name = f.raw_name().ok().map(|n| n.to_string()).or_else(|| {
                                // demangle language-aware name if present
                                f.demangle()
                                    .ok()
                                    .map(|n| n.to_string())
                            });
                            name.map(|s| {
                                if opts.demangle {
                                    demangle_symbol(&s)
                                } else {
                                    s
                                }
                            })
                        });
                        let fpath = frame
                            .location
                            .as_ref()
                            .and_then(|l| l.file)
                            .map(|p| display_path(p, opts.basenames));
                        let ln = frame.location.as_ref().and_then(|l| l.line);

                        if first {
                            if function.is_none() {
                                function = func.clone();
                            }
                            if file_name.is_none() {
                                file_name = fpath.clone();
                                line = ln;
                                column = frame.location.as_ref().and_then(|l| l.column);
                            }
                            first = false;
                        } else if opts.inlines {
                            inlined.push(InlineFrame {
                                function: func,
                                file: fpath,
                                line: ln,
                            });
                        }
                    }
                    Ok(None) => break,
                    Err(_) => break,
                }
            }
        }
    }

    Ok(ResolvedAddress {
        address: addr,
        function,
        file: file_name,
        line,
        column,
        inlined,
    })
}

fn display_path(path: &str, basenames: bool) -> String {
    if basenames {
        Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(path)
            .to_string()
    } else {
        path.to_string()
    }
}

impl ResolvedAddress {
    pub fn format_gnu(&self, opts: &Addr2LineOptions) -> String {
        let mut out = String::new();
        if opts.pretty {
            if opts.functions {
                let f = self.function.as_deref().unwrap_or("??");
                let file = self.file.as_deref().unwrap_or("??");
                let line = self
                    .line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "?".into());
                writeln!(out, "{f} at {file}:{line}").ok();
            } else {
                let file = self.file.as_deref().unwrap_or("??");
                let line = self
                    .line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "?".into());
                writeln!(out, "{file}:{line}").ok();
            }
            for inl in &self.inlined {
                let f = inl.function.as_deref().unwrap_or("??");
                let file = inl.file.as_deref().unwrap_or("??");
                let line = inl
                    .line
                    .map(|l| l.to_string())
                    .unwrap_or_else(|| "?".into());
                writeln!(out, " (inlined by) {f} at {file}:{line}").ok();
            }
        } else {
            if opts.functions {
                writeln!(out, "{}", self.function.as_deref().unwrap_or("??")).ok();
            }
            let file = self.file.as_deref().unwrap_or("??");
            match self.line {
                Some(l) => {
                    writeln!(out, "{file}:{l}").ok();
                }
                None => {
                    writeln!(out, "{file}:?").ok();
                }
            }
            if opts.inlines {
                for inl in &self.inlined {
                    if opts.functions {
                        writeln!(out, "{}", inl.function.as_deref().unwrap_or("??")).ok();
                    }
                    let file = inl.file.as_deref().unwrap_or("??");
                    match inl.line {
                        Some(l) => {
                            writeln!(out, "{file}:{l}").ok();
                        }
                        None => {
                            writeln!(out, "{file}:?").ok();
                        }
                    }
                }
            }
        }
        out
    }
}
