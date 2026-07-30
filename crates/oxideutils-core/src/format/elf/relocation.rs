//! ELF relocations (readelf -r / objdump -r style).

use crate::prelude::*;
use goblin::elf::Elf;

/// Pretty-print dynamic / PLT relocations with symbol names when available.
pub fn format_relocs(elf: &Elf<'_>) -> String {
    let mut s = String::new();
    let mut any = false;

    if !elf.dynrelas.is_empty() {
        any = true;
        s.push_str(&format!(
            "\nRelocation section '.rela.dyn' at offset 0x0 contains {} entries:\n",
            elf.dynrelas.len()
        ));
        s.push_str("  Offset          Info           Type           Sym. Value    Sym. Name + Addend\n");
        for rela in &elf.dynrelas {
            let (sym_name, sym_val) = resolve_sym(elf, rela.r_sym);
            let addend = rela.r_addend.unwrap_or(0);
            s.push_str(&format!(
                "{:016x}  {:016x} {:14} {:016x} {} + {}\n",
                rela.r_offset,
                info_word(elf.is_64, rela.r_sym, rela.r_type),
                reloc_type_name(elf.header.e_machine, rela.r_type),
                sym_val,
                sym_name,
                addend
            ));
        }
    }

    if !elf.dynrels.is_empty() {
        any = true;
        s.push_str(&format!(
            "\nRelocation section '.rel.dyn' contains {} entries:\n",
            elf.dynrels.len()
        ));
        s.push_str("  Offset          Info           Type           Sym. Value    Sym. Name\n");
        for rel in &elf.dynrels {
            let (sym_name, sym_val) = resolve_sym(elf, rel.r_sym);
            s.push_str(&format!(
                "{:016x}  {:016x} {:14} {:016x} {}\n",
                rel.r_offset,
                info_word(elf.is_64, rel.r_sym, rel.r_type),
                reloc_type_name(elf.header.e_machine, rel.r_type),
                sym_val,
                sym_name
            ));
        }
    }

    if !elf.pltrelocs.is_empty() {
        any = true;
        s.push_str(&format!(
            "\nRelocation section '.rela.plt' contains {} entries:\n",
            elf.pltrelocs.len()
        ));
        s.push_str("  Offset          Info           Type           Sym. Value    Sym. Name + Addend\n");
        for rela in &elf.pltrelocs {
            let (sym_name, sym_val) = resolve_sym(elf, rela.r_sym);
            let addend = rela.r_addend.unwrap_or(0);
            s.push_str(&format!(
                "{:016x}  {:016x} {:14} {:016x} {} + {}\n",
                rela.r_offset,
                info_word(elf.is_64, rela.r_sym, rela.r_type),
                reloc_type_name(elf.header.e_machine, rela.r_type),
                sym_val,
                sym_name,
                addend
            ));
        }
    }

    if !any {
        s.push_str("\nThere are no relocations in this file.\n");
    }
    s
}

fn resolve_sym(elf: &Elf<'_>, idx: usize) -> (String, u64) {
    if idx == 0 {
        return (String::new(), 0);
    }
    if let Some(sym) = elf.dynsyms.get(idx) {
        let name = elf.dynstrtab.get_at(sym.st_name).unwrap_or("").to_string();
        return (name, sym.st_value);
    }
    if let Some(sym) = elf.syms.get(idx) {
        let name = elf.strtab.get_at(sym.st_name).unwrap_or("").to_string();
        return (name, sym.st_value);
    }
    (format!("#{idx}"), 0)
}

fn info_word(is_64: bool, sym: usize, rtype: u32) -> u64 {
    if is_64 {
        ((sym as u64) << 32) | (rtype as u64)
    } else {
        ((sym as u64) << 8) | (rtype as u64 & 0xff)
    }
}

/// Common x86_64 / generic reloc type names (partial).
fn reloc_type_name(machine: u16, rtype: u32) -> String {
    // EM_X86_64 = 62
    if machine == 62 {
        let name = match rtype {
            0 => "R_X86_64_NONE",
            1 => "R_X86_64_64",
            2 => "R_X86_64_PC32",
            5 => "R_X86_64_COPY",
            6 => "R_X86_64_GLOB_DAT",
            7 => "R_X86_64_JUMP_SLOT",
            8 => "R_X86_64_RELATIVE",
            9 => "R_X86_64_GOTPCREL",
            10 => "R_X86_64_32",
            11 => "R_X86_64_32S",
            16 => "R_X86_64_DTPMOD64",
            17 => "R_X86_64_DTPOFF64",
            18 => "R_X86_64_TPOFF64",
            24 => "R_X86_64_PC64",
            37 => "R_X86_64_IRELATIVE",
            42 => "R_X86_64_REX_GOTPCRELX",
            _ => "",
        };
        if !name.is_empty() {
            return name.into();
        }
    }
    // EM_AARCH64 = 183
    if machine == 183 {
        let name = match rtype {
            0 => "R_AARCH64_NONE",
            257 => "R_AARCH64_ABS64",
            1025 => "R_AARCH64_COPY",
            1026 => "R_AARCH64_GLOB_DAT",
            1027 => "R_AARCH64_JUMP_SLOT",
            1028 => "R_AARCH64_RELATIVE",
            1032 => "R_AARCH64_IRELATIVE",
            _ => "",
        };
        if !name.is_empty() {
            return name.into();
        }
    }
    format!("R_TYPE({rtype})")
}
