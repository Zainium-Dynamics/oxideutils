//! GOT section contents (GNU readelf 2.46 `--got-contents`).

use crate::prelude::*;
use goblin::elf::Elf;

/// Dump Global Offset Table style sections (`.got`, `.got.plt`, `.plt.got`, …).
pub fn format_got_contents(elf: &Elf<'_>, data: &[u8]) -> String {
    let mut s = String::new();
    let mut any = false;
    let is_64 = elf.is_64;
    let le = elf.little_endian;
    let word = if is_64 { 8 } else { 4 };

    for sh in &elf.section_headers {
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("");
        if !is_got_section(name) {
            continue;
        }
        let start = sh.sh_offset as usize;
        let size = sh.sh_size as usize;
        if start.saturating_add(size) > data.len() || size == 0 {
            continue;
        }
        any = true;
        let nents = size / word;
        s.push_str(&format!(
            "\nGlobal Offset Table '{name}' at offset 0x{:x} contains {nents} entries:\n",
            sh.sh_offset
        ));
        s.push_str("  Index  Address           Value\n");
        let bytes = &data[start..start + size];
        let base_addr = sh.sh_addr;
        for i in 0..nents {
            let off = i * word;
            let val = if is_64 {
                read_u64(&bytes[off..off + 8], le)
            } else {
                read_u32(&bytes[off..off + 4], le) as u64
            };
            let addr = base_addr + (i * word) as u64;
            s.push_str(&format!("  {i:5}  {addr:016x}  {val:016x}\n"));
        }
    }

    if !any {
        s.push_str("\nThere is no GOT section in this file.\n");
    }
    s
}

fn is_got_section(name: &str) -> bool {
    name == ".got"
        || name == ".got.plt"
        || name == ".plt.got"
        || name == ".igot"
        || name == ".igot.plt"
        || name.starts_with(".got.")
}

fn read_u32(b: &[u8], le: bool) -> u32 {
    let a: [u8; 4] = b.try_into().unwrap_or([0; 4]);
    if le {
        u32::from_le_bytes(a)
    } else {
        u32::from_be_bytes(a)
    }
}

fn read_u64(b: &[u8], le: bool) -> u64 {
    let a: [u8; 8] = b.try_into().unwrap_or([0; 8]);
    if le {
        u64::from_le_bytes(a)
    } else {
        u64::from_be_bytes(a)
    }
}
