//! oxide-ld — GNU ld subset (binutils 2.46.1 option + link semantics).

mod archive;
mod dynamic;
mod elfout;
mod libsearch;
mod linker;
mod objload;
mod reloc;
mod script;

use anyhow::{Context, Result};
use linker::{link_elf_executable, LinkArg, LinkerConfig};
use std::env;
use std::fs;
use std::path::PathBuf;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let mut output_path = PathBuf::from("a.out");
    let mut link_args: Vec<LinkArg> = Vec::new();
    let mut search_dirs: Vec<PathBuf> = Vec::new();
    let mut sysroot: Option<PathBuf> = env::var_os("OXIDE_LD_SYSROOT").map(PathBuf::from);
    let mut dynamic_linker: Option<String> = env::var("OXIDE_LD_DYNAMIC_LINKER").ok();
    let mut is_shared = false;
    let mut is_pie = false;
    let mut no_interp = false;
    let mut static_only = false;
    let mut verbose = false;
    let mut entry = String::from("_start");
    let mut soname: Option<String> = None;
    let mut script: Option<String> = None;

    let mut idx = 1;
    while idx < args.len() {
        let arg = &args[idx];
        match arg.as_str() {
            "-o" => {
                if idx + 1 < args.len() {
                    output_path = PathBuf::from(&args[idx + 1]);
                    idx += 1;
                }
            }
            "-L" => {
                if idx + 1 < args.len() {
                    search_dirs.push(PathBuf::from(&args[idx + 1]));
                    idx += 1;
                }
            }
            "-l" => {
                if idx + 1 < args.len() {
                    link_args.push(LinkArg::Lib(args[idx + 1].clone()));
                    idx += 1;
                }
            }
            "-dynamic-linker" | "--dynamic-linker" | "-I" => {
                if idx + 1 < args.len() {
                    dynamic_linker = Some(args[idx + 1].clone());
                    no_interp = false;
                    idx += 1;
                }
            }
            "--sysroot" => {
                if idx + 1 < args.len() {
                    sysroot = Some(PathBuf::from(&args[idx + 1]));
                    idx += 1;
                }
            }
            "-no-dynamic-linker" | "--no-dynamic-linker" => no_interp = true,
            "-shared" | "--shared" | "-Bshareable" => {
                is_shared = true;
                is_pie = false;
            }
            "-pie" | "--pie" | "--pic-executable" => {
                is_pie = true;
                is_shared = false;
            }
            "-no-pie" | "--no-pie" => is_pie = false,
            "-static" | "--static" | "-Bstatic" => static_only = true,
            "-e" | "--entry" => {
                if idx + 1 < args.len() {
                    entry = args[idx + 1].clone();
                    idx += 1;
                }
            }
            "-soname" | "--soname" | "-h" => {
                if idx + 1 < args.len() {
                    soname = Some(args[idx + 1].clone());
                    idx += 1;
                }
            }
            "-T" | "--script" => {
                if idx + 1 < args.len() {
                    let p = &args[idx + 1];
                    script = Some(
                        fs::read_to_string(p)
                            .with_context(|| format!("cannot read linker script {p}"))?,
                    );
                    idx += 1;
                }
            }
            "-v" | "--verbose" | "-V" => verbose = true,
            "--help" => {
                print_usage();
                return Ok(());
            }
            _ => {
                if let Some(dir) = arg.strip_prefix("-L") {
                    if !dir.is_empty() {
                        search_dirs.push(PathBuf::from(dir));
                    }
                } else if let Some(name) = arg.strip_prefix("-l") {
                    if !name.is_empty() {
                        link_args.push(LinkArg::Lib(name.to_string()));
                    }
                } else if let Some(path) = arg.strip_prefix("-T") {
                    if !path.is_empty() {
                        script = Some(
                            fs::read_to_string(path)
                                .with_context(|| format!("cannot read linker script {path}"))?,
                        );
                    }
                } else if let Some(name) = arg.strip_prefix("-soname=") {
                    soname = Some(name.to_string());
                } else if let Some(dir) = arg.strip_prefix("--sysroot=") {
                    sysroot = Some(PathBuf::from(dir));
                } else if arg.starts_with("-m")
                    || arg.starts_with("--hash-style")
                    || arg.starts_with("-z")
                    || arg == "-g"
                    || arg == "-s"
                    || arg == "-S"
                    || arg == "--gc-sections"
                    || arg == "-rpath"
                    || arg.starts_with("-rpath=")
                    || arg == "--as-needed"
                    || arg == "--no-as-needed"
                {
                    // accept common gcc-passed flags; skip their args when needed
                    if matches!(arg.as_str(), "-rpath" | "-z") && idx + 1 < args.len() {
                        idx += 1;
                    }
                } else if !arg.starts_with('-') {
                    link_args.push(LinkArg::File(PathBuf::from(arg)));
                }
            }
        }
        idx += 1;
    }

    // Precedence: explicit `-dynamic-linker`/`OXIDE_LD_DYNAMIC_LINKER` >
    // sysroot-relative musl convention (`{sysroot}/lib/ld-musl-x86_64.so.1`,
    // matching a musl target's actual on-disk layout, not glibc's
    // `/lib64/ld-linux-x86-64.so.2`) > a last-resort default so zero-config
    // invocations keep working. No FHS assumption is baked in beyond this
    // single fallback string, and it's overridden by either mechanism above.
    let dynamic_linker = dynamic_linker.unwrap_or_else(|| {
        sysroot
            .as_ref()
            .map(|root| {
                root.join("lib/ld-musl-x86_64.so.1")
                    .to_string_lossy()
                    .into_owned()
            })
            .unwrap_or_else(|| {
                "/overlayer/syshub/x86_64-zainium-linux-musl/lib/ld-musl-x86_64.so.1".to_string()
            })
    });

    if verbose {
        println!("[*] oxide-ld — multi-section + relocs + archive/-l + dynamic-linking subset");
        println!("[*] shared={is_shared} pie={is_pie} static={static_only} no_interp={no_interp}");
        println!("[*] interp={dynamic_linker}");
        println!("[*] entry={entry}");
        println!("[*] sysroot={sysroot:?} search_dirs={search_dirs:?}");
        println!("[*] output={}", output_path.display());
    }

    link_elf_executable(&LinkerConfig {
        output_path,
        link_args,
        search_dirs,
        sysroot,
        dynamic_linker,
        is_shared,
        is_pie,
        no_interp,
        static_only,
        verbose,
        entry,
        soname,
        script,
    })?;
    Ok(())
}

fn print_usage() {
    eprintln!(
        "\
Usage: oxide-ld [options] file...
  -o FILE                   Output file
  -e SYM, --entry SYM       Entry symbol (default _start)
  -I PROG, --dynamic-linker PROG
  --no-dynamic-linker       Omit PT_INTERP
  -shared / -pie            ET_DYN shared or PIE
  -static                   Archives only; error if a symbol needs a shared lib
  -lNAME, -LDIR             Library search (real .a/.so resolution + GROUP() scripts)
  --sysroot DIR             Also search {{DIR}}/lib and {{DIR}}/usr/lib (or $OXIDE_LD_SYSROOT)
  -soname NAME              DT_SONAME for -shared output
  -T SCRIPT, --script FILE  GNU ld script subset (ENTRY, SECTIONS)
  -v                        Verbose

Implements GNU ld 2.46.1 subset: section merge, symbol resolve, archive/-l
resolution, eager-bound (DF_BIND_NOW) dynamic linking via PLT/GOT,
R_X86_64_{{64,PC32,PLT32,32,32S,RELATIVE,JUMP_SLOT}} relocs, default ELF layout."
    );
}
