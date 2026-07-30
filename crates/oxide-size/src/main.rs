//! oxide-size — list section sizes (GNU size compatible).

use clap::Parser;
use object::{Object, ObjectSection, SectionKind};
use oxideutils_core::cli::help::{self, VERSION, print_version};
use oxideutils_core::cli::utils::Status;
use oxideutils_core::error::Result;
use oxideutils_core::format::object::OxideObject;
use oxideutils_core::utils::{map_file, read_file};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Debug, Parser)]
#[command(
    name = "oxide-size",
    about = help::size::ABOUT,
    long_about = help::size::LONG_ABOUT,
    version = VERSION,
    disable_version_flag = true,
    styles = help::clap_styles(),
)]
struct Args {
    /// Output format: berkeley (default) or sysv
    #[arg(
        short = 'A',
        long = "format",
        default_value = "berkeley",
        value_name = "FORMAT"
    )]
    format: String,

    /// Print totals for Berkeley format
    #[arg(short = 't', long = "totals")]
    totals: bool,

    /// Version banner
    #[arg(short = 'V', long = "version")]
    version: bool,

    /// Files to measure (default: a.out)
    #[arg(value_name = "FILE")]
    files: Vec<PathBuf>,
}

fn main() -> ExitCode {
    if oxideutils_core::exit_if_tool_disabled!("size") {
        return ExitCode::from(2);
    }
    let args = Args::parse();
    if args.version {
        print_version("oxide-size");
        return ExitCode::SUCCESS;
    }
    let files = if args.files.is_empty() {
        vec![PathBuf::from("a.out")]
    } else {
        args.files.clone()
    };

    let berkeley = !matches!(args.format.as_str(), "sysv" | "SysV" | "s");
    if berkeley {
        println!(
            "{:>10} {:>10} {:>10} {:>10} {:>10} filename",
            "text", "data", "bss", "dec", "hex"
        );
    }

    let mut status = Status::ok();
    let mut tot_text = 0u64;
    let mut tot_data = 0u64;
    let mut tot_bss = 0u64;

    for f in &files {
        match process(f, berkeley) {
            Ok((t, d, b)) => {
                tot_text += t;
                tot_data += d;
                tot_bss += b;
            }
            Err(e) => status.record(Err(e)),
        }
    }
    if args.totals && berkeley {
        let dec = tot_text + tot_data + tot_bss;
        println!("{tot_text:>10} {tot_data:>10} {tot_bss:>10} {dec:>10} {dec:>10x} (TOTALS)");
    }
    status.exit_code()
}

fn process(path: &Path, berkeley: bool) -> Result<(u64, u64, u64)> {
    let data = map_file(path)
        .map(|m| m.to_vec())
        .or_else(|_| read_file(path))?;
    let obj = OxideObject::parse(path.display(), &data)?;

    let mut text = 0u64;
    let mut data_sz = 0u64;
    let mut bss = 0u64;

    if berkeley {
        for sec in obj.file.sections() {
            let size = sec.size();
            match sec.kind() {
                SectionKind::Text => text += size,
                SectionKind::Data
                | SectionKind::ReadOnlyData
                | SectionKind::ReadOnlyString
                | SectionKind::ReadOnlyDataWithRel => data_sz += size,
                SectionKind::UninitializedData | SectionKind::Common => bss += size,
                _ => {}
            }
        }
        let dec = text + data_sz + bss;
        println!(
            "{text:>10} {data_sz:>10} {bss:>10} {dec:>10} {dec:>10x} {}",
            path.display()
        );
    } else {
        println!("{}  :", path.display());
        println!("{:>10} {:>10} {:>10}", "section", "size", "addr");
        for sec in obj.file.sections() {
            let name = sec.name().unwrap_or("?");
            if name.is_empty() {
                continue;
            }
            println!("{name:>10} {:>10} {:#010x}", sec.size(), sec.address());
            match sec.kind() {
                SectionKind::Text => text += sec.size(),
                SectionKind::Data
                | SectionKind::ReadOnlyData
                | SectionKind::ReadOnlyString
                | SectionKind::ReadOnlyDataWithRel => data_sz += sec.size(),
                SectionKind::UninitializedData | SectionKind::Common => bss += sec.size(),
                _ => {}
            }
        }
        let total = text + data_sz + bss;
        println!("{:>10} {total:>10}", "Total");
    }
    Ok((text, data_sz, bss))
}
