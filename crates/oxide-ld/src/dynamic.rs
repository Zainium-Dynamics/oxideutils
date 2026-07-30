//! Dynamic-linking data builder: `.dynsym` / `.dynstr` / `.hash` / `.plt` /
//! `.got.plt` / `.rela.plt` / `.rela.dyn` / `.dynamic`.
//!
//! Scope (documented, not hidden): eager (`DF_BIND_NOW`) binding only — no
//! lazy PLT0/`_dl_runtime_resolve` trampoline, no symbol versioning, no TLS,
//! no IFUNC. `glibc`'s `ld.so` processes `.rela.plt` during load regardless
//! of `DF_BIND_NOW` when the classic lazy stub is absent from `.plt`, so a
//! minimal "jump straight through the GOT slot" stub is sufficient and
//! matches what a real loader expects structurally (`.dynamic`/`.dynsym`/
//! relocation *tags*), even though the `.plt` bytes are simpler than GNU
//! ld's PLT0+stub layout.

use object::elf;
use std::collections::BTreeMap;

pub const PLT_ENTRY_SIZE: u64 = 16;
pub const GOT_ENTRY_SIZE: u64 = 8;
/// `Elf64_Sym` size.
pub const SYMENT_SIZE: u64 = 24;
/// `Elf64_Rela` size.
pub const RELAENT_SIZE: u64 = 24;

/// One self-relocation needed so a PIE/shared object stays correct under
/// ASLR: `word64 = load_bias + addend` at `r_offset`, applied by `ld.so`.
pub struct RelativeReloc {
    pub r_offset: u64,
    pub addend: i64,
}

/// Classic SysV `.hash` (`elf_hash` from the ELF gABI, `bfd/elf.c` style).
pub fn elf_hash(name: &str) -> u32 {
    let mut h: u32 = 0;
    for &b in name.as_bytes() {
        h = (h << 4).wrapping_add(b as u32);
        let g = h & 0xf000_0000;
        if g != 0 {
            h ^= g >> 24;
        }
        h &= !g;
    }
    h
}

pub fn build_hash_section(dynsym_names: &[&str]) -> Vec<u8> {
    // dynsym_names does NOT include the mandatory index-0 STN_UNDEF entry.
    let nchain = (dynsym_names.len() + 1) as u32;
    let nbucket = (nchain).max(1);
    let mut buckets = vec![0u32; nbucket as usize];
    let mut chain = vec![0u32; nchain as usize];
    for (i, name) in dynsym_names.iter().enumerate() {
        let sym_index = (i + 1) as u32;
        let b = (elf_hash(name) % nbucket) as usize;
        chain[sym_index as usize] = buckets[b];
        buckets[b] = sym_index;
    }
    let mut out = Vec::with_capacity(8 + buckets.len() * 4 + chain.len() * 4);
    out.extend_from_slice(&nbucket.to_le_bytes());
    out.extend_from_slice(&nchain.to_le_bytes());
    for b in buckets {
        out.extend_from_slice(&b.to_le_bytes());
    }
    for c in chain {
        out.extend_from_slice(&c.to_le_bytes());
    }
    out
}

/// `Elf64_Sym`: `st_name st_info st_other st_shndx st_value st_size`.
pub fn write_dynsym_entry(
    out: &mut Vec<u8>,
    st_name: u32,
    st_info: u8,
    st_shndx: u16,
    st_value: u64,
    st_size: u64,
) {
    out.extend_from_slice(&st_name.to_le_bytes());
    out.push(st_info);
    out.push(0); // st_other
    out.extend_from_slice(&st_shndx.to_le_bytes());
    out.extend_from_slice(&st_value.to_le_bytes());
    out.extend_from_slice(&st_size.to_le_bytes());
}

/// One `.plt` stub: `jmp *gotplt_slot(%rip)`, padded to `PLT_ENTRY_SIZE`.
pub fn build_plt_stub(gotplt_slot_vma: u64, plt_entry_vma: u64) -> [u8; PLT_ENTRY_SIZE as usize] {
    let mut out = [0x90u8; PLT_ENTRY_SIZE as usize];
    out[0] = 0xff;
    out[1] = 0x25;
    // rip at the time of the jmp is plt_entry_vma + 6 (end of this instruction).
    let disp = gotplt_slot_vma as i64 - (plt_entry_vma as i64 + 6);
    out[2..6].copy_from_slice(&(disp as i32).to_le_bytes());
    out
}

pub fn write_rela_entry(out: &mut Vec<u8>, r_offset: u64, r_type: u32, r_sym: u32, r_addend: i64) {
    let r_info = ((r_sym as u64) << 32) | r_type as u64;
    out.extend_from_slice(&r_offset.to_le_bytes());
    out.extend_from_slice(&r_info.to_le_bytes());
    out.extend_from_slice(&r_addend.to_le_bytes());
}

/// `Elf64_Dyn`: `d_tag d_val`.
pub fn write_dyn_entry(out: &mut Vec<u8>, tag: i64, val: u64) {
    out.extend_from_slice(&(tag as u64).to_le_bytes());
    out.extend_from_slice(&val.to_le_bytes());
}

pub struct DynamicLayout {
    pub needed: Vec<String>,
    pub soname: Option<String>,
    pub hash_vma: u64,
    pub dynsym_vma: u64,
    pub dynstr_vma: u64,
    pub dynstr_size: u64,
    pub rela_plt_vma: u64,
    pub rela_plt_size: u64,
    pub rela_dyn_vma: u64,
    pub rela_dyn_size: u64,
    pub pltgot_vma: u64,
}

/// Build the `.dynamic` section contents (list of `Elf64_Dyn` entries).
pub fn build_dynamic_section(layout: &DynamicLayout, dynstr_off_of: &BTreeMap<String, u32>) -> Vec<u8> {
    let mut out = Vec::new();
    for lib in &layout.needed {
        let off = dynstr_off_of.get(lib).copied().unwrap_or(0);
        write_dyn_entry(&mut out, elf::DT_NEEDED as i64, off as u64);
    }
    if let Some(soname) = &layout.soname {
        let off = dynstr_off_of.get(soname).copied().unwrap_or(0);
        write_dyn_entry(&mut out, elf::DT_SONAME as i64, off as u64);
    }
    write_dyn_entry(&mut out, elf::DT_HASH as i64, layout.hash_vma);
    write_dyn_entry(&mut out, elf::DT_STRTAB as i64, layout.dynstr_vma);
    write_dyn_entry(&mut out, elf::DT_SYMTAB as i64, layout.dynsym_vma);
    write_dyn_entry(&mut out, elf::DT_STRSZ as i64, layout.dynstr_size);
    write_dyn_entry(&mut out, elf::DT_SYMENT as i64, SYMENT_SIZE);
    if layout.rela_plt_size > 0 {
        write_dyn_entry(&mut out, elf::DT_PLTGOT as i64, layout.pltgot_vma);
        write_dyn_entry(&mut out, elf::DT_PLTRELSZ as i64, layout.rela_plt_size);
        write_dyn_entry(&mut out, elf::DT_PLTREL as i64, elf::DT_RELA as u64);
        write_dyn_entry(&mut out, elf::DT_JMPREL as i64, layout.rela_plt_vma);
    }
    if layout.rela_dyn_size > 0 {
        write_dyn_entry(&mut out, elf::DT_RELA as i64, layout.rela_dyn_vma);
        write_dyn_entry(&mut out, elf::DT_RELASZ as i64, layout.rela_dyn_size);
        write_dyn_entry(&mut out, elf::DT_RELAENT as i64, RELAENT_SIZE);
    }
    write_dyn_entry(&mut out, elf::DT_FLAGS as i64, elf::DF_BIND_NOW as u64);
    write_dyn_entry(&mut out, elf::DT_NULL as i64, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_matches_reference() {
        // Known vector from the ELF gABI spec appendix for `elf_hash`.
        assert_eq!(elf_hash(""), 0);
        assert_eq!(elf_hash("printf"), 0x077905a6);
    }

    #[test]
    fn plt_stub_disp_is_rip_relative() {
        let stub = build_plt_stub(0x404000, 0x401020);
        assert_eq!(&stub[0..2], &[0xff, 0x25]);
        let disp = i32::from_le_bytes(stub[2..6].try_into().unwrap());
        assert_eq!(0x401020i64 + 6 + disp as i64, 0x404000);
    }
}
