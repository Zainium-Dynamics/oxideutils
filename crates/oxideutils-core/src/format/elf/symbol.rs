//! ELF symbol tables (readelf -s).

use crate::prelude::*;
use goblin::elf::Elf;

pub fn format_symtab(elf: &Elf<'_>) -> String {
    let mut s = String::new();
    if !elf.syms.is_empty() {
        s.push_str(&format!(
            "\nSymbol table '.symtab' contains {} entries:\n",
            elf.syms.len()
        ));
        s.push_str("   Num:    Value          Size Type    Bind   Vis      Ndx Name\n");
        for (i, sym) in elf.syms.iter().enumerate() {
            let name = elf.strtab.get_at(sym.st_name).unwrap_or("");
            s.push_str(&format!(
                "  {i:4}: {:016x} {:5} {:<7} {:<6} {:<7} {:>3} {}\n",
                sym.st_value,
                sym.st_size,
                st_type(sym.st_info),
                st_bind(sym.st_info),
                "DEFAULT",
                ndx(sym.st_shndx),
                name
            ));
        }
    }
    if !elf.dynsyms.is_empty() {
        s.push_str(&format!(
            "\nSymbol table '.dynsym' contains {} entries:\n",
            elf.dynsyms.len()
        ));
        s.push_str("   Num:    Value          Size Type    Bind   Vis      Ndx Name\n");
        for (i, sym) in elf.dynsyms.iter().enumerate() {
            let name = elf.dynstrtab.get_at(sym.st_name).unwrap_or("");
            s.push_str(&format!(
                "  {i:4}: {:016x} {:5} {:<7} {:<6} {:<7} {:>3} {}\n",
                sym.st_value,
                sym.st_size,
                st_type(sym.st_info),
                st_bind(sym.st_info),
                "DEFAULT",
                ndx(sym.st_shndx),
                name
            ));
        }
    }
    if s.is_empty() {
        s.push_str("\nNo symbols.\n");
    }
    s
}

fn st_type(info: u8) -> &'static str {
    match info & 0xf {
        0 => "NOTYPE",
        1 => "OBJECT",
        2 => "FUNC",
        3 => "SECTION",
        4 => "FILE",
        5 => "COMMON",
        6 => "TLS",
        _ => "UNKNOWN",
    }
}

fn st_bind(info: u8) -> &'static str {
    match info >> 4 {
        0 => "LOCAL",
        1 => "GLOBAL",
        2 => "WEAK",
        _ => "UNKNOWN",
    }
}

fn ndx(i: usize) -> String {
    match i {
        0 => "UND".into(),
        0xfff1 => "ABS".into(),
        0xfff2 => "COM".into(),
        n => n.to_string(),
    }
}
