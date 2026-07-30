//! oxide-elfedit — patch the ELF header (e_machine, e_type, OSABI,
//! ABI version) of one or more files in place, optionally gated by
//! `--input-*` filters. Mirrors GNU `elfedit`'s scope: direct fixed-offset
//! header edits, not a general ELF rewriter (that's `objcopy`'s job).
//!
//! `e_ident` (16 bytes), `e_type` (u16 @ offset 16) and `e_machine` (u16 @
//! offset 18) sit at the same byte offsets in both Elf32_Ehdr and
//! Elf64_Ehdr, so no 32/64-bit branch is needed for what this tool touches.

use clap::Parser;
use oxideutils_core::error::{OxideError, Result};
use oxideutils_core::utils::atomic_write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use oxideutils_core::cli::help::{self, VERSION, print_version};

#[derive(Debug, Parser)]
#[command(
    name = "oxide-elfedit",
    about = help::elfedit::ABOUT,
    long_about = help::elfedit::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    #[arg(long = "input-mach", value_name = "MACHINE")]
    input_mach: Option<String>,
    #[arg(long = "input-type", value_name = "TYPE")]
    input_type: Option<String>,
    #[arg(long = "input-osabi", value_name = "OSABI")]
    input_osabi: Option<String>,
    #[arg(long = "input-abiversion", value_name = "VERSION")]
    input_abiversion: Option<u8>,

    #[arg(long = "output-mach", value_name = "MACHINE")]
    output_mach: Option<String>,
    #[arg(long = "output-type", value_name = "TYPE")]
    output_type: Option<String>,
    #[arg(long = "output-osabi", value_name = "OSABI")]
    output_osabi: Option<String>,
    #[arg(long = "output-abiversion", value_name = "VERSION")]
    output_abiversion: Option<u8>,

    #[arg(short = 'V', long = "version")]
    version: bool,

    #[arg(value_name = "ELFFILE")]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("elfedit") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-elfedit");
        return ExitCode::SUCCESS;
    }

    let output_mach = args.output_mach.as_deref().map(parse_machine).transpose();
    let output_type = args.output_type.as_deref().map(parse_type).transpose();
    let output_osabi = args.output_osabi.as_deref().map(parse_osabi).transpose();
    let (output_mach, output_type, output_osabi) = match (output_mach, output_type, output_osabi) {
        (Ok(m), Ok(t), Ok(o)) => (m, t, o),
        (Err(e), _, _) | (_, Err(e), _) | (_, _, Err(e)) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    if output_mach.is_none()
        && output_type.is_none()
        && output_osabi.is_none()
        && args.output_abiversion.is_none()
    {
        eprintln!(
            "oxide-elfedit: at least one of --output-mach, --output-type, \
             --output-osabi or --output-abiversion is required"
        );
        return ExitCode::from(2);
    }

    if args.files.is_empty() {
        use clap::CommandFactory;
        let mut cmd = Args::command();
        let _ = cmd.print_help();
        println!();
        return ExitCode::from(2);
    }

    let input_mach = match args.input_mach.as_deref().map(parse_machine).transpose() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let input_type = match args.input_type.as_deref().map(parse_type).transpose() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    let input_osabi = match args.input_osabi.as_deref().map(parse_osabi).transpose() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };

    let edit = Edit {
        input_mach,
        input_type,
        input_osabi,
        input_abiversion: args.input_abiversion,
        output_mach,
        output_type,
        output_osabi,
        output_abiversion: args.output_abiversion,
    };

    let mut had_error = false;
    for file in &args.files {
        if let Err(e) = edit_file(file, &edit) {
            eprintln!("{e}");
            had_error = true;
        }
    }
    if had_error {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

struct Edit {
    input_mach: Option<u16>,
    input_type: Option<u16>,
    input_osabi: Option<u8>,
    input_abiversion: Option<u8>,
    output_mach: Option<u16>,
    output_type: Option<u16>,
    output_osabi: Option<u8>,
    output_abiversion: Option<u8>,
}

const EI_OSABI: usize = 7;
const EI_ABIVERSION: usize = 8;
const E_TYPE_OFFSET: usize = 16;
const E_MACHINE_OFFSET: usize = 18;
const EHDR_MIN_LEN: usize = 20;

fn edit_file(path: &Path, edit: &Edit) -> Result<()> {
    let mut data = oxideutils_core::utils::read_file(path)?;
    if data.len() < EHDR_MIN_LEN || &data[0..4] != b"\x7fELF" {
        return Err(OxideError::format(
            path.display().to_string(),
            "not an ELF file",
        ));
    }
    let little_endian = match data[5] {
        1 => true,
        2 => false,
        _ => {
            return Err(OxideError::format(
                path.display().to_string(),
                "invalid ELF data encoding (e_ident[EI_DATA])",
            ));
        }
    };

    let cur_type = read_u16(&data, E_TYPE_OFFSET, little_endian);
    let cur_mach = read_u16(&data, E_MACHINE_OFFSET, little_endian);
    let cur_osabi = data[EI_OSABI];
    let cur_abiversion = data[EI_ABIVERSION];

    if let Some(want) = edit.input_mach
        && want != cur_mach
    {
        return Ok(());
    }
    if let Some(want) = edit.input_type
        && want != cur_type
    {
        return Ok(());
    }
    if let Some(want) = edit.input_osabi
        && want != cur_osabi
    {
        return Ok(());
    }
    if let Some(want) = edit.input_abiversion
        && want != cur_abiversion
    {
        return Ok(());
    }

    let mut changed = false;
    if let Some(m) = edit.output_mach {
        write_u16(&mut data, E_MACHINE_OFFSET, m, little_endian);
        changed = true;
    }
    if let Some(t) = edit.output_type {
        write_u16(&mut data, E_TYPE_OFFSET, t, little_endian);
        changed = true;
    }
    if let Some(o) = edit.output_osabi {
        data[EI_OSABI] = o;
        changed = true;
    }
    if let Some(v) = edit.output_abiversion {
        data[EI_ABIVERSION] = v;
        changed = true;
    }

    if changed {
        atomic_write(path, &data, Some(path))?;
    }
    Ok(())
}

fn read_u16(data: &[u8], off: usize, little_endian: bool) -> u16 {
    let b = [data[off], data[off + 1]];
    if little_endian {
        u16::from_le_bytes(b)
    } else {
        u16::from_be_bytes(b)
    }
}

fn write_u16(data: &mut [u8], off: usize, val: u16, little_endian: bool) {
    let b = if little_endian {
        val.to_le_bytes()
    } else {
        val.to_be_bytes()
    };
    data[off] = b[0];
    data[off + 1] = b[1];
}

/// Machine names elfedit(1) documents: i386, IAMCU, L1OM, K1OM, x86-64.
/// Also accepts a raw decimal ELF machine number for anything else.
fn parse_machine(s: &str) -> Result<u16> {
    match s.to_ascii_lowercase().as_str() {
        "i386" => Ok(3),
        "iamcu" => Ok(6),
        "x86-64" | "x86_64" => Ok(62),
        "l1om" => Ok(180),
        "k1om" => Ok(181),
        other => other
            .parse()
            .map_err(|_| OxideError::InvalidArgument(format!("unknown --*-mach '{s}'"))),
    }
}

/// File types elfedit(1) documents: rel, exec, dyn. `core`/`none` accepted
/// as a bonus (real ELF types the format supports, just not in the man page).
fn parse_type(s: &str) -> Result<u16> {
    match s.to_ascii_lowercase().as_str() {
        "none" => Ok(0),
        "rel" => Ok(1),
        "exec" => Ok(2),
        "dyn" => Ok(3),
        "core" => Ok(4),
        other => other
            .parse()
            .map_err(|_| OxideError::InvalidArgument(format!("unknown --*-type '{s}'"))),
    }
}

/// OSABI names from elfedit(1) (ELFOSABI_* in binutils include/elf/common.h).
fn parse_osabi(s: &str) -> Result<u8> {
    match s.to_ascii_lowercase().as_str() {
        "none" => Ok(0),
        "hpux" => Ok(1),
        "netbsd" => Ok(2),
        "gnu" | "linux" => Ok(3),
        "solaris" => Ok(6),
        "aix" => Ok(7),
        "irix" => Ok(8),
        "freebsd" => Ok(9),
        "tru64" => Ok(10),
        "modesto" => Ok(11),
        "openbsd" => Ok(12),
        "openvms" => Ok(13),
        "nsk" => Ok(14),
        "aros" => Ok(15),
        "fenixos" => Ok(16),
        other => other
            .parse()
            .map_err(|_| OxideError::InvalidArgument(format!("unknown --*-osabi '{s}'"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn minimal_elf() -> Vec<u8> {
        // 64-byte ELF64 header shell: magic, class=64, data=LE, version=1,
        // osabi=0, abiversion=0, pad, type=EXEC(2), machine=x86-64(62)...
        let mut h = vec![0u8; 64];
        h[0..4].copy_from_slice(b"\x7fELF");
        h[4] = 2; // ELFCLASS64
        h[5] = 1; // little endian
        h[6] = 1; // EI_VERSION
        write_u16(&mut h, E_TYPE_OFFSET, 2, true); // ET_EXEC
        write_u16(&mut h, E_MACHINE_OFFSET, 62, true); // EM_X86_64
        h
    }

    #[test]
    fn rewrites_osabi_and_type() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.elf");
        fs::write(&path, minimal_elf()).unwrap();

        let edit = Edit {
            input_mach: None,
            input_type: None,
            input_osabi: None,
            input_abiversion: None,
            output_mach: None,
            output_type: Some(3),  // ET_DYN
            output_osabi: Some(3), // ELFOSABI_GNU
            output_abiversion: None,
        };
        edit_file(&path, &edit).unwrap();

        let data = fs::read(&path).unwrap();
        assert_eq!(data[EI_OSABI], 3);
        assert_eq!(read_u16(&data, E_TYPE_OFFSET, true), 3);
        // untouched field
        assert_eq!(read_u16(&data, E_MACHINE_OFFSET, true), 62);
    }

    #[test]
    fn input_filter_skips_non_matching_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.elf");
        fs::write(&path, minimal_elf()).unwrap();

        let edit = Edit {
            input_mach: Some(3), // i386 — file is x86-64, shouldn't match
            input_type: None,
            input_osabi: None,
            input_abiversion: None,
            output_mach: None,
            output_type: Some(3),
            output_osabi: None,
            output_abiversion: None,
        };
        edit_file(&path, &edit).unwrap();

        let data = fs::read(&path).unwrap();
        // type must be unchanged since the input-mach filter didn't match
        assert_eq!(read_u16(&data, E_TYPE_OFFSET, true), 2);
    }

    #[test]
    fn rejects_non_elf() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.bin");
        fs::write(&path, b"not an elf file at all").unwrap();

        let edit = Edit {
            input_mach: None,
            input_type: None,
            input_osabi: None,
            input_abiversion: None,
            output_mach: None,
            output_type: Some(2),
            output_osabi: None,
            output_abiversion: None,
        };
        assert!(edit_file(&path, &edit).is_err());
    }
}
