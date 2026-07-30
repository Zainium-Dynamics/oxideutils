//! Linker engine — multi-section merge, symbol resolution, reloc application,
//! archive/`-l` resolution, and (eager-bound) dynamic linking.
//!
//! Models GNU ld flow (`ldmain.c` / `ldlang.c` / `ldelf.c`) at a reduced
//! scale:
//! 1. Resolve every input (object / archive / `-lNAME` / shared object /
//!    `GROUP()` script) in command-line order.
//! 2. Merge input sections into output sections via the linker script.
//! 3. Resolve global symbols; for archives, only extract members that still
//!    satisfy an undefined reference (iterative fixpoint); for symbols only
//!    available from a `DT_NEEDED` shared object, resolve dynamically
//!    (function calls only, via PLT — see `dynamic.rs` for the documented
//!    scope of what "dynamic linking" means here).
//! 4. Assign VMAs to every allocated section (regular + dynamic-linking).
//! 5. Apply relocations; build PLT/GOT/`.dynamic` content now that VMAs
//!    are known.
//! 6. Emit ELF `ET_EXEC` / `ET_DYN` with `PT_LOAD` (+ `PT_INTERP` /
//!    `PT_DYNAMIC` as needed) and real section headers + `.symtab`.

use crate::archive::{LoadedArchive, load_archive};
use crate::dynamic::{self, RelativeReloc};
use crate::elfout::{self, ExecutableImage, Section as OutSec, SymbolEnt};
use crate::libsearch::{self, ResolvedInput, SharedLibInfo};
use crate::objload::{DefinedSymbol, LoadedObject, parse_one_object};
use crate::reloc::apply_reloc;
use crate::script::LinkerScript;
use anyhow::{Context, Result, bail};
use object::elf;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkType {
    Executable,
    Pie,
    Shared,
}

impl LinkType {
    pub fn from_flags(is_shared: bool, is_pie: bool) -> Self {
        if is_shared {
            LinkType::Shared
        } else if is_pie {
            LinkType::Pie
        } else {
            LinkType::Executable
        }
    }

    pub fn elf_type(self) -> u16 {
        match self {
            LinkType::Executable => elf::ET_EXEC,
            LinkType::Pie | LinkType::Shared => elf::ET_DYN,
        }
    }

    pub fn wants_interp(self, no_interp: bool) -> bool {
        if no_interp {
            return false;
        }
        !matches!(self, LinkType::Shared)
    }

    pub fn is_pic(self) -> bool {
        matches!(self, LinkType::Pie | LinkType::Shared)
    }
}

#[derive(Debug, Clone)]
pub enum LinkArg {
    File(PathBuf),
    Lib(String),
}

pub struct LinkerConfig {
    pub output_path: PathBuf,
    pub link_args: Vec<LinkArg>,
    pub search_dirs: Vec<PathBuf>,
    /// `--sysroot`: library search also checks `{sysroot}/lib` and
    /// `{sysroot}/usr/lib`. No sysroot + no `-L` means *no* implicit
    /// directories are searched — see `libsearch` module docs for why we
    /// don't fall back to host FHS paths.
    pub sysroot: Option<PathBuf>,
    pub dynamic_linker: String,
    pub is_shared: bool,
    pub is_pie: bool,
    pub no_interp: bool,
    pub static_only: bool,
    pub verbose: bool,
    pub entry: String,
    pub soname: Option<String>,
    /// Optional -T script contents.
    pub script: Option<String>,
}

impl LinkerConfig {
    pub fn link_type(&self) -> LinkType {
        LinkType::from_flags(self.is_shared, self.is_pie)
    }
}

struct OutputSectionData {
    data: Vec<u8>,
    /// pending relocs still using section-relative symbol offsets — applied
    /// once VMAs are known.
    relocs: Vec<(u64, u32, i64, String)>,
    align: u64,
}

/// One resolved link input, in original command-line order.
enum Unit {
    Object(LoadedObject),
    Archive(LoadedArchive),
    Shared(SharedLibInfo, PathBuf),
}

pub fn link_elf_executable(config: &LinkerConfig) -> Result<()> {
    let link_type = config.link_type();
    let mut script = config
        .script
        .as_deref()
        .map(LinkerScript::parse)
        .unwrap_or_default();
    script.entry = Some(config.entry.clone());

    if config.link_args.is_empty() {
        return emit_empty_smoke_binary(config, link_type);
    }

    // ---- 1. Resolve every input in order ----
    let mut units: Vec<Unit> = Vec::new();
    for arg in &config.link_args {
        resolve_link_arg(arg, config, &mut units)?;
    }

    // ---- 2 & 3. Merge root objects, then fixpoint-resolve archives,
    //             tracking still-undefined references and dynamic exports ----
    let mut outputs: BTreeMap<String, OutputSectionData> = BTreeMap::new();
    let mut defined: BTreeMap<String, DefinedSymbol> = BTreeMap::new();
    let mut undefined_refs: BTreeSet<String> = BTreeSet::new();
    let mut needed_libs: Vec<String> = Vec::new();
    let mut dynamic_exports: BTreeMap<String, String> = BTreeMap::new();
    let mut archive_members: Vec<(&LoadedArchive, usize)> = Vec::new();

    for unit in &units {
        match unit {
            Unit::Object(obj) => {
                merge_object(
                    obj,
                    &script,
                    &mut outputs,
                    &mut defined,
                    &mut undefined_refs,
                );
            }
            Unit::Archive(ar) => {
                for i in 0..ar.members.len() {
                    archive_members.push((ar, i));
                }
            }
            Unit::Shared(info, _path) => {
                if !needed_libs.contains(&info.soname) {
                    needed_libs.push(info.soname.clone());
                }
                for name in &info.exports {
                    dynamic_exports
                        .entry(name.clone())
                        .or_insert_with(|| info.soname.clone());
                }
            }
        }
    }

    let mut extracted = vec![false; archive_members.len()];
    loop {
        let mut changed = false;
        for (idx, (ar, member_idx)) in archive_members.iter().enumerate() {
            if extracted[idx] {
                continue;
            }
            let member = &ar.members[*member_idx];
            let provides = member
                .object
                .symbols
                .iter()
                .any(|s| undefined_refs.contains(&s.name));
            if provides {
                merge_object(
                    &member.object,
                    &script,
                    &mut outputs,
                    &mut defined,
                    &mut undefined_refs,
                );
                extracted[idx] = true;
                changed = true;
                if config.verbose {
                    println!(
                        "[*] extracted {}({}) for a needed symbol",
                        ar.path.display(),
                        member.name
                    );
                }
            }
        }
        if !changed {
            break;
        }
    }

    // ---- Phase 0: linker-provided crt symbols (PROVIDE-style: only if the
    //      program didn't already define them itself) ----
    inject_crt_symbols(&mut outputs, &mut defined, &mut undefined_refs);

    // ---- classify remaining undefined refs: dynamic import (PLT-able) or fatal ----
    let mut plt_imports: BTreeSet<String> = BTreeSet::new();
    let mut truly_undefined: Vec<String> = Vec::new();
    for name in &undefined_refs {
        if defined.contains_key(name) {
            continue;
        }
        if dynamic_exports.contains_key(name) {
            let kinds = reloc_kinds_for_symbol(&outputs, name);
            if kinds.iter().all(|k| *k == elf::R_X86_64_PLT32) && !kinds.is_empty() {
                plt_imports.insert(name.clone());
            } else {
                bail!(
                    "oxide-ld: `{name}' is imported from a shared library via a non-call \
                     relocation — GOT-relative data-symbol imports are not yet supported \
                     (function calls via PLT are)"
                );
            }
        } else {
            truly_undefined.push(name.clone());
        }
    }
    if !truly_undefined.is_empty() {
        for u in &truly_undefined {
            eprintln!("oxide-ld: undefined reference to `{u}'");
        }
        bail!("oxide-ld: {} undefined reference(s)", truly_undefined.len());
    }

    // ---- PIC-mode absolute-32 relocations against local symbols can't be
    //      expressed as a RELATIVE dynamic reloc (which is always 64-bit) ----
    if link_type.is_pic() {
        for (out_name, out) in &outputs {
            for (offset, r_type, _, sym_name) in &out.relocs {
                if matches!(*r_type, elf::R_X86_64_32 | elf::R_X86_64_32S)
                    && defined.contains_key(sym_name)
                {
                    bail!(
                        "oxide-ld: relocation R_X86_64_32(S) against `{sym_name}' in {out_name}+{offset:#x} \
                         can not be used when making a PIE/shared object"
                    );
                }
            }
        }
    }

    let mut plt_index: BTreeMap<String, usize> = BTreeMap::new();
    for (i, name) in plt_imports.iter().enumerate() {
        plt_index.insert(name.clone(), i);
    }

    // ---- dynsym export set (only meaningful for -shared) ----
    let mut dyn_export_syms: Vec<String> = Vec::new();
    if matches!(link_type, LinkType::Shared) {
        dyn_export_syms = defined
            .iter()
            .filter(|(_, s)| s.global)
            .map(|(n, _)| n.clone())
            .collect();
        dyn_export_syms.sort();
    }

    let has_dynamic = !plt_imports.is_empty()
        || !dyn_export_syms.is_empty()
        || matches!(link_type, LinkType::Shared);

    // ---- Phase 1: TLS layout (Variant II, x86_64 psABI) ----
    // `.tdata` (initialized) then `.tbss` (zero-fill) form one logical
    // template block starting at offset 0; `tpoff(sym) = block_offset(sym)
    // - round_up(tls_size, tls_align)`. This is independent of wherever
    // `.tdata`/`.tbss` actually end up in the final VMA layout — the offset
    // is purely a function of their *sizes*, known right after merge.
    let tdata_size = outputs
        .get(".tdata")
        .map(|o| o.data.len() as u64)
        .unwrap_or(0);
    let tbss_size = outputs
        .get(".tbss")
        .map(|o| o.data.len() as u64)
        .unwrap_or(0);
    let tls_align = outputs
        .get(".tdata")
        .map(|o| o.align)
        .into_iter()
        .chain(outputs.get(".tbss").map(|o| o.align))
        .max()
        .unwrap_or(1)
        .max(1);
    let tls_size = tdata_size + tbss_size;
    let tls_block_size = (tls_size + tls_align - 1) & !(tls_align - 1);
    let tpoff_of = |sym: &DefinedSymbol| -> i64 {
        let block_offset = if sym.section == ".tdata" {
            sym.offset
        } else {
            tdata_size + sym.offset
        };
        block_offset as i64 - tls_block_size as i64
    };

    // R_X86_64_GOTTPOFF needs one link-time-filled GOT slot per referenced
    // TLS symbol (no dynamic relocation record at all — static link, the
    // value is a compile-time constant once tls_block_size is known).
    let mut gottpoff_syms: BTreeSet<String> = BTreeSet::new();
    for out in outputs.values() {
        for (_, r_type, _, sym) in &out.relocs {
            if *r_type == elf::R_X86_64_GOTTPOFF {
                gottpoff_syms.insert(sym.clone());
            }
        }
    }
    let mut gottpoff_index: BTreeMap<String, usize> = BTreeMap::new();
    let mut got_tls_data = Vec::with_capacity(gottpoff_syms.len() * 8);
    for (i, sym_name) in gottpoff_syms.iter().enumerate() {
        gottpoff_index.insert(sym_name.clone(), i);
        let tpoff = defined.get(sym_name).map(tpoff_of).with_context(|| {
            format!("oxide-ld: `{sym_name}@gottpoff' does not name a .tdata/.tbss symbol")
        })?;
        got_tls_data.extend_from_slice(&(tpoff as u64).to_le_bytes());
    }

    // (out_section, offset_within_section, target_symbol) for every abs64
    // relocation against a locally-defined symbol in a PIC output — these
    // become `.rela.dyn` R_X86_64_RELATIVE entries once VMAs are known.
    let relative_sites: Vec<(String, u64, String)> = if link_type.is_pic() {
        outputs
            .iter()
            .flat_map(|(sec_name, o)| {
                o.relocs
                    .iter()
                    .filter(|&(_off, r_type, _, _sym)| *r_type == elf::R_X86_64_64)
                    .map(|(off, _r_type, _, sym)| (sec_name.clone(), *off, sym.clone()))
            })
            .filter(|(_, _, sym)| defined.contains_key(sym))
            .collect()
    } else {
        Vec::new()
    };

    // ---- 4. Build the ExecutableImage skeleton (regular sections first) ----
    let mut image = ExecutableImage {
        e_type: link_type.elf_type(),
        entry: 0,
        interp: if link_type.wants_interp(config.no_interp) {
            Some(config.dynamic_linker.clone())
        } else {
            None
        },
        sections: Vec::new(),
        dynamic_sections: Vec::new(),
        symbols: Vec::new(),
    };

    for name in ordered_section_names(&outputs) {
        let out = &outputs[&name];
        let is_bss = name == ".bss" || name == ".tbss";
        let mut sec = OutSec::new(&name, elf::SHT_PROGBITS);
        sec.alloc = true;
        sec.writable = matches!(name.as_str(), ".data" | ".bss" | ".tdata" | ".tbss")
            || name.starts_with(".data.");
        sec.executable = name == ".text" || name.starts_with(".text.");
        sec.align = out.align.max(1);
        sec.nobits = is_bss;
        sec.data = out.data.clone();
        image.sections.push(sec);
    }

    if !gottpoff_syms.is_empty() {
        let mut got_tls_sec = OutSec::new(".got.tls", elf::SHT_PROGBITS);
        got_tls_sec.alloc = true;
        got_tls_sec.align = 8;
        got_tls_sec.entsize = 8;
        got_tls_sec.data = got_tls_data;
        image.sections.push(got_tls_sec);
    }

    // ---- dynsym name list (imports first, then our own exports) ----
    let mut dynsym_names: Vec<String> = plt_imports.iter().cloned().collect();
    dynsym_names.extend(dyn_export_syms.iter().cloned());

    let mut dynstr_names: Vec<String> = dynsym_names.clone();
    dynstr_names.extend(needed_libs.iter().cloned());
    if let Some(s) = &config.soname {
        dynstr_names.push(s.clone());
    }
    let (dynstr_data, dynstr_off) = build_dynstr(&dynstr_names);

    if has_dynamic {
        let hash_data = dynamic::build_hash_section(
            &dynsym_names.iter().map(String::as_str).collect::<Vec<_>>(),
        );

        let mut dynsym_data = vec![0u8; 24]; // STN_UNDEF
        for name in &dynsym_names {
            let is_import = plt_imports.contains(name);
            let off = *dynstr_off.get(name).unwrap_or(&0);
            let info = ((elf::STB_GLOBAL) << 4) | elf::STT_FUNC;
            let shndx = if is_import { 0 } else { elf::SHN_ABS }; // patched below for exports
            dynamic::write_dynsym_entry(&mut dynsym_data, off, info, shndx, 0, 0);
        }

        let mut hash_sec = OutSec::new(".hash", elf::SHT_HASH);
        hash_sec.alloc = true;
        hash_sec.align = 8;
        hash_sec.entsize = 4;
        hash_sec.data = hash_data;

        let mut dynsym_sec = OutSec::new(".dynsym", elf::SHT_DYNSYM);
        dynsym_sec.alloc = true;
        dynsym_sec.align = 8;
        dynsym_sec.entsize = dynamic::SYMENT_SIZE;
        dynsym_sec.link = Some(".dynstr".to_string());
        dynsym_sec.info = 1 + plt_imports.len() as u32;
        dynsym_sec.data = dynsym_data;

        let mut dynstr_sec = OutSec::new(".dynstr", elf::SHT_STRTAB);
        dynstr_sec.alloc = true;
        dynstr_sec.align = 1;
        dynstr_sec.data = dynstr_data;

        let mut rela_plt_sec = OutSec::new(".rela.plt", elf::SHT_RELA);
        rela_plt_sec.alloc = true;
        rela_plt_sec.align = 8;
        rela_plt_sec.entsize = dynamic::RELAENT_SIZE;
        rela_plt_sec.link = Some(".dynsym".to_string());
        rela_plt_sec.data = vec![0u8; plt_imports.len() * dynamic::RELAENT_SIZE as usize];

        let mut plt_sec = OutSec::new(".plt", elf::SHT_PROGBITS);
        plt_sec.alloc = true;
        plt_sec.executable = true;
        plt_sec.align = 16;
        plt_sec.entsize = dynamic::PLT_ENTRY_SIZE;
        plt_sec.data = vec![0x90u8; plt_imports.len() * dynamic::PLT_ENTRY_SIZE as usize];

        // .rela.dyn: RELATIVE self-relocs for PIC abs64 against local syms.
        let relative_count = relative_sites.len();
        let mut rela_dyn_sec = OutSec::new(".rela.dyn", elf::SHT_RELA);
        rela_dyn_sec.alloc = true;
        rela_dyn_sec.align = 8;
        rela_dyn_sec.entsize = dynamic::RELAENT_SIZE;
        rela_dyn_sec.link = Some(".dynsym".to_string());
        rela_dyn_sec.data = vec![0u8; relative_count * dynamic::RELAENT_SIZE as usize];

        image.dynamic_sections.push(hash_sec);
        image.dynamic_sections.push(dynsym_sec);
        image.dynamic_sections.push(dynstr_sec);
        image.dynamic_sections.push(rela_dyn_sec);
        image.dynamic_sections.push(rela_plt_sec);
        image.dynamic_sections.push(plt_sec);

        let mut gotplt_sec = OutSec::new(".got.plt", elf::SHT_PROGBITS);
        gotplt_sec.alloc = true;
        gotplt_sec.writable = true;
        gotplt_sec.align = 8;
        gotplt_sec.entsize = 8;
        gotplt_sec.data = vec![0u8; plt_imports.len() * dynamic::GOT_ENTRY_SIZE as usize];

        let mut dynamic_sec = OutSec::new(".dynamic", elf::SHT_DYNAMIC);
        dynamic_sec.alloc = true;
        dynamic_sec.writable = true;
        dynamic_sec.align = 8;
        dynamic_sec.entsize = 16;
        dynamic_sec.link = Some(".dynstr".to_string());
        // sized by build_dynamic_section below once real tag list is known;
        // placeholder same length (tag count is static given our inputs).
        let placeholder_len = dynamic_entry_count(
            &needed_libs,
            config.soname.is_some(),
            !plt_imports.is_empty(),
            relative_count > 0,
        ) * 16;
        dynamic_sec.data = vec![0u8; placeholder_len];

        image.dynamic_sections.push(gotplt_sec);
        image.dynamic_sections.push(dynamic_sec);
    }

    // ---- assign VMAs (pass 1) ----
    let base_rx: u64 = if link_type.is_pic() { 0 } else { 0x400000 };
    let vmas = elfout::assign_vmas(&mut image, base_rx);

    // ---- patch dynamic-linking content now that VMAs are known ----
    if has_dynamic {
        let relatives: Vec<RelativeReloc> = relative_sites
            .iter()
            .map(|(sec_name, off, sym)| {
                let r_offset = vmas.get(sec_name).copied().unwrap_or(0) + off;
                let target = &defined[sym];
                let addend =
                    vmas.get(&target.section).copied().unwrap_or(0) as i64 + target.offset as i64;
                RelativeReloc { r_offset, addend }
            })
            .collect();
        patch_dynamic_content(
            &mut image,
            &vmas,
            &plt_imports,
            &plt_index,
            &dyn_export_syms,
            &defined,
            &needed_libs,
            config.soname.as_deref(),
            &dynstr_off,
            &relatives,
        );
    }

    // ---- 5. apply static relocations ----
    let mut abs_syms: BTreeMap<String, u64> = BTreeMap::new();
    for (name, sym) in &defined {
        let vma = vmas.get(&sym.section).copied().unwrap_or(0);
        abs_syms.insert(name.clone(), vma + sym.offset);
    }
    let plt_vma = vmas.get(".plt").copied().unwrap_or(0);
    let got_tls_vma = vmas.get(".got.tls").copied().unwrap_or(0);
    for sec in image.sections.iter_mut() {
        let Some(out) = outputs.get(&sec.name) else {
            continue;
        };
        let sec_vma = vmas.get(&sec.name).copied().unwrap_or(0);
        for (offset, r_type, addend, sym_name) in &out.relocs {
            let s = if matches!(*r_type, elf::R_X86_64_TPOFF32 | elf::R_X86_64_TPOFF64) {
                let sym = defined.get(sym_name).with_context(|| {
                    format!("oxide-ld: `{sym_name}@tpoff' does not name a .tdata/.tbss symbol")
                })?;
                tpoff_of(sym) as u64
            } else if *r_type == elf::R_X86_64_GOTTPOFF {
                got_tls_vma + gottpoff_index[sym_name] as u64 * 8
            } else if let Some(&idx) = plt_index.get(sym_name) {
                plt_vma + idx as u64 * dynamic::PLT_ENTRY_SIZE
            } else {
                *abs_syms.get(sym_name).unwrap_or(&0)
            };
            let p = sec_vma + offset;
            apply_reloc(&mut sec.data, *offset as usize, *r_type, s, *addend, p)
                .with_context(|| format!("reloc {sym_name} in {}+{offset:#x}", sec.name))?;
        }
    }

    // ---- entry + symtab ----
    let entry_name = script.entry.as_deref().unwrap_or("_start");
    let entry_addr = abs_syms
        .get(entry_name)
        .copied()
        .or_else(|| abs_syms.get("main").copied())
        .unwrap_or(0);
    image.entry = entry_addr;

    for (name, sym) in &defined {
        let vma = vmas.get(&sym.section).copied().unwrap_or(0) + sym.offset;
        image.symbols.push(SymbolEnt {
            name: name.clone(),
            value: vma,
            size: 0,
            info: ((if sym.global || sym.weak {
                elf::STB_GLOBAL
            } else {
                elf::STB_LOCAL
            }) << 4)
                | elf::STT_NOTYPE,
            shndx: 1,
            global: sym.global || sym.weak,
        });
    }

    let bytes = elfout::emit(&image, base_rx)?;
    if let Some(parent) = config.output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(&config.output_path, &bytes).with_context(|| {
        format!(
            "Failed to write linked binary: {}",
            config.output_path.display()
        )
    })?;

    if config.verbose {
        println!(
            "[OK] {:?} e_type={} entry={entry_addr:#x} needed={needed_libs:?} plt_imports={} -> {} ({} bytes)",
            link_type,
            link_type.elf_type(),
            plt_imports.len(),
            config.output_path.display(),
            bytes.len()
        );
    }
    Ok(())
}

/// Symbol/section merge for one object into the running output-section map.
/// Also folds the object's referenced-but-not-locally-defined symbol names
/// into `undefined_refs` (minus anything this very merge just defined).
fn merge_object(
    obj: &LoadedObject,
    script: &LinkerScript,
    outputs: &mut BTreeMap<String, OutputSectionData>,
    defined: &mut BTreeMap<String, DefinedSymbol>,
    undefined_refs: &mut BTreeSet<String>,
) {
    let mut local_base: BTreeMap<String, u64> = BTreeMap::new();
    for sec in &obj.sections {
        let Some(out_name) = script.map_input_section(&sec.name) else {
            continue;
        };
        let out = outputs
            .entry(out_name.clone())
            .or_insert_with(|| OutputSectionData {
                data: Vec::new(),
                relocs: Vec::new(),
                align: 1,
            });
        let align = sec.align.max(1);
        out.align = out.align.max(align);
        while !out.data.is_empty() && !(out.data.len() as u64).is_multiple_of(align) {
            out.data.push(if out_name == ".text" { 0x90 } else { 0 });
        }
        let base = out.data.len() as u64;
        local_base.insert(sec.name.clone(), base);
        out.data.extend_from_slice(&sec.data);
        for (off, r_type, addend, sym) in &sec.relocs {
            out.relocs.push((base + off, *r_type, *addend, sym.clone()));
        }
    }
    for sym in &obj.symbols {
        let Some(out_name) = script.map_input_section(&sym.section) else {
            continue;
        };
        let base = local_base.get(&sym.section).copied().unwrap_or(0);
        let candidate = DefinedSymbol {
            name: sym.name.clone(),
            section: out_name,
            offset: base + sym.offset,
            global: sym.global,
            weak: sym.weak,
        };
        match defined.get(&sym.name) {
            None => {
                defined.insert(sym.name.clone(), candidate);
            }
            Some(prev) if prev.weak && !candidate.weak => {
                defined.insert(sym.name.clone(), candidate);
            }
            Some(prev) if !prev.global && candidate.global => {
                defined.insert(sym.name.clone(), candidate);
            }
            _ => {}
        }
    }
    for name in obj.referenced_symbols() {
        if !defined.contains_key(name) {
            undefined_refs.insert(name.to_string());
        }
    }
    // Anything this object just defined might have been queued as undefined
    // by an *earlier* object; drop it now that it's satisfied.
    undefined_refs.retain(|n| !defined.contains_key(n));
}

/// Phase 0: synthesize the `PROVIDE`-style symbols real crt0/crti/crtn code
/// (musl-zainium's `crt1.c`/`Scrt1.c`, relibc's `src/crt0`) reads on
/// startup — GNU ld's default script defines these unconditionally via
/// `PROVIDE_HIDDEN`/bare assignment even with no `-T` script at all; we
/// were emitting none of them, which silently breaks constructor running
/// and any `_end`/`__bss_start`-based allocator bootstrap.
///
/// `PROVIDE` semantics: only define a name if the program didn't already
/// define it itself.
fn inject_crt_symbols(
    outputs: &mut BTreeMap<String, OutputSectionData>,
    defined: &mut BTreeMap<String, DefinedSymbol>,
    undefined_refs: &mut BTreeSet<String>,
) {
    // Always instantiate these three (possibly empty) so __start/__end are
    // always a consistent, valid, zero-length range rather than undefined
    // when no object contributes to them.
    for name in [".init_array", ".fini_array", ".preinit_array"] {
        outputs
            .entry(name.to_string())
            .or_insert_with(|| OutputSectionData {
                data: Vec::new(),
                relocs: Vec::new(),
                align: 8,
            });
    }

    let provide =
        |defined: &mut BTreeMap<String, DefinedSymbol>, name: &str, section: &str, offset: u64| {
            if defined.contains_key(name) {
                return;
            }
            defined.insert(
                name.to_string(),
                DefinedSymbol {
                    name: name.to_string(),
                    section: section.to_string(),
                    offset,
                    global: true,
                    weak: false,
                },
            );
        };

    for (base, start_name, end_name) in [
        (".init_array", "__init_array_start", "__init_array_end"),
        (".fini_array", "__fini_array_start", "__fini_array_end"),
        (
            ".preinit_array",
            "__preinit_array_start",
            "__preinit_array_end",
        ),
    ] {
        let size = outputs.get(base).map(|o| o.data.len() as u64).unwrap_or(0);
        provide(defined, start_name, base, 0);
        provide(defined, end_name, base, size);
    }

    // _edata/edata: end of initialized data, right before .bss. __bss_start:
    // start of .bss. _end/end: end of .bss — the top of the static image.
    // When `.bss` doesn't exist, all three collapse to the same point (end
    // of whichever real section is last), matching what GNU ld's default
    // script would produce for a program with no uninitialized data.
    let anchor = [".bss", ".data", ".rodata", ".text"]
        .iter()
        .find(|n| outputs.contains_key(**n))
        .copied();
    if let Some(bss_or_fallback) = anchor {
        let bss_size = outputs
            .get(".bss")
            .map(|o| o.data.len() as u64)
            .unwrap_or(0);
        let data_end_anchor = [".data", ".rodata", ".text"]
            .iter()
            .find(|n| outputs.contains_key(**n))
            .copied()
            .unwrap_or(bss_or_fallback);
        let data_end_size = outputs
            .get(data_end_anchor)
            .map(|o| o.data.len() as u64)
            .unwrap_or(0);

        provide(defined, "_edata", data_end_anchor, data_end_size);
        provide(defined, "edata", data_end_anchor, data_end_size);
        if outputs.contains_key(".bss") {
            provide(defined, "__bss_start", ".bss", 0);
            provide(defined, "_end", ".bss", bss_size);
            provide(defined, "end", ".bss", bss_size);
        } else {
            provide(defined, "__bss_start", data_end_anchor, data_end_size);
            provide(defined, "_end", data_end_anchor, data_end_size);
            provide(defined, "end", data_end_anchor, data_end_size);
        }
    }

    undefined_refs.retain(|n| !defined.contains_key(n));
}

fn ordered_section_names(outputs: &BTreeMap<String, OutputSectionData>) -> Vec<String> {
    let preferred = [
        ".text",
        ".rodata",
        ".init_array",
        ".fini_array",
        ".preinit_array",
        ".tdata",
        ".tbss",
        ".data",
        ".bss",
    ];
    let mut out: Vec<String> = preferred
        .iter()
        .filter(|n| outputs.contains_key(**n))
        .map(|n| n.to_string())
        .collect();
    for name in outputs.keys() {
        if !preferred.contains(&name.as_str()) {
            out.push(name.clone());
        }
    }
    out
}

fn reloc_kinds_for_symbol(outputs: &BTreeMap<String, OutputSectionData>, name: &str) -> Vec<u32> {
    outputs
        .values()
        .flat_map(|o| o.relocs.iter())
        .filter(|(_, _, _, s)| s == name)
        .map(|(_, r_type, _, _)| *r_type)
        .collect()
}

fn build_dynstr(names: &[String]) -> (Vec<u8>, BTreeMap<String, u32>) {
    let mut data = vec![0u8];
    let mut offsets = BTreeMap::new();
    for n in names {
        if offsets.contains_key(n) {
            continue;
        }
        let off = data.len() as u32;
        data.extend_from_slice(n.as_bytes());
        data.push(0);
        offsets.insert(n.clone(), off);
    }
    (data, offsets)
}

fn dynamic_entry_count(
    needed: &[String],
    has_soname: bool,
    has_plt: bool,
    has_rela_dyn: bool,
) -> usize {
    // NEEDED*n + SONAME? + HASH+STRTAB+SYMTAB+STRSZ+SYMENT (5) +
    // (PLTGOT+PLTRELSZ+PLTREL+JMPREL)?(4) + (RELA+RELASZ+RELAENT)?(3) + FLAGS + NULL
    needed.len()
        + has_soname as usize
        + 5
        + if has_plt { 4 } else { 0 }
        + if has_rela_dyn { 3 } else { 0 }
        + 1 // DT_FLAGS
        + 1 // DT_NULL
}

#[allow(clippy::too_many_arguments)]
fn patch_dynamic_content(
    image: &mut ExecutableImage,
    vmas: &BTreeMap<String, u64>,
    plt_imports: &BTreeSet<String>,
    plt_index: &BTreeMap<String, usize>,
    dyn_export_syms: &[String],
    defined: &BTreeMap<String, DefinedSymbol>,
    needed: &[String],
    soname: Option<&str>,
    dynstr_off: &BTreeMap<String, u32>,
    relatives: &[RelativeReloc],
) {
    let gotplt_vma = vmas.get(".got.plt").copied().unwrap_or(0);
    let plt_vma = vmas.get(".plt").copied().unwrap_or(0);
    let hash_vma = vmas.get(".hash").copied().unwrap_or(0);
    let dynsym_vma = vmas.get(".dynsym").copied().unwrap_or(0);
    let dynstr_vma = vmas.get(".dynstr").copied().unwrap_or(0);
    let rela_plt_vma = vmas.get(".rela.plt").copied().unwrap_or(0);
    let rela_dyn_vma = vmas.get(".rela.dyn").copied().unwrap_or(0);

    let dynstr_size = image
        .dynamic_sections
        .iter()
        .find(|s| s.name == ".dynstr")
        .map(|s| s.data.len() as u64)
        .unwrap_or(0);

    // .plt + .rela.plt + .got.plt content.
    let mut plt_bytes = Vec::new();
    let mut rela_plt_bytes = Vec::new();
    for name in plt_imports {
        let idx = plt_index[name];
        let entry_vma = plt_vma + idx as u64 * dynamic::PLT_ENTRY_SIZE;
        let slot_vma = gotplt_vma + idx as u64 * dynamic::GOT_ENTRY_SIZE;
        plt_bytes.extend_from_slice(&dynamic::build_plt_stub(slot_vma, entry_vma));
        // dynsym index: import entries are written first (index 1..=n).
        let dynsym_idx = 1 + idx as u32;
        dynamic::write_rela_entry(
            &mut rela_plt_bytes,
            slot_vma,
            elf::R_X86_64_JUMP_SLOT,
            dynsym_idx,
            0,
        );
    }
    set_section_data(image, ".plt", plt_bytes);
    set_section_data(image, ".rela.plt", rela_plt_bytes);

    // .dynsym: patch export entries' st_value/st_shndx now that we know them.
    if let Some(sec) = image
        .dynamic_sections
        .iter_mut()
        .find(|s| s.name == ".dynsym")
    {
        let base = 24 + plt_imports.len() * 24; // skip STN_UNDEF + imports
        for (i, name) in dyn_export_syms.iter().enumerate() {
            let off = base + i * 24;
            if let Some(sym) = defined.get(name) {
                let value = sym.offset; // patched to absolute below via vmas of its section
                let vma = vmas.get(&sym.section).copied().unwrap_or(0) + value;
                sec.data[off + 4] = elf::SHN_ABS as u8; // shndx low byte (SHN_ABS=0xfff1)
                sec.data[off + 5] = (elf::SHN_ABS >> 8) as u8;
                sec.data[off + 8..off + 16].copy_from_slice(&vma.to_le_bytes());
            }
        }
    }

    // .rela.dyn (RELATIVE self-relocs for PIC abs64 against locally defined syms).
    let mut rela_dyn_bytes = Vec::new();
    for r in relatives {
        dynamic::write_rela_entry(
            &mut rela_dyn_bytes,
            r.r_offset,
            elf::R_X86_64_RELATIVE,
            0,
            r.addend,
        );
    }
    set_section_data(image, ".rela.dyn", rela_dyn_bytes);

    let layout = dynamic::DynamicLayout {
        needed: needed.to_vec(),
        soname: soname.map(|s| s.to_string()),
        hash_vma,
        dynsym_vma,
        dynstr_vma,
        dynstr_size,
        rela_plt_vma,
        rela_plt_size: (plt_imports.len() * dynamic::RELAENT_SIZE as usize) as u64,
        rela_dyn_vma,
        rela_dyn_size: (relatives.len() * dynamic::RELAENT_SIZE as usize) as u64,
        pltgot_vma: gotplt_vma,
    };
    let dyn_bytes = dynamic::build_dynamic_section(&layout, dynstr_off);
    set_section_data(image, ".dynamic", dyn_bytes);
}

fn set_section_data(image: &mut ExecutableImage, name: &str, data: Vec<u8>) {
    if let Some(sec) = image.dynamic_sections.iter_mut().find(|s| s.name == name) {
        debug_assert_eq!(
            sec.data.len(),
            data.len(),
            "section {name} size drifted between layout and emit passes"
        );
        sec.data = data;
    }
}

fn resolve_link_arg(arg: &LinkArg, config: &LinkerConfig, units: &mut Vec<Unit>) -> Result<()> {
    match arg {
        LinkArg::File(path) => resolve_path(path, config, units),
        LinkArg::Lib(name) => {
            let path = libsearch::find_library(
                name,
                &config.search_dirs,
                config.static_only,
                config.sysroot.as_deref(),
            )?;
            resolve_path(&path, config, units)
        }
    }
}

fn resolve_path(path: &Path, config: &LinkerConfig, units: &mut Vec<Unit>) -> Result<()> {
    if !path.exists() {
        bail!("cannot open {}: No such file or directory", path.display());
    }
    if let Some(members) = libsearch::expand_group_script(path, config.sysroot.as_deref())? {
        for m in members {
            match m {
                ResolvedInput::ObjectOrArchive(p) => resolve_path(&p, config, units)?,
                ResolvedInput::SharedObject(p) => {
                    let info = libsearch::scan_shared_object(&p)?;
                    units.push(Unit::Shared(info, p));
                }
            }
        }
        return Ok(());
    }
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    if bytes.starts_with(b"!<arch>\n") {
        units.push(Unit::Archive(load_archive(&bytes, path)?));
        return Ok(());
    }
    if libsearch::is_elf_shared_object(path)? {
        let info = libsearch::scan_shared_object(path)?;
        units.push(Unit::Shared(info, path.to_path_buf()));
        return Ok(());
    }
    units.push(Unit::Object(parse_one_object(&bytes, path)?));
    Ok(())
}

fn emit_empty_smoke_binary(config: &LinkerConfig, link_type: LinkType) -> Result<()> {
    let mut image = ExecutableImage {
        e_type: link_type.elf_type(),
        entry: 0,
        interp: if link_type.wants_interp(config.no_interp) {
            Some(config.dynamic_linker.clone())
        } else {
            None
        },
        sections: Vec::new(),
        dynamic_sections: Vec::new(),
        symbols: Vec::new(),
    };
    let mut text = OutSec::new(".text", elf::SHT_PROGBITS);
    text.alloc = true;
    text.executable = true;
    text.align = 16;
    text.data = vec![0x31, 0xc0, 0xc3]; // xor eax,eax; ret
    image.sections.push(text);

    let base_rx: u64 = if link_type.is_pic() { 0 } else { 0x400000 };
    let vmas = elfout::assign_vmas(&mut image, base_rx);
    image.entry = vmas.get(".text").copied().unwrap_or(base_rx);

    let bytes = elfout::emit(&image, base_rx)?;
    fs::write(&config.output_path, &bytes).with_context(|| {
        format!(
            "Failed to write linked binary: {}",
            config.output_path.display()
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_etype_interp(data: &[u8]) -> (u16, Option<String>) {
        let e_type = u16::from_le_bytes([data[16], data[17]]);
        let phoff = u64::from_le_bytes(data[32..40].try_into().unwrap()) as usize;
        let phentsize = u16::from_le_bytes([data[54], data[55]]) as usize;
        let phnum = u16::from_le_bytes([data[56], data[57]]) as usize;
        let mut interp = None;
        for i in 0..phnum {
            let off = phoff + i * phentsize;
            let p_type = u32::from_le_bytes(data[off..off + 4].try_into().unwrap());
            if p_type == 3 {
                let p_offset =
                    u64::from_le_bytes(data[off + 8..off + 16].try_into().unwrap()) as usize;
                let p_filesz =
                    u64::from_le_bytes(data[off + 32..off + 40].try_into().unwrap()) as usize;
                let s = std::str::from_utf8(&data[p_offset..p_offset + p_filesz])
                    .unwrap()
                    .trim_end_matches('\0')
                    .to_string();
                interp = Some(s);
            }
        }
        (e_type, interp)
    }

    fn base_config(output_path: PathBuf, is_shared: bool, is_pie: bool) -> LinkerConfig {
        LinkerConfig {
            output_path,
            link_args: vec![],
            search_dirs: vec![],
            sysroot: None,
            dynamic_linker: "/lib64/ld-linux-x86-64.so.2".into(),
            is_shared,
            is_pie,
            no_interp: false,
            static_only: false,
            verbose: false,
            entry: "_start".into(),
            soname: None,
            script: None,
        }
    }

    #[test]
    fn link_types() {
        assert_eq!(LinkType::from_flags(true, true), LinkType::Shared);
        assert_eq!(LinkType::Pie.elf_type(), elf::ET_DYN);
        assert!(LinkType::Executable.wants_interp(false));
        assert!(!LinkType::Shared.wants_interp(false));
    }

    #[test]
    fn empty_link_exec_and_shared() {
        let dir = tempfile::tempdir().unwrap();
        let out = dir.path().join("a.out");
        link_elf_executable(&base_config(out.clone(), false, false)).unwrap();
        let (t, i) = parse_etype_interp(&fs::read(&out).unwrap());
        assert_eq!(t, elf::ET_EXEC);
        assert!(i.unwrap().contains("ld-linux"));

        let out2 = dir.path().join("lib.so");
        link_elf_executable(&base_config(out2.clone(), true, false)).unwrap();
        let (t2, i2) = parse_etype_interp(&fs::read(&out2).unwrap());
        assert_eq!(t2, elf::ET_DYN);
        assert!(i2.is_none());
    }
}
