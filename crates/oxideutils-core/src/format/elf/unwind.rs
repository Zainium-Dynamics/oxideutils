//! Exception unwind info summary (`.eh_frame` / `.eh_frame_hdr`).
//!
//! Full CIE/FDE pretty-print is deferred; this gives a useful size/layout summary
//! for daily `readelf -u` style inspection.

use crate::prelude::*;
use goblin::elf::Elf;

/// Summarise unwind sections present in the ELF.
pub fn format_unwind(elf: &Elf<'_>, data: &[u8]) -> String {
    let mut s = String::new();
    let mut any = false;

    for sh in &elf.section_headers {
        let name = elf.shdr_strtab.get_at(sh.sh_name).unwrap_or("");
        if name != ".eh_frame"
            && name != ".eh_frame_hdr"
            && name != ".debug_frame"
            && name != ".gcc_except_table"
        {
            continue;
        }
        any = true;
        let start = sh.sh_offset as usize;
        let size = sh.sh_size as usize;
        s.push_str(&format!(
            "\nUnwind section '{name}' at offset 0x{:x} contains {} bytes:\n",
            sh.sh_offset, size
        ));
        s.push_str(&format!(
            "  Address: 0x{:x}  Align: {}  Flags: 0x{:x}\n",
            sh.sh_addr, sh.sh_addralign, sh.sh_flags
        ));
        if name == ".eh_frame" && start + size <= data.len() && size >= 8 {
            // Count CIE/FDE-ish length records (rough)
            let bytes = &data[start..start + size];
            let (n, cie, fde) = count_eh_frame_records(bytes, elf.little_endian);
            s.push_str(&format!(
                "  Approx records: {n}  (CIE-like: {cie}, FDE-like: {fde})\n"
            ));
            s.push_str(
                "  Note: detailed CIE/FDE dump not yet implemented; use llvm-dwarfdump --eh-frame for deep analysis.\n",
            );
        } else if name == ".eh_frame_hdr" && start + 4 <= data.len() {
            let ver = data[start];
            s.push_str(&format!("  eh_frame_hdr version byte: {ver}\n"));
        }
    }

    if !any {
        s.push_str("\nNo unwind sections (.eh_frame / .eh_frame_hdr) found.\n");
    }
    s
}

fn count_eh_frame_records(data: &[u8], le: bool) -> (usize, usize, usize) {
    let mut off = 0usize;
    let mut n = 0usize;
    let mut cie = 0usize;
    let mut fde = 0usize;
    while off + 4 <= data.len() {
        let len = if le {
            u32::from_le_bytes(data[off..off + 4].try_into().unwrap())
        } else {
            u32::from_be_bytes(data[off..off + 4].try_into().unwrap())
        };
        if len == 0 {
            // terminator
            break;
        }
        // 0xffffffff means 64-bit extended length — skip for rough count
        if len == 0xffff_ffff {
            break;
        }
        let rec_size = 4 + len as usize;
        if off + rec_size > data.len() {
            break;
        }
        // CIE has cie_id == 0 at next 4 bytes
        if off + 8 <= data.len() {
            let id = if le {
                u32::from_le_bytes(data[off + 4..off + 8].try_into().unwrap())
            } else {
                u32::from_be_bytes(data[off + 4..off + 8].try_into().unwrap())
            };
            if id == 0 {
                cie += 1;
            } else {
                fde += 1;
            }
        }
        n += 1;
        off += rec_size;
        // align to 4? DWARF EH often natural
    }
    (n, cie, fde)
}
