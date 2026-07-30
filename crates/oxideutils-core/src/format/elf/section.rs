//! ELF section headers (readelf -S).

use crate::prelude::*;
use goblin::elf::Elf;

pub fn format_section_headers(elf: &Elf<'_>) -> String {
    let mut s = format!(
        "There are {} section headers, starting at offset 0x{:x}:\n\n",
        elf.section_headers.len(),
        elf.header.e_shoff
    );
    s.push_str("Section Headers:\n");
    s.push_str("  [Nr] Name              Type            Address          Off    Size   ES Flg Lk Inf Al\n");
    for (i, sh) in elf.section_headers.iter().enumerate() {
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("<corrupt>");
        s.push_str(&format!(
            "  [{i:2}] {:<17.17} {:<15} {:016x} {:06x} {:06x} {:02x} {:3} {:2} {:3} {:2}\n",
            name,
            sh_type(sh.sh_type),
            sh.sh_addr,
            sh.sh_offset,
            sh.sh_size,
            sh.sh_entsize,
            sh_flags(sh.sh_flags),
            sh.sh_link,
            sh.sh_info,
            sh.sh_addralign,
        ));
    }
    // Compressed section summary (SHF_COMPRESSED = 0x800)
    let mut compressed = Vec::new();
    for sh in &elf.section_headers {
        if sh.sh_flags & 0x800 != 0 {
            let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("?");
            compressed.push(format!("{name} (0x{:x} bytes on disk)", sh.sh_size));
        }
    }
    if !compressed.is_empty() {
        s.push_str("\nCompressed section(s):\n");
        for c in compressed {
            s.push_str(&format!("  {c}\n"));
        }
    }

    s.push_str("Key to Flags:\n");
    s.push_str("  W (write), A (alloc), X (execute), M (merge), S (strings), I (info),\n");
    s.push_str("  L (link order), O (extra OS processing required), G (group), T (TLS),\n");
    s.push_str("  C (compressed), x (unknown), o (OS specific), E (exclude),\n");
    s.push_str("  D (mbind), p (processor specific)\n");
    s
}

fn sh_type(t: u32) -> &'static str {
    match t {
        0 => "NULL",
        1 => "PROGBITS",
        2 => "SYMTAB",
        3 => "STRTAB",
        4 => "RELA",
        5 => "HASH",
        6 => "DYNAMIC",
        7 => "NOTE",
        8 => "NOBITS",
        9 => "REL",
        11 => "DYNSYM",
        14 => "INIT_ARRAY",
        15 => "FINI_ARRAY",
        17 => "GNU_HASH", // often vendor
        0x6ffffff6 => "GNU_HASH",
        0x6fffffff => "VERSYM",
        0x6ffffffe => "VERNEED",
        0x6ffffffd => "VERDEF",
        _ => "UNKNOWN",
    }
}

fn sh_flags(f: u64) -> String {
    let mut o = String::new();
    if f & 0x1 != 0 {
        o.push('W');
    }
    if f & 0x2 != 0 {
        o.push('A');
    }
    if f & 0x4 != 0 {
        o.push('X');
    }
    if f & 0x10 != 0 {
        o.push('M');
    }
    if f & 0x20 != 0 {
        o.push('S');
    }
    if f & 0x40 != 0 {
        o.push('I');
    }
    if f & 0x400 != 0 {
        o.push('T');
    }
    if f & 0x800 != 0 {
        o.push('C');
    }
    o
}
