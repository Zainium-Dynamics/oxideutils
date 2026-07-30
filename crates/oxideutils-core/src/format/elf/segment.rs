//! ELF program headers (readelf -l).

use crate::prelude::*;
use goblin::elf::Elf;

pub fn format_program_headers(elf: &Elf<'_>) -> String {
    let mut s = format!(
        "\nElf file type is {}\nEntry point 0x{:x}\nThere are {} program headers, starting at offset {}\n\n",
        match elf.header.e_type {
            2 => "EXEC (Executable file)",
            3 => "DYN (Shared object file)",
            1 => "REL (Relocatable file)",
            4 => "CORE (Core file)",
            _ => "UNKNOWN",
        },
        elf.entry,
        elf.program_headers.len(),
        elf.header.e_phoff
    );
    s.push_str("Program Headers:\n");
    s.push_str("  Type           Offset             VirtAddr           PhysAddr\n");
    s.push_str("                 FileSiz            MemSiz              Flags  Align\n");
    for ph in &elf.program_headers {
        s.push_str(&format!(
            "  {:<14} 0x{:016x} 0x{:016x} 0x{:016x}\n",
            ph_type(ph.p_type),
            ph.p_offset,
            ph.p_vaddr,
            ph.p_paddr
        ));
        s.push_str(&format!(
            "                 0x{:016x} 0x{:016x}  {}    0x{:x}\n",
            ph.p_filesz,
            ph.p_memsz,
            ph_flags(ph.p_flags),
            ph.p_align
        ));
    }
    s
}

fn ph_type(t: u32) -> &'static str {
    match t {
        0 => "NULL",
        1 => "LOAD",
        2 => "DYNAMIC",
        3 => "INTERP",
        4 => "NOTE",
        5 => "SHLIB",
        6 => "PHDR",
        7 => "TLS",
        0x6474e550 => "GNU_EH_FRAME",
        0x6474e551 => "GNU_STACK",
        0x6474e552 => "GNU_RELRO",
        0x6474e553 => "GNU_PROPERTY",
        _ => "UNKNOWN",
    }
}

fn ph_flags(f: u32) -> String {
    let mut o = String::new();
    o.push(if f & 4 != 0 { 'R' } else { ' ' });
    o.push(if f & 2 != 0 { 'W' } else { ' ' });
    o.push(if f & 1 != 0 { 'E' } else { ' ' });
    o
}
