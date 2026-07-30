//! Apply ELF x86_64 relocations (from BFD `elf64-x86-64.c` reloc howtos).
//!
//! Supported r_types (object::elf):
//!   R_X86_64_NONE, R_X86_64_64, R_X86_64_PC32, R_X86_64_PLT32,
//!   R_X86_64_32, R_X86_64_32S, R_X86_64_PC64, R_X86_64_TPOFF32/64,
//!   R_X86_64_GOTTPOFF
//!
//! TLS relocs: the caller (`linker.rs`) passes an already-computed `s` —
//! either `tpoff(sym)` bit-reinterpreted as `u64` for TPOFF32/64, or the
//! `.got.tls` slot's VMA for GOTTPOFF — this module just does the generic
//! "plain absolute write" / "PC-relative write" arithmetic either way.

use anyhow::{Result, bail};
use object::elf;

/// Apply one relocation into `section_data` at `offset`.
/// `p` is the place address (section VMA + offset).
pub fn apply_reloc(
    section_data: &mut [u8],
    offset: usize,
    r_type: u32,
    s: u64,
    a: i64,
    p: u64,
) -> Result<()> {
    match r_type {
        elf::R_X86_64_NONE => Ok(()),
        elf::R_X86_64_64 | elf::R_X86_64_PC64 | elf::R_X86_64_TPOFF64 => {
            if offset + 8 > section_data.len() {
                bail!("reloc offset out of range");
            }
            let val = if r_type == elf::R_X86_64_PC64 {
                (s as i64).wrapping_add(a).wrapping_sub(p as i64) as u64
            } else {
                (s as i64).wrapping_add(a) as u64
            };
            section_data[offset..offset + 8].copy_from_slice(&val.to_le_bytes());
            Ok(())
        }
        elf::R_X86_64_PC32
        | elf::R_X86_64_PLT32
        | elf::R_X86_64_GOTPCREL
        | elf::R_X86_64_GOTTPOFF => {
            // PLT32/GOTPCREL/GOTTPOFF: for static link treat like PC32 (S+A-P)
            if offset + 4 > section_data.len() {
                bail!("reloc offset out of range");
            }
            // Read existing implicit addend
            let impl_addend =
                i32::from_le_bytes(section_data[offset..offset + 4].try_into().unwrap());
            let addend = if a != 0 { a } else { impl_addend as i64 };
            let val = (s as i64).wrapping_add(addend).wrapping_sub(p as i64) as i32;
            section_data[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
            Ok(())
        }
        elf::R_X86_64_32 | elf::R_X86_64_32S | elf::R_X86_64_TPOFF32 => {
            if offset + 4 > section_data.len() {
                bail!("reloc offset out of range");
            }
            let impl_addend =
                i32::from_le_bytes(section_data[offset..offset + 4].try_into().unwrap());
            let addend = if a != 0 { a } else { impl_addend as i64 };
            let val = (s as i64).wrapping_add(addend) as u32;
            section_data[offset..offset + 4].copy_from_slice(&val.to_le_bytes());
            Ok(())
        }
        elf::R_X86_64_8 | elf::R_X86_64_PC8 => {
            if offset >= section_data.len() {
                bail!("reloc offset out of range");
            }
            let val = if r_type == elf::R_X86_64_PC8 {
                (s as i64).wrapping_add(a).wrapping_sub(p as i64) as u8
            } else {
                (s as i64).wrapping_add(a) as u8
            };
            section_data[offset] = val;
            Ok(())
        }
        other => bail!("unsupported relocation type {other}"),
    }
}

/// Map object::RelocationKind to ELF r_type when reading generic relocs.
pub fn r_type_from_object(
    kind: object::RelocationKind,
    size: u8,
    encoding: object::RelocationEncoding,
) -> u32 {
    use object::{RelocationEncoding as E, RelocationKind as K};
    match (kind, size, encoding) {
        (K::Absolute, 64, _) => elf::R_X86_64_64,
        (K::Absolute, 32, E::X86Signed) => elf::R_X86_64_32S,
        (K::Absolute, 32, _) => elf::R_X86_64_32,
        (K::Relative, 32, _) | (K::GotRelative, 32, _) => elf::R_X86_64_PC32,
        (K::PltRelative, 32, _) => elf::R_X86_64_PLT32,
        (K::Relative, 64, _) => elf::R_X86_64_PC64,
        _ => elf::R_X86_64_NONE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pc32_call() {
        // at P=0x401005, S=0x401100, A=-4 → disp = 0x401100 - 4 - 0x401005 = 0xF7
        let mut buf = [0u8; 8];
        // opcode e8 + reloc at 1
        buf[0] = 0xe8;
        apply_reloc(&mut buf, 1, elf::R_X86_64_PC32, 0x401100, -4, 0x401005).unwrap();
        let disp = i32::from_le_bytes(buf[1..5].try_into().unwrap());
        assert_eq!(disp, 0x401100 - 4 - 0x401005);
    }

    #[test]
    fn abs64() {
        let mut buf = [0u8; 8];
        apply_reloc(&mut buf, 0, elf::R_X86_64_64, 0x404000, 0, 0).unwrap();
        assert_eq!(u64::from_le_bytes(buf), 0x404000);
    }
}
