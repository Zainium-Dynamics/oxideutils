//! ELF file header formatting (readelf -h).

use crate::prelude::*;
use crate::format::utils::elf_machine_name;
use goblin::elf::Elf;

pub fn format_elf_header(elf: &Elf<'_>) -> String {
    let h = &elf.header;
    let mut s = String::new();
    s.push_str("ELF Header:\n");
    s.push_str(&format!(
        "  Magic:   {}\n",
        h.e_ident
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    ));
    s.push_str(&format!(
        "  Class:                             {}\n",
        if elf.is_64 { "ELF64" } else { "ELF32" }
    ));
    s.push_str(&format!(
        "  Data:                              {}\n",
        if elf.little_endian {
            "2's complement, little endian"
        } else {
            "2's complement, big endian"
        }
    ));
    s.push_str(&format!(
        "  Version:                           {} (current)\n",
        h.e_ident[6]
    ));
    s.push_str(&format!(
        "  OS/ABI:                            {}\n",
        osabi(h.e_ident[7])
    ));
    s.push_str(&format!(
        "  ABI Version:                       {}\n",
        h.e_ident[8]
    ));
    s.push_str(&format!(
        "  Type:                              {}\n",
        match h.e_type {
            0 => "NONE (None)".into(),
            1 => "REL (Relocatable file)".into(),
            2 => "EXEC (Executable file)".into(),
            3 => "DYN (Shared object file)".into(),
            4 => "CORE (Core file)".into(),
            t => format!("{t}"),
        }
    ));
    s.push_str(&format!(
        "  Machine:                           {}\n",
        elf_machine_name(h.e_machine)
    ));
    s.push_str(&format!(
        "  Version:                           0x{:x}\n",
        h.e_version
    ));
    s.push_str(&format!(
        "  Entry point address:               0x{:x}\n",
        elf.entry
    ));
    s.push_str(&format!(
        "  Start of program headers:          {} (bytes into file)\n",
        h.e_phoff
    ));
    s.push_str(&format!(
        "  Start of section headers:          {} (bytes into file)\n",
        h.e_shoff
    ));
    s.push_str(&format!(
        "  Flags:                             0x{:x}\n",
        h.e_flags
    ));
    s.push_str(&format!(
        "  Size of this header:               {} (bytes)\n",
        h.e_ehsize
    ));
    s.push_str(&format!(
        "  Size of program headers:           {} (bytes)\n",
        h.e_phentsize
    ));
    s.push_str(&format!(
        "  Number of program headers:         {}\n",
        h.e_phnum
    ));
    s.push_str(&format!(
        "  Size of section headers:           {} (bytes)\n",
        h.e_shentsize
    ));
    s.push_str(&format!(
        "  Number of section headers:         {}\n",
        h.e_shnum
    ));
    s.push_str(&format!(
        "  Section header string table index: {}\n",
        h.e_shstrndx
    ));
    s
}

fn osabi(v: u8) -> &'static str {
    match v {
        0 => "UNIX - System V",
        1 => "HP-UX",
        2 => "NetBSD",
        3 => "Linux",
        6 => "Solaris",
        7 => "AIX",
        8 => "IRIX",
        9 => "FreeBSD",
        12 => "OpenBSD",
        _ => "Unknown",
    }
}
