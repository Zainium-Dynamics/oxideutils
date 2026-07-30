//! ELF dynamic section (readelf -d).

use crate::prelude::*;
use goblin::elf::Elf;

pub fn format_dynamic(elf: &Elf<'_>) -> String {
    let mut s = String::new();
    let Some(dyn_) = &elf.dynamic else {
        return "\nThere is no dynamic section in this file.\n".into();
    };
    s.push_str(&format!(
        "\nDynamic section contains {} entries:\n",
        dyn_.dyns.len()
    ));
    s.push_str("  Tag        Type                         Name/Value\n");
    for d in &dyn_.dyns {
        let tag = d.d_tag as i64;
        let val = format_dyn_value(elf, tag, d.d_val);
        s.push_str(&format!(
            " 0x{:08x} ({:<12}) {}\n",
            d.d_tag,
            tag_name(tag),
            val
        ));
    }
    s
}

fn format_dyn_value(elf: &Elf<'_>, tag: i64, val: u64) -> String {
    match tag {
        1 => {
            // DT_NEEDED
            let name = elf.dynstrtab.get_at(val as usize).unwrap_or("");
            if name.is_empty() {
                format!("0x{val:x}")
            } else {
                format!("Shared library: [{name}]")
            }
        }
        14 => {
            // DT_SONAME
            let name = elf.dynstrtab.get_at(val as usize).unwrap_or("");
            format!("Library soname: [{name}]")
        }
        15 | 29 => {
            // DT_RPATH / DT_RUNPATH
            let name = elf.dynstrtab.get_at(val as usize).unwrap_or("");
            format!("Library path: [{name}]")
        }
        _ => format!("0x{val:x}"),
    }
}

fn tag_name(t: i64) -> &'static str {
    match t {
        0 => "NULL",
        1 => "NEEDED",
        2 => "PLTRELSZ",
        3 => "PLTGOT",
        4 => "HASH",
        5 => "STRTAB",
        6 => "SYMTAB",
        7 => "RELA",
        8 => "RELASZ",
        9 => "RELAENT",
        10 => "STRSZ",
        11 => "SYMENT",
        12 => "INIT",
        13 => "FINI",
        14 => "SONAME",
        15 => "RPATH",
        16 => "SYMBOLIC",
        17 => "REL",
        18 => "RELSZ",
        19 => "RELENT",
        20 => "PLTREL",
        21 => "DEBUG",
        22 => "TEXTREL",
        23 => "JMPREL",
        24 => "BIND_NOW",
        25 => "INIT_ARRAY",
        26 => "FINI_ARRAY",
        0x6ffffef5 => "GNU_HASH",
        0x6ffffffb => "FLAGS_1",
        0x6ffffff0 => "VERSYM",
        0x6ffffffe => "VERNEED",
        0x6fffffff => "VERNEEDNUM",
        _ => "UNKNOWN",
    }
}
