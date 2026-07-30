//! Shared build-script logic for OxideUtils.
//!
//! Each crate's `build.rs` does:
//! ```ignore
//! #[path = "../../build/oxide_build.rs"]
//! mod oxide_build;
//! fn main() { oxide_build::for_package("objdump"); }
//! ```
//!
//! Reads **oxideutils.toml** at the workspace root (or `OXIDEUTILS_CONFIG`) and:
//! - emits `cargo:rustc-cfg=...` for tools / standalone / features
//! - applies static CRT linking when requested
//! - re-runs on TOML changes

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct BuildConfig {
    pub standalone: bool,
    pub static_link: bool,
    pub dynamic_link: bool,
    pub kernel: bool,
    pub lto_note: bool,
    pub disasm: bool,
    pub disasm_aarch64: bool,
    pub dwarf: bool,
    pub color: bool,
    pub json: bool,
    pub tools: ToolsConfig,
}

#[derive(Debug, Clone)]
pub struct ToolsConfig {
    pub objdump: bool,
    pub nm: bool,
    pub readelf: bool,
    pub size: bool,
    pub strings: bool,
    pub ar: bool,
    pub strip: bool,
    pub objcopy: bool,
    pub addr2line: bool,
    pub cxxfilt: bool,
    pub elfedit: bool,
    pub multicall: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        Self {
            standalone: false,
            static_link: false,
            dynamic_link: true,
            kernel: false,
            lto_note: true,
            disasm: true,
            disasm_aarch64: true,
            dwarf: true,
            color: true,
            json: true,
            tools: ToolsConfig::default(),
        }
    }
}

impl Default for ToolsConfig {
    fn default() -> Self {
        Self {
            objdump: true,
            nm: true,
            readelf: true,
            size: true,
            strings: true,
            ar: true,
            strip: true,
            objcopy: true,
            addr2line: true,
            cxxfilt: true,
            elfedit: true,
            multicall: true,
        }
    }
}

/// Entry for tool crates (`objdump`, `nm`, …) or `core` / `multicall`.
pub fn for_package(package: &str) {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let root = find_workspace_root(&manifest_dir);
    let toml_path = find_toml(&root);

    println!("cargo:rerun-if-env-changed=OXIDEUTILS_CONFIG");
    if let Some(ref p) = toml_path {
        println!("cargo:rerun-if-changed={}", p.display());
    }
    // Also watch repo root template
    let default_toml = root.join("oxideutils.toml");
    if default_toml.exists() {
        println!("cargo:rerun-if-changed={}", default_toml.display());
    }

    let cfg = toml_path
        .as_ref()
        .and_then(|p| load_toml(p).ok())
        .unwrap_or_default();

    apply_link_mode(&cfg);
    apply_feature_cfgs(&cfg);
    apply_tool_gate(package, &cfg);
    apply_standalone(&cfg);

    // Expose path for runtime (optional)
    if let Some(ref p) = toml_path {
        println!("cargo:rustc-env=OXIDEUTILS_BUILD_TOML={}", p.display());
    }
    println!(
        "cargo:rustc-env=OXIDEUTILS_BUILD_STANDALONE={}",
        cfg.standalone
    );
    println!(
        "cargo:rustc-env=OXIDEUTILS_BUILD_STATIC={}",
        cfg.static_link
    );

    // Human-readable plan (host side)
    if package == "core" {
        write_build_plan(&root, &cfg, toml_path.as_deref());
    }
}

fn find_workspace_root(start: &Path) -> PathBuf {
    let mut dir = start.to_path_buf();
    loop {
        if dir.join("Cargo.toml").exists() && dir.join("crates").is_dir() {
            // workspace root has crates/
            return dir;
        }
        if dir.join("oxideutils.toml").exists() && dir.join("Cargo.toml").exists() {
            return dir;
        }
        if !dir.pop() {
            break;
        }
    }
    // oxideutils-core is crates/oxideutils-core → parent.parent
    start
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(start)
        .to_path_buf()
}

fn find_toml(root: &Path) -> Option<PathBuf> {
    if let Ok(p) = env::var("OXIDEUTILS_CONFIG") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    for rel in ["oxideutils.toml", "config/oxideutils.toml"] {
        let p = root.join(rel);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn load_toml(path: &Path) -> Result<BuildConfig, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    parse_build_toml(&text)
}

/// Minimal TOML subset parser for [build] / [tools] / [features] booleans.
/// Avoids pulling serde into every build script.
fn parse_build_toml(text: &str) -> Result<BuildConfig, String> {
    let mut cfg = BuildConfig::default();
    let mut section = String::new();

    for raw in text.lines() {
        let line = raw.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }
        let Some((k, v)) = line.split_once('=') else {
            continue;
        };
        let key = k.trim();
        let val = parse_bool(v.trim()).unwrap_or(false);

        match section.as_str() {
            "build" => match key {
                "standalone" => cfg.standalone = val,
                "static" | "static_link" => {
                    cfg.static_link = val;
                    if val {
                        cfg.dynamic_link = false;
                    }
                }
                "dynamic" | "dynamic_link" => {
                    cfg.dynamic_link = val;
                    if val {
                        cfg.static_link = false;
                    }
                }
                "kernel" | "no_std_kernel" => cfg.kernel = val,
                "lto" => cfg.lto_note = val,
                _ => {}
            },
            "features" => match key {
                "disasm" => cfg.disasm = val,
                "disasm_aarch64" | "aarch64" => cfg.disasm_aarch64 = val,
                "dwarf" | "addr2line" => cfg.dwarf = val,
                "color" => cfg.color = val,
                "json" => cfg.json = val,
                _ => {}
            },
            "tools" => match key {
                "objdump" => cfg.tools.objdump = val,
                "nm" => cfg.tools.nm = val,
                "readelf" => cfg.tools.readelf = val,
                "size" => cfg.tools.size = val,
                "strings" => cfg.tools.strings = val,
                "ar" => cfg.tools.ar = val,
                "strip" => cfg.tools.strip = val,
                "objcopy" => cfg.tools.objcopy = val,
                "addr2line" => cfg.tools.addr2line = val,
                "cxxfilt" | "c++filt" => cfg.tools.cxxfilt = val,
                "elfedit" => cfg.tools.elfedit = val,
                "multicall" | "oxideutils" => cfg.tools.multicall = val,
                _ => {}
            },
            _ => {}
        }
    }

    // Consistency: static wins over dynamic if both true
    if cfg.static_link {
        cfg.dynamic_link = false;
    }
    if !cfg.static_link && !cfg.dynamic_link {
        cfg.dynamic_link = true;
    }

    Ok(cfg)
}

fn parse_bool(s: &str) -> Option<bool> {
    match s.trim().trim_matches('"').to_ascii_lowercase().as_str() {
        "true" | "1" | "yes" | "on" => Some(true),
        "false" | "0" | "no" | "off" => Some(false),
        _ => None,
    }
}

fn apply_link_mode(cfg: &BuildConfig) {
    if cfg.static_link {
        println!("cargo:rustc-cfg=oxide_static_link");
        println!("cargo:rustc-env=OXIDE_LINK_MODE=static");
        // Full static works cleanly on musl; on glibc we only set the cfg + warn.
        let env = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
        let os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
        if os == "linux" && env == "musl" {
            println!("cargo:rustc-link-arg=-static");
        } else if os == "linux" {
            println!(
                "cargo:warning=build.static=true: for a fully static binary use \
                 `cargo build --release --target x86_64-unknown-linux-musl` \
                 (or aarch64-unknown-linux-musl)."
            );
        }
    } else {
        println!("cargo:rustc-cfg=oxide_dynamic_link");
        println!("cargo:rustc-env=OXIDE_LINK_MODE=dynamic");
    }
}

fn apply_feature_cfgs(cfg: &BuildConfig) {
    if cfg.disasm {
        println!("cargo:rustc-cfg=oxide_feat_disasm");
    }
    if cfg.disasm_aarch64 {
        println!("cargo:rustc-cfg=oxide_feat_disasm_aarch64");
    }
    if cfg.dwarf {
        println!("cargo:rustc-cfg=oxide_feat_dwarf");
    }
    if cfg.color {
        println!("cargo:rustc-cfg=oxide_feat_color");
    }
    if cfg.json {
        println!("cargo:rustc-cfg=oxide_feat_json");
    }
    if cfg.kernel {
        println!("cargo:rustc-cfg=oxide_feat_kernel");
    }
}

fn apply_standalone(cfg: &BuildConfig) {
    if cfg.standalone {
        println!("cargo:rustc-cfg=oxide_standalone");
    }
}

fn apply_tool_gate(package: &str, cfg: &BuildConfig) {
    let enabled = match package {
        "core" | "multicall" => cfg.tools.multicall || cfg.standalone,
        "objdump" => cfg.tools.objdump && !cfg.standalone,
        "nm" => cfg.tools.nm && !cfg.standalone,
        "readelf" => cfg.tools.readelf && !cfg.standalone,
        "size" => cfg.tools.size && !cfg.standalone,
        "strings" => cfg.tools.strings && !cfg.standalone,
        "ar" => cfg.tools.ar && !cfg.standalone,
        "strip" => cfg.tools.strip && !cfg.standalone,
        "objcopy" => cfg.tools.objcopy && !cfg.standalone,
        "addr2line" => cfg.tools.addr2line && !cfg.standalone,
        "cxxfilt" => cfg.tools.cxxfilt && !cfg.standalone,
        "elfedit" => cfg.tools.elfedit && !cfg.standalone,
        _ => true,
    };

    if !enabled {
        println!("cargo:rustc-cfg=oxide_tool_disabled");
        println!("cargo:rustc-env=OXIDE_TOOL_ENABLED=0");
    } else {
        println!("cargo:rustc-env=OXIDE_TOOL_ENABLED=1");
    }

    // When standalone, core multicall still wants to know which tools to advertise
    if cfg.tools.objdump {
        println!("cargo:rustc-cfg=oxide_tool_objdump");
    }
    if cfg.tools.nm {
        println!("cargo:rustc-cfg=oxide_tool_nm");
    }
    if cfg.tools.readelf {
        println!("cargo:rustc-cfg=oxide_tool_readelf");
    }
    if cfg.tools.size {
        println!("cargo:rustc-cfg=oxide_tool_size");
    }
    if cfg.tools.strings {
        println!("cargo:rustc-cfg=oxide_tool_strings");
    }
    if cfg.tools.ar {
        println!("cargo:rustc-cfg=oxide_tool_ar");
    }
    if cfg.tools.strip {
        println!("cargo:rustc-cfg=oxide_tool_strip");
    }
    if cfg.tools.objcopy {
        println!("cargo:rustc-cfg=oxide_tool_objcopy");
    }
    if cfg.tools.addr2line {
        println!("cargo:rustc-cfg=oxide_tool_addr2line");
    }
    if cfg.tools.cxxfilt {
        println!("cargo:rustc-cfg=oxide_tool_cxxfilt");
    }
    if cfg.tools.elfedit {
        println!("cargo:rustc-cfg=oxide_tool_elfedit");
    }
}

fn write_build_plan(root: &Path, cfg: &BuildConfig, toml: Option<&Path>) {
    let plan = root.join("target").join("oxideutils-build-plan.txt");
    let _ = fs::create_dir_all(root.join("target"));
    let mut s = String::new();
    s.push_str("OxideUtils build plan (from oxideutils.toml)\n");
    s.push_str("============================================\n");
    if let Some(p) = toml {
        s.push_str(&format!("config: {}\n", p.display()));
    } else {
        s.push_str("config: (defaults — no oxideutils.toml found)\n");
    }
    s.push_str(&format!("standalone     = {}\n", cfg.standalone));
    s.push_str(&format!("static_link    = {}\n", cfg.static_link));
    s.push_str(&format!("dynamic_link   = {}\n", cfg.dynamic_link));
    s.push_str(&format!("kernel (flag)  = {}\n", cfg.kernel));
    s.push_str(&format!("features.disasm          = {}\n", cfg.disasm));
    s.push_str(&format!("features.disasm_aarch64  = {}\n", cfg.disasm_aarch64));
    s.push_str(&format!("features.dwarf           = {}\n", cfg.dwarf));
    s.push_str("tools:\n");
    s.push_str(&format!("  objdump   = {}\n", cfg.tools.objdump));
    s.push_str(&format!("  nm        = {}\n", cfg.tools.nm));
    s.push_str(&format!("  readelf   = {}\n", cfg.tools.readelf));
    s.push_str(&format!("  size      = {}\n", cfg.tools.size));
    s.push_str(&format!("  strings   = {}\n", cfg.tools.strings));
    s.push_str(&format!("  ar        = {}  (also gates oxide-ranlib)\n", cfg.tools.ar));
    s.push_str(&format!("  strip     = {}\n", cfg.tools.strip));
    s.push_str(&format!("  objcopy   = {}\n", cfg.tools.objcopy));
    s.push_str(&format!("  addr2line = {}\n", cfg.tools.addr2line));
    s.push_str(&format!("  cxxfilt   = {}\n", cfg.tools.cxxfilt));
    s.push_str(&format!("  elfedit   = {}\n", cfg.tools.elfedit));
    s.push_str(&format!("  multicall = {}\n", cfg.tools.multicall));
    if cfg.standalone {
        s.push_str("\nNOTE: standalone=true → separate oxide-* bins are disabled stubs.\n");
        s.push_str("      Use the single binary:  target/release/oxideutils <tool> …\n");
    }
    if cfg.kernel {
        s.push_str("\nNOTE: build.kernel=true → also run (separate target-dir):\n");
        s.push_str(
            "  cargo build -p oxideutils-core --release --no-default-features \\\n\
             \x20   --features alloc,disasm,kernel --target-dir target-nostd\n",
        );
    }
    s.push_str("\nBuild with:  cargo build --release\n");
    let _ = fs::write(&plan, s);
}
