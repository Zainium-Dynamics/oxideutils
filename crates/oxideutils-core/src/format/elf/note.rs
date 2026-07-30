//! ELF notes (readelf -n).

use crate::prelude::*;
use goblin::elf::Elf;

pub fn format_notes(elf: &Elf<'_>, data: &[u8]) -> String {
    let mut s = String::new();
    let mut any = false;

    for sh in &elf.section_headers {
        if sh.sh_type != goblin::elf::section_header::SHT_NOTE {
            continue;
        }
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("<note>");
        let start = sh.sh_offset as usize;
        let end = start.saturating_add(sh.sh_size as usize);
        if end > data.len() || start >= data.len() {
            continue;
        }
        let note_data = &data[start..end];
        any = true;
        s.push_str(&format!(
            "\nDisplaying notes found in: {name}\n  Owner                Data size \tDescription\n"
        ));

        // Manual parse of ELF notes (n_namesz, n_descsz, n_type)
        let mut off = 0usize;
        let align = if elf.is_64 { 8 } else { 4 };
        while off + 12 <= note_data.len() {
            let namesz = u32::from_le_bytes(note_data[off..off + 4].try_into().unwrap()) as usize;
            let descsz =
                u32::from_le_bytes(note_data[off + 4..off + 8].try_into().unwrap()) as usize;
            let ntype = u32::from_le_bytes(note_data[off + 8..off + 12].try_into().unwrap());
            off += 12;
            let name_bytes = if off + namesz <= note_data.len() {
                let nb = &note_data[off..off + namesz];
                // strip trailing NUL
                let end = nb.iter().position(|&b| b == 0).unwrap_or(nb.len());
                String::from_utf8_lossy(&nb[..end]).into_owned()
            } else {
                break;
            };
            off = align_up(off + namesz, align);
            let desc_off = off;
            off = align_up(off + descsz, align);
            if desc_off + descsz > note_data.len() {
                break;
            }
            s.push_str(&format!(
                "  {:<20} 0x{:08x}\t{}\n",
                name_bytes,
                descsz,
                note_type_desc(&name_bytes, ntype)
            ));
            // spacing is tab-separated like readelf
            if name_bytes == "GNU" && ntype == 3 && descsz >= 16 {
                // NT_GNU_BUILD_ID
                let desc = &note_data[desc_off..desc_off + descsz];
                let hex: String = desc.iter().map(|b| format!("{b:02x}")).collect();
                s.push_str(&format!("    Build ID: {hex}\n"));
            } else if name_bytes == "GNU" && ntype == 1 {
                s.push_str("    NT_GNU_ABI_TAG\n");
            }
        }
    }

    if !any {
        s.push_str("\nNo notes found in this file.\n");
    }
    s
}

fn align_up(v: usize, a: usize) -> usize {
    (v + a - 1) & !(a - 1)
}

fn note_type_desc(owner: &str, t: u32) -> &'static str {
    if owner == "GNU" {
        match t {
            1 => "NT_GNU_ABI_TAG (ABI version tag)",
            2 => "NT_GNU_HWCAP (DSO-supplied software HWCAP info)",
            3 => "NT_GNU_BUILD_ID (unique build ID bitstring)",
            4 => "NT_GNU_GOLD_VERSION (gold version)",
            5 => "NT_GNU_PROPERTY_TYPE_0",
            _ => "Unknown note type",
        }
    } else {
        match t {
            1 => "NT_VERSION (version)",
            _ => "Unknown note type",
        }
    }
}
