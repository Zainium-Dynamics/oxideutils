//! Final ELF64 image assembly: ELF header, program headers, section headers,
//! `.symtab`/`.strtab`/`.shstrtab`, and (optionally) the dynamic-linking
//! section set built by [`crate::dynamic`].
//!
//! `object::write` (used by `oxide-as` for `.o` files) only knows how to
//! write relocatable `ET_REL` objects — it has no program-header / segment
//! concept — so a linked executable/shared object has to be built by hand
//! here, the same way `bfd/elf64-x86-64.c` + `bfd/elfxx-target.h` do it in
//! real `ld`, just far smaller.
//!
//! Layout convention (deliberately simplified vs. real `ld`, documented not
//! hidden): exactly two `PT_LOAD`s — one R+X covering headers, `.interp`,
//! `.text`/`.rodata`, and every read-only dynamic-linking section
//! (`.dynsym`/`.dynstr`/`.hash`/`.rela.dyn`/`.rela.plt`/`.plt`); one R+W
//! covering `.got`/`.got.plt`/`.dynamic`/`.data`/`.bss`. Real `ld` splits
//! further for RELRO hardening; we don't.
//!
//! For the R+X segment `p_offset == 0`, so for any section in it
//! `file_offset == vma - base_rx` — no separate bookkeeping needed. The R+W
//! segment's `p_offset` is chosen freely (must only share `p_vaddr`'s
//! page-alignment, which page-aligned choices trivially satisfy), so its
//! sections use `file_offset == rw_file_off + (vma - rw_vma0)`.

use anyhow::Result;
use object::elf;
use std::collections::BTreeMap;

pub const EHDR_SIZE: u64 = 64;
pub const PHDR_SIZE: u64 = 56;
pub const SHDR_SIZE: u64 = 64;
pub const PAGE_ALIGN: u64 = 0x1000;

/// A section that will end up in the output file (and, if `alloc`, mapped by
/// some `PT_LOAD`).
pub struct Section {
    pub name: String,
    pub sh_type: u32,
    pub data: Vec<u8>,
    /// `.bss`-style: occupies `data.len()` bytes of memory but zero file bytes.
    pub nobits: bool,
    pub alloc: bool,
    pub writable: bool,
    pub executable: bool,
    pub align: u64,
    pub entsize: u64,
    /// `sh_link` target, resolved to a section index at emit time.
    pub link: Option<String>,
    pub info: u32,
    /// Assigned by [`assign_vmas`]; `0` (and meaningless) until then.
    pub vma: u64,
}

impl Section {
    pub fn new(name: &str, sh_type: u32) -> Self {
        Section {
            name: name.to_string(),
            sh_type,
            data: Vec::new(),
            nobits: false,
            alloc: false,
            writable: false,
            executable: false,
            align: 1,
            entsize: 0,
            link: None,
            info: 0,
            vma: 0,
        }
    }
}

pub struct SymbolEnt {
    pub name: String,
    pub value: u64,
    pub size: u64,
    pub info: u8,
    pub shndx: u16,
    pub global: bool,
}

pub struct ExecutableImage {
    pub e_type: u16,
    pub entry: u64,
    pub interp: Option<String>,
    /// Non-dynamic-linking sections in link order (`.text`, `.rodata`,
    /// `.data`, `.bss`, ...). Dynamic-linking sections (if any) are appended
    /// separately via `dynamic_sections` so callers don't need to know the
    /// fixed set/order `dynamic.rs` uses.
    pub sections: Vec<Section>,
    pub dynamic_sections: Vec<Section>,
    pub symbols: Vec<SymbolEnt>,
}

fn header_reserve(image: &ExecutableImage) -> u64 {
    align_up(
        EHDR_SIZE + phnum(image) as u64 * PHDR_SIZE + interp_len(image),
        16,
    )
}

fn interp_len(image: &ExecutableImage) -> u64 {
    image
        .interp
        .as_ref()
        .map(|s| s.len() as u64 + 1)
        .unwrap_or(0)
}

fn has_rx(image: &ExecutableImage) -> bool {
    all_alloc(image).any(|s| !s.writable)
}

fn has_rw(image: &ExecutableImage) -> bool {
    all_alloc(image).any(|s| s.writable)
}

fn has_dynamic(image: &ExecutableImage) -> bool {
    image.dynamic_sections.iter().any(|s| s.name == ".dynamic")
}

fn has_tls(image: &ExecutableImage) -> bool {
    image
        .sections
        .iter()
        .any(|s| s.name == ".tdata" || s.name == ".tbss")
}

fn phnum(image: &ExecutableImage) -> u16 {
    image.interp.is_some() as u16
        + has_rx(image) as u16
        + has_rw(image) as u16
        + has_dynamic(image) as u16
        + has_tls(image) as u16
}

fn all_alloc(image: &ExecutableImage) -> impl Iterator<Item = &Section> {
    image
        .sections
        .iter()
        .chain(image.dynamic_sections.iter())
        .filter(|s| s.alloc)
}

fn all_alloc_mut(image: &mut ExecutableImage) -> impl Iterator<Item = &mut Section> {
    image
        .sections
        .iter_mut()
        .chain(image.dynamic_sections.iter_mut())
        .filter(|s| s.alloc)
}

fn align_up(v: u64, a: u64) -> u64 {
    if a <= 1 { v } else { (v + a - 1) & !(a - 1) }
}

/// Pass 1: assign every `alloc` section's final VMA (mutates `image` in
/// place) and return a name -> VMA map. Call this once, use the map to fill
/// in address-dependent content (PLT stubs, `.rela.plt`, `.rela.dyn`,
/// `.dynamic`, and to patch outstanding relocations), then call [`emit`].
/// Section *sizes* must not change between this call and [`emit`] — only
/// byte *contents* of already-sized buffers may be patched afterward.
pub fn assign_vmas(image: &mut ExecutableImage, base_rx: u64) -> BTreeMap<String, u64> {
    let mut vmas = BTreeMap::new();
    let mut cursor = base_rx + header_reserve(image);
    for sec in all_alloc_mut(image).filter(|s| !s.writable) {
        cursor = align_up(cursor, sec.align.max(1));
        sec.vma = cursor;
        vmas.insert(sec.name.clone(), cursor);
        cursor += sec.data.len() as u64;
    }
    cursor = align_up(cursor, PAGE_ALIGN);
    // Non-`.bss`-style sections first, `nobits` ones last, so the "how far
    // does the RW segment's *file* content extend" computation in `emit`
    // (which stops at the last non-`nobits` section) never has to pad real
    // file bytes through a `.bss` gap that isn't actually at the very end.
    for sec in all_alloc_mut(image).filter(|s| s.writable && !s.nobits) {
        cursor = align_up(cursor, sec.align.max(1));
        sec.vma = cursor;
        vmas.insert(sec.name.clone(), cursor);
        cursor += sec.data.len() as u64;
    }
    for sec in all_alloc_mut(image).filter(|s| s.writable && s.nobits) {
        cursor = align_up(cursor, sec.align.max(1));
        sec.vma = cursor;
        vmas.insert(sec.name.clone(), cursor);
        cursor += sec.data.len() as u64;
    }
    vmas
}

/// Pass 2: write the final file. Requires [`assign_vmas`] to have already
/// run (and section sizes to be unchanged since).
pub fn emit(image: &ExecutableImage, base_rx: u64) -> Result<Vec<u8>> {
    let pn = phnum(image);
    let hdr_reserve = header_reserve(image);
    let interp_bytes = image.interp.as_ref().map(|s| {
        let mut v = s.as_bytes().to_vec();
        v.push(0);
        v
    });
    let dynamic_sec = image.dynamic_sections.iter().find(|s| s.name == ".dynamic");

    // Sorted by actual VMA (assigned by `assign_vmas`), not declaration
    // order — `assign_vmas` deliberately places `nobits` (`.bss`-style)
    // sections last within the RW group regardless of where callers put
    // them in `image.sections`/`image.dynamic_sections`.
    let mut rx_secs: Vec<&Section> = all_alloc(image).filter(|s| !s.writable).collect();
    let mut rw_secs: Vec<&Section> = all_alloc(image).filter(|s| s.writable).collect();
    rx_secs.sort_by_key(|s| s.vma);
    rw_secs.sort_by_key(|s| s.vma);

    let rx_end_vma = rx_secs
        .last()
        .map(|s| s.vma + s.data.len() as u64)
        .unwrap_or(base_rx + hdr_reserve);
    let rx_filesz = rx_end_vma - base_rx;

    let rw_vma0 = rw_secs.first().map(|s| s.vma).unwrap_or(0);
    let rw_off = align_up(rx_filesz, PAGE_ALIGN);
    let rw_end_vma = rw_secs
        .last()
        .map(|s| s.vma + s.data.len() as u64)
        .unwrap_or(rw_vma0);
    let rw_memsz = rw_end_vma - rw_vma0;
    let rw_filesz: u64 = rw_secs
        .iter()
        .filter(|s| !s.nobits)
        .map(|s| {
            // contiguous-with-padding size up to (but not including) this
            // section's own bytes is handled by the write loop below; for the
            // *segment* filesz we just need "up to the end of the last non-bss
            // section with data", i.e. skip trailing .bss from filesz.
            s.vma + s.data.len() as u64 - rw_vma0
        })
        .max()
        .unwrap_or(0);

    // ---- non-alloc metadata section names for .shstrtab ----
    let mut shstrtab_names: Vec<&str> = vec![""];
    for s in image.sections.iter().chain(image.dynamic_sections.iter()) {
        shstrtab_names.push(&s.name);
    }
    shstrtab_names.extend([".symtab", ".strtab", ".shstrtab"]);
    let (shstrtab_data, shstrtab_off) = build_strtab(&shstrtab_names);
    let (symtab_data, strtab_data, first_global) = build_symtab(image);

    let symtab_off = align_up(rw_off + rw_filesz, 8);
    let strtab_off = symtab_off + symtab_data.len() as u64;
    let shstrtab_off_file = strtab_off + strtab_data.len() as u64;
    let shoff = align_up(shstrtab_off_file + shstrtab_data.len() as u64, 8);

    let total_secs = image.sections.len() + image.dynamic_sections.len();
    let mut out = Vec::new();
    write_ehdr(
        &mut out,
        image.e_type,
        elf::EM_X86_64,
        image.entry,
        EHDR_SIZE,
        pn,
        shoff,
        (total_secs + 4) as u16, // null + regular/dyn secs + symtab + strtab + shstrtab
        (total_secs + 3) as u16, // shstrtab index
    );

    const PF_X: u32 = 1;
    const PF_W: u32 = 2;
    const PF_R: u32 = 4;
    if let Some(ref ib) = interp_bytes {
        let off = EHDR_SIZE + pn as u64 * PHDR_SIZE;
        write_phdr(
            &mut out,
            elf::PT_INTERP,
            PF_R,
            off,
            base_rx + off,
            ib.len() as u64,
            ib.len() as u64,
            1,
        );
    }
    if !rx_secs.is_empty() {
        write_phdr(
            &mut out,
            elf::PT_LOAD,
            PF_R | PF_X,
            0,
            base_rx,
            rx_filesz,
            rx_filesz,
            PAGE_ALIGN,
        );
    }
    if !rw_secs.is_empty() {
        write_phdr(
            &mut out,
            elf::PT_LOAD,
            PF_R | PF_W,
            rw_off,
            rw_vma0,
            rw_filesz,
            rw_memsz.max(rw_filesz),
            PAGE_ALIGN,
        );
    }
    if let Some(d) = dynamic_sec {
        write_phdr(
            &mut out,
            elf::PT_DYNAMIC,
            PF_R | PF_W,
            rw_off + (d.vma - rw_vma0),
            d.vma,
            d.data.len() as u64,
            d.data.len() as u64,
            8,
        );
    }
    let tdata_sec = image.sections.iter().find(|s| s.name == ".tdata");
    let tbss_sec = image.sections.iter().find(|s| s.name == ".tbss");
    if tdata_sec.is_some() || tbss_sec.is_some() {
        // `.tdata`/`.tbss` need not be VMA-adjacent (tpoff is computed from
        // their *sizes*, not their layout position — see `linker.rs`), so
        // p_vaddr/p_offset anchor on whichever is present; `.tbss` alone
        // contributes 0 file bytes (filesz stays 0, memsz covers it).
        let anchor = tdata_sec.or(tbss_sec).unwrap();
        let tdata_filesz = tdata_sec.map(|s| s.data.len() as u64).unwrap_or(0);
        let tbss_memsz = tbss_sec.map(|s| s.data.len() as u64).unwrap_or(0);
        let align = tdata_sec
            .map(|s| s.align)
            .into_iter()
            .chain(tbss_sec.map(|s| s.align))
            .max()
            .unwrap_or(8)
            .max(1);
        let offset = rw_off + (anchor.vma - rw_vma0);
        write_phdr(
            &mut out,
            elf::PT_TLS,
            PF_R,
            offset,
            anchor.vma,
            tdata_filesz,
            tdata_filesz + tbss_memsz,
            align,
        );
    }

    // ---- headers region content: interp string, then RX section bytes ----
    if let Some(ref ib) = interp_bytes {
        out.extend_from_slice(ib);
    }
    out.resize(hdr_reserve as usize, 0);
    for s in &rx_secs {
        let want = (s.vma - base_rx) as usize;
        if out.len() < want {
            out.resize(want, 0x90);
        }
        out.extend_from_slice(&s.data);
    }
    out.resize(rw_off as usize, 0);
    for s in &rw_secs {
        if s.nobits {
            continue;
        }
        let want = (rw_off + (s.vma - rw_vma0)) as usize;
        if out.len() < want {
            out.resize(want, 0);
        }
        out.extend_from_slice(&s.data);
    }
    out.resize(symtab_off as usize, 0);
    out.extend_from_slice(&symtab_data);
    out.extend_from_slice(&strtab_data);
    out.extend_from_slice(&shstrtab_data);
    out.resize(shoff as usize, 0);

    // ---- section header table ----
    write_shdr(&mut out, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    let mut name_to_shndx: BTreeMap<&str, u16> = BTreeMap::new();
    let mut shndx = 1u16;
    for s in image.sections.iter().chain(image.dynamic_sections.iter()) {
        name_to_shndx.insert(&s.name, shndx);
        shndx += 1;
    }
    for s in image.sections.iter().chain(image.dynamic_sections.iter()) {
        let flags = (if s.alloc { elf::SHF_ALLOC } else { 0 })
            | (if s.writable { elf::SHF_WRITE } else { 0 })
            | (if s.executable { elf::SHF_EXECINSTR } else { 0 });
        let addr = if s.alloc { s.vma } else { 0 };
        let offset = if !s.alloc || s.nobits {
            0
        } else if !s.writable {
            s.vma - base_rx
        } else {
            rw_off + (s.vma - rw_vma0)
        };
        let link_idx = s
            .link
            .as_deref()
            .and_then(|l| name_to_shndx.get(l))
            .copied()
            .unwrap_or(0);
        write_shdr(
            &mut out,
            *shstrtab_off.get(s.name.as_str()).unwrap_or(&0),
            if s.nobits { elf::SHT_NOBITS } else { s.sh_type },
            flags as u64,
            addr,
            offset,
            s.data.len() as u64,
            link_idx as u32,
            s.info,
            s.align.max(1),
            s.entsize,
        );
    }
    let symtab_shndx = shndx;
    write_shdr(
        &mut out,
        *shstrtab_off.get(".symtab").unwrap_or(&0),
        elf::SHT_SYMTAB,
        0,
        0,
        symtab_off,
        symtab_data.len() as u64,
        (symtab_shndx + 1) as u32,
        first_global,
        8,
        24,
    );
    write_shdr(
        &mut out,
        *shstrtab_off.get(".strtab").unwrap_or(&0),
        elf::SHT_STRTAB,
        0,
        0,
        strtab_off,
        strtab_data.len() as u64,
        0,
        0,
        1,
        0,
    );
    write_shdr(
        &mut out,
        *shstrtab_off.get(".shstrtab").unwrap_or(&0),
        elf::SHT_STRTAB,
        0,
        0,
        shstrtab_off_file,
        shstrtab_data.len() as u64,
        0,
        0,
        1,
        0,
    );

    Ok(out)
}

fn build_strtab(names: &[&str]) -> (Vec<u8>, BTreeMap<String, u32>) {
    let mut data = vec![0u8];
    let mut offsets: BTreeMap<String, u32> = BTreeMap::new();
    for &n in names {
        if n.is_empty() || offsets.contains_key(n) {
            continue;
        }
        let off = data.len() as u32;
        data.extend_from_slice(n.as_bytes());
        data.push(0);
        offsets.insert(n.to_string(), off);
    }
    (data, offsets)
}

fn build_symtab(image: &ExecutableImage) -> (Vec<u8>, Vec<u8>, u32) {
    let mut locals: Vec<&SymbolEnt> = image.symbols.iter().filter(|s| !s.global).collect();
    let mut globals: Vec<&SymbolEnt> = image.symbols.iter().filter(|s| s.global).collect();
    locals.sort_by(|a, b| a.name.cmp(&b.name));
    globals.sort_by(|a, b| a.name.cmp(&b.name));

    let mut strtab = vec![0u8];
    let mut symtab = vec![0u8; 24]; // mandatory null symbol at index 0

    let push_sym = |symtab: &mut Vec<u8>, strtab: &mut Vec<u8>, s: &SymbolEnt| {
        let name_off = strtab.len() as u32;
        strtab.extend_from_slice(s.name.as_bytes());
        strtab.push(0);
        symtab.extend_from_slice(&name_off.to_le_bytes());
        symtab.push(s.info);
        symtab.push(0);
        symtab.extend_from_slice(&s.shndx.to_le_bytes());
        symtab.extend_from_slice(&s.value.to_le_bytes());
        symtab.extend_from_slice(&s.size.to_le_bytes());
    };
    for s in &locals {
        push_sym(&mut symtab, &mut strtab, s);
    }
    let first_global = 1 + locals.len() as u32;
    for s in &globals {
        push_sym(&mut symtab, &mut strtab, s);
    }
    (symtab, strtab, first_global)
}

#[allow(clippy::too_many_arguments)]
fn write_ehdr(
    out: &mut Vec<u8>,
    e_type: u16,
    e_machine: u16,
    entry: u64,
    phoff: u64,
    phnum: u16,
    shoff: u64,
    shnum: u16,
    shstrndx: u16,
) {
    out.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    out.extend_from_slice(&e_type.to_le_bytes());
    out.extend_from_slice(&e_machine.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&entry.to_le_bytes());
    out.extend_from_slice(&phoff.to_le_bytes());
    out.extend_from_slice(&shoff.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&(EHDR_SIZE as u16).to_le_bytes());
    out.extend_from_slice(&(PHDR_SIZE as u16).to_le_bytes());
    out.extend_from_slice(&phnum.to_le_bytes());
    out.extend_from_slice(&(SHDR_SIZE as u16).to_le_bytes());
    out.extend_from_slice(&shnum.to_le_bytes());
    out.extend_from_slice(&shstrndx.to_le_bytes());
}

#[allow(clippy::too_many_arguments)]
fn write_phdr(
    out: &mut Vec<u8>,
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
) {
    out.extend_from_slice(&p_type.to_le_bytes());
    out.extend_from_slice(&p_flags.to_le_bytes());
    out.extend_from_slice(&p_offset.to_le_bytes());
    out.extend_from_slice(&p_vaddr.to_le_bytes());
    out.extend_from_slice(&p_vaddr.to_le_bytes());
    out.extend_from_slice(&p_filesz.to_le_bytes());
    out.extend_from_slice(&p_memsz.to_le_bytes());
    out.extend_from_slice(&p_align.to_le_bytes());
}

#[allow(clippy::too_many_arguments)]
fn write_shdr(
    out: &mut Vec<u8>,
    name: u32,
    sh_type: u32,
    flags: u64,
    addr: u64,
    offset: u64,
    size: u64,
    link: u32,
    info: u32,
    align: u64,
    entsize: u64,
) {
    out.extend_from_slice(&name.to_le_bytes());
    out.extend_from_slice(&sh_type.to_le_bytes());
    out.extend_from_slice(&flags.to_le_bytes());
    out.extend_from_slice(&addr.to_le_bytes());
    out.extend_from_slice(&offset.to_le_bytes());
    out.extend_from_slice(&size.to_le_bytes());
    out.extend_from_slice(&link.to_le_bytes());
    out.extend_from_slice(&info.to_le_bytes());
    out.extend_from_slice(&align.to_le_bytes());
    out.extend_from_slice(&entsize.to_le_bytes());
}
