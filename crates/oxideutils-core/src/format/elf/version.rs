//! ELF symbol versioning (GNU readelf `-V` / `--version-info`).
//!
//! Covers `.gnu.version` (versym), `.gnu.version_r` (verneed), `.gnu.version_d` (verdef).

use crate::prelude::*;
use goblin::elf::Elf;

/// Format GNU symbol version sections (readelf -V style summary).
pub fn format_version_info(elf: &Elf<'_>) -> String {
    let mut s = String::new();
    let mut any = false;

    if let Some(verneed) = &elf.verneed {
        any = true;
        s.push_str("\nVersion needs section '.gnu.version_r' contains version needs:\n");
        s.push_str(" Addr: offset 0  (displaying structure)\n");
        for need in verneed.iter() {
            let file = elf
                .dynstrtab
                .get_at(need.vn_file)
                .unwrap_or("<unknown>");
            s.push_str(&format!(
                "  0x{:04x}: Version: {}  File: {}  Cnt: {}\n",
                need.vn_version, need.vn_version, file, need.vn_cnt
            ));
            for aux in need.iter() {
                let name = elf.dynstrtab.get_at(aux.vna_name).unwrap_or("?");
                let flags = if aux.vna_flags != 0 {
                    format!("  flags: 0x{:x}", aux.vna_flags)
                } else {
                    String::new()
                };
                s.push_str(&format!(
                    "  0x{:04x}:   Name: {}  Flags: {}{}  Version: {}\n",
                    aux.vna_other, name, aux.vna_flags, flags, aux.vna_other
                ));
            }
        }
    }

    if let Some(verdef) = &elf.verdef {
        any = true;
        s.push_str("\nVersion definition section '.gnu.version_d' contains version definitions:\n");
        for def in verdef.iter() {
            let mut names = Vec::new();
            for aux in def.iter() {
                let name = elf.dynstrtab.get_at(aux.vda_name).unwrap_or("?");
                names.push(name.to_string());
            }
            let primary = names.first().map(|x| x.as_str()).unwrap_or("?");
            s.push_str(&format!(
                "  0x{:04x}: Rev: {}  Flags: {:#x}  Index: {}  Cnt: {}  Name: {}\n",
                def.vd_ndx, def.vd_version, def.vd_flags, def.vd_ndx, def.vd_cnt, primary
            ));
            for (i, n) in names.iter().enumerate().skip(1) {
                s.push_str(&format!("  0x{:04x}: Parent {}: {}\n", def.vd_ndx, i, n));
            }
        }
    }

    if let Some(versym) = &elf.versym {
        any = true;
        let n = versym.len();
        s.push_str(&format!(
            "\nVersion symbols section '.gnu.version' contains {n} entries:\n"
        ));
        s.push_str(" Addr: (relative to section)  Offset: 000000\n");
        // Compact dump: 4 per line like readelf often does
        let mut line = String::new();
        for (i, vs) in versym.iter().enumerate() {
            if i % 4 == 0 {
                if !line.is_empty() {
                    s.push_str(&line);
                    s.push('\n');
                }
                line = format!("  {i:3}:");
            }
            let hidden = if vs.is_hidden() { "h" } else { " " };
            let label = if vs.is_local() {
                "*local*".to_string()
            } else if vs.is_global() {
                "*global*".to_string()
            } else {
                format!("{}", vs.version())
            };
            // Pair with dynsym name when possible
            let sym_name = elf
                .dynsyms
                .get(i)
                .and_then(|sym| elf.dynstrtab.get_at(sym.st_name))
                .unwrap_or("");
            if sym_name.is_empty() {
                line.push_str(&format!(" {hidden}{label:<10}"));
            } else {
                line.push_str(&format!(" {hidden}{label}({sym_name})"));
            }
        }
        if !line.is_empty() {
            s.push_str(&line);
            s.push('\n');
        }
    }

    if !any {
        s.push_str("\nNo version information found in this file.\n");
    }
    s
}
