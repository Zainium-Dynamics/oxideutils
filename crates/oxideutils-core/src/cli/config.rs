//! Runtime + TOML configuration for OxideUtils (Zainium Dynamics).
//!
//! We use **TOML** (not `.configuration` / ini). Booleans are plain `true` / `false`.
//!
//! Search order (first found wins for file load; env can still override):
//! 1. `$OXIDEUTILS_CONFIG` (path to a `.toml` file)
//! 2. `./oxideutils.toml`
//! 3. `./config/oxideutils.toml`
//! 4. `$XDG_CONFIG_HOME/oxideutils/config.toml`
//! 5. `~/.config/oxideutils/config.toml`

use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// TOML schema — all switches prefer `true` / `false`
// ---------------------------------------------------------------------------

/// Root document: `oxideutils.toml`
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct OxideToml {
    /// Global defaults shared by every tool.
    pub oxideutils: GlobalSection,
    /// Colour / terminal output.
    pub color: ColorSection,
    /// Disassembly defaults (objdump).
    pub disasm: DisasmSection,
    /// objdump-specific.
    pub objdump: ObjdumpSection,
    /// nm-specific.
    pub nm: NmSection,
    /// readelf-specific.
    pub readelf: ReadelfSection,
    /// strip / objcopy safety defaults.
    pub mutate: MutateSection,
    /// addr2line defaults.
    pub addr2line: Addr2lineSection,
    /// Logging / diagnostics.
    pub log: LogSection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GlobalSection {
    /// Prefer GNU-compatible text output shapes.
    pub gnu_compatible: bool,
    /// Emit machine-readable JSON where a tool supports it.
    pub json: bool,
    /// Default demangle for symbols / addr2line / objdump -t.
    pub demangle: bool,
    /// Wide columns when a tool supports it.
    pub wide: bool,
    /// Parallel multi-file when implemented (reserved).
    pub parallel: bool,
    /// Continue multi-file tools after the first error.
    pub continue_on_error: bool,
    /// Verbose diagnostics to stderr.
    pub verbose: bool,
    /// Quiet: suppress non-essential stderr.
    pub quiet: bool,
}

impl Default for GlobalSection {
    fn default() -> Self {
        Self {
            gnu_compatible: true,
            json: false,
            demangle: false,
            wide: false,
            parallel: false,
            continue_on_error: true,
            verbose: false,
            quiet: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ColorSection {
    /// Master switch: if false, never colour.
    pub enabled: bool,
    /// If true and `enabled`, only colour when stderr is a TTY.
    pub auto: bool,
    /// Force colour even when not a TTY (ignored if `enabled = false`).
    pub always: bool,
}

impl Default for ColorSection {
    fn default() -> Self {
        Self {
            enabled: true,
            auto: true,
            always: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DisasmSection {
    pub show_raw_insn: bool,
    pub disassemble_zeroes: bool,
    /// Use AT&T / gas-like syntax when backend allows.
    pub gas_syntax: bool,
    pub uppercase_hex: bool,
}

impl Default for DisasmSection {
    fn default() -> Self {
        Self {
            show_raw_insn: true,
            disassemble_zeroes: false,
            gas_syntax: true,
            uppercase_hex: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ObjdumpSection {
    pub demangle: bool,
    pub wide: bool,
    /// If true, `-d` without other display flags is enough (already true in CLI).
    pub allow_disassemble_only: bool,
}

impl Default for ObjdumpSection {
    fn default() -> Self {
        Self {
            demangle: false,
            wide: false,
            allow_disassemble_only: true,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NmSection {
    pub demangle: bool,
    pub numeric_sort: bool,
    pub size_sort: bool,
    pub reverse: bool,
    pub print_size: bool,
    pub print_file_name: bool,
    pub extern_only: bool,
    pub undefined_only: bool,
    pub defined_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ReadelfSection {
    pub all_by_default: bool,
    pub show_notes: bool,
}

impl Default for ReadelfSection {
    fn default() -> Self {
        Self {
            all_by_default: false,
            show_notes: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MutateSection {
    /// strip: default to strip-all when no mode flags given.
    pub strip_all_default: bool,
    /// Prefer temp file + rename for in-place writes.
    pub safe_inplace: bool,
    /// Preserve timestamps when tool supports it.
    pub preserve_dates: bool,
}

impl Default for MutateSection {
    fn default() -> Self {
        Self {
            strip_all_default: true,
            safe_inplace: true,
            preserve_dates: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Addr2lineSection {
    pub demangle: bool,
    pub functions: bool,
    pub pretty: bool,
    pub basenames: bool,
    pub inlines: bool,
    pub show_addresses: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LogSection {
    pub verbose: bool,
    pub quiet: bool,
}

// ---------------------------------------------------------------------------
// Load / paths
// ---------------------------------------------------------------------------

impl OxideToml {
    /// Built-in defaults (all sections).
    pub fn defaults() -> Self {
        Self::default()
    }

    /// Parse TOML text.
    pub fn parse_str(s: &str) -> Result<Self, String> {
        toml::from_str(s).map_err(|e| format!("oxideutils.toml: {e}"))
    }

    /// Load from an explicit file path.
    pub fn load_file(path: &Path) -> Result<Self, String> {
        let text = fs::read_to_string(path)
            .map_err(|e| format!("oxideutils.toml: cannot read {}: {e}", path.display()))?;
        Self::parse_str(&text)
    }

    /// Discover and load the first existing config file; else defaults.
    pub fn load() -> Self {
        if let Some(path) = Self::discover_path() {
            match Self::load_file(&path) {
                Ok(cfg) => return cfg.apply_env_overrides(),
                Err(e) => {
                    eprintln!("oxideutils: warning: {e} (using defaults)");
                }
            }
        }
        Self::defaults().apply_env_overrides()
    }

    /// Same as [`load`] but also returns which file was used (if any).
    pub fn load_with_source() -> (Self, Option<PathBuf>) {
        if let Some(path) = Self::discover_path() {
            match Self::load_file(&path) {
                Ok(cfg) => return (cfg.apply_env_overrides(), Some(path)),
                Err(e) => eprintln!("oxideutils: warning: {e} (using defaults)"),
            }
        }
        (Self::defaults().apply_env_overrides(), None)
    }

    /// Ordered search for a config file.
    pub fn discover_path() -> Option<PathBuf> {
        if let Ok(p) = env::var("OXIDEUTILS_CONFIG") {
            let pb = PathBuf::from(p);
            if pb.is_file() {
                return Some(pb);
            }
        }
        let candidates = [
            PathBuf::from("oxideutils.toml"),
            PathBuf::from("config/oxideutils.toml"),
        ];
        for c in candidates {
            if c.is_file() {
                return Some(c);
            }
        }
        if let Ok(xdg) = env::var("XDG_CONFIG_HOME") {
            let p = PathBuf::from(xdg).join("oxideutils/config.toml");
            if p.is_file() {
                return Some(p);
            }
        }
        if let Ok(home) = env::var("HOME") {
            let p = PathBuf::from(home).join(".config/oxideutils/config.toml");
            if p.is_file() {
                return Some(p);
            }
        }
        None
    }

    /// Serialize current config to pretty TOML (for `--print-config` / docs).
    pub fn to_toml_string(&self) -> Result<String, String> {
        toml::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Environment overrides (applied after file). Still booleans via presence / values.
    pub fn apply_env_overrides(mut self) -> Self {
        if env::var_os("NO_COLOR").is_some() {
            self.color.enabled = false;
            self.color.always = false;
        }
        if let Ok(v) = env::var("OXIDEUTILS_COLOR") {
            match v.as_str() {
                "always" | "1" | "true" => {
                    self.color.enabled = true;
                    self.color.always = true;
                    self.color.auto = false;
                }
                "never" | "0" | "false" => {
                    self.color.enabled = false;
                }
                "auto" => {
                    self.color.enabled = true;
                    self.color.auto = true;
                    self.color.always = false;
                }
                _ => {}
            }
        }
        if env_truthy("OXIDEUTILS_JSON") {
            self.oxideutils.json = true;
        }
        if env_truthy("OXIDEUTILS_ENHANCED") {
            self.oxideutils.gnu_compatible = false;
        }
        if env_truthy("OXIDEUTILS_DEMANGLE") {
            self.oxideutils.demangle = true;
            self.objdump.demangle = true;
            self.nm.demangle = true;
            self.addr2line.demangle = true;
        }
        if env_truthy("OXIDEUTILS_VERBOSE") {
            self.oxideutils.verbose = true;
            self.log.verbose = true;
        }
        if env_truthy("OXIDEUTILS_QUIET") {
            self.oxideutils.quiet = true;
            self.log.quiet = true;
        }
        self
    }

    pub fn use_color(&self) -> bool {
        if !self.color.enabled {
            return false;
        }
        if self.color.always {
            return true;
        }
        if self.color.auto {
            return atty_stderr();
        }
        true
    }
}

fn env_truthy(key: &str) -> bool {
    match env::var(key) {
        Ok(v) if v.is_empty() => true,
        Ok(v) => matches!(
            v.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        ),
        Err(env::VarError::NotPresent) => false,
        Err(_) => false,
    }
}

fn atty_stderr() -> bool {
    use std::io::IsTerminal;
    std::io::stderr().is_terminal()
}

// ---------------------------------------------------------------------------
// Backward-compatible RuntimeConfig façade
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    pub color: ColorMode,
    pub json: bool,
    pub gnu_compatible: bool,
    pub wide: bool,
    pub demangle: bool,
    pub verbose: bool,
    pub quiet: bool,
    /// Full TOML tree when loaded.
    pub toml: OxideToml,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self::from_toml(OxideToml::defaults())
    }
}

impl RuntimeConfig {
    pub fn from_toml(t: OxideToml) -> Self {
        let color = if !t.color.enabled {
            ColorMode::Never
        } else if t.color.always {
            ColorMode::Always
        } else if t.color.auto {
            ColorMode::Auto
        } else {
            ColorMode::Always
        };
        Self {
            color,
            json: t.oxideutils.json,
            gnu_compatible: t.oxideutils.gnu_compatible,
            wide: t.oxideutils.wide,
            demangle: t.oxideutils.demangle,
            verbose: t.oxideutils.verbose || t.log.verbose,
            quiet: t.oxideutils.quiet || t.log.quiet,
            toml: t,
        }
    }

    /// Load TOML file (if any) + env overrides.
    pub fn load() -> Self {
        Self::from_toml(OxideToml::load())
    }

    /// Env-only (legacy name).
    pub fn from_env() -> Self {
        Self::from_toml(OxideToml::defaults().apply_env_overrides())
    }

    pub fn use_color(&self) -> bool {
        match self.color {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => atty_stderr(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_bools_from_toml() {
        let s = r#"
[oxideutils]
gnu_compatible = true
json = false
demangle = true

[color]
enabled = true
auto = true
always = false

[disasm]
show_raw_insn = true
disassemble_zeroes = false
"#;
        let c = OxideToml::parse_str(s).unwrap();
        assert!(c.oxideutils.gnu_compatible);
        assert!(!c.oxideutils.json);
        assert!(c.oxideutils.demangle);
        assert!(c.disasm.show_raw_insn);
        assert!(!c.disasm.disassemble_zeroes);
    }

    #[test]
    fn defaults_roundtrip() {
        let t = OxideToml::defaults();
        let s = t.to_toml_string().unwrap();
        let t2 = OxideToml::parse_str(&s).unwrap();
        assert!(t2.oxideutils.gnu_compatible);
    }
}
