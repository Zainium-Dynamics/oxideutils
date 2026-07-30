//! ELF format helpers (readelf / objdump private headers).
//! Works on `no_std` + `alloc` (kernel) and `std` (userland).

use crate::prelude::*;

pub mod dynamic;
pub mod got;
pub mod header;
pub mod note;
pub mod relocation;
pub mod section;
pub mod segment;
pub mod sframe;
pub mod symbol;
pub mod unwind;
pub mod version;

use crate::error::{OxideError, Result};
use goblin::elf::Elf;

/// Parsed ELF with goblin for detailed readelf-style output.
pub struct ElfFile<'data> {
    pub elf: Elf<'data>,
    pub data: &'data [u8],
}

impl<'data> ElfFile<'data> {
    pub fn parse(path: impl core::fmt::Display, data: &'data [u8]) -> Result<Self> {
        let path = format!("{path}");
        let elf = Elf::parse(data)
            .map_err(|e| OxideError::format(path, format!("ELF parse error: {e}")))?;
        Ok(Self { elf, data })
    }

    pub fn is_64(&self) -> bool {
        self.elf.is_64
    }

    pub fn little_endian(&self) -> bool {
        self.elf.little_endian
    }

    pub fn entry(&self) -> u64 {
        self.elf.entry
    }

    pub fn machine(&self) -> u16 {
        self.elf.header.e_machine
    }

    pub fn type_str(&self) -> &'static str {
        match self.elf.header.e_type {
            1 => "REL (Relocatable file)",
            2 => "EXEC (Executable file)",
            3 => "DYN (Shared object file)",
            4 => "CORE (Core file)",
            _ => "UNKNOWN",
        }
    }

    pub fn format_elf_header(&self) -> String {
        header::format_elf_header(&self.elf)
    }

    pub fn format_section_headers(&self) -> String {
        section::format_section_headers(&self.elf)
    }

    pub fn format_program_headers(&self) -> String {
        segment::format_program_headers(&self.elf)
    }

    pub fn format_dynamic(&self) -> String {
        dynamic::format_dynamic(&self.elf)
    }

    pub fn format_relocs(&self) -> String {
        relocation::format_relocs(&self.elf)
    }

    pub fn format_symbols(&self) -> String {
        symbol::format_symtab(&self.elf)
    }

    pub fn format_notes(&self) -> String {
        note::format_notes(&self.elf, self.data)
    }

    /// GNU symbol versioning (`-V` / `--version-info`).
    pub fn format_version_info(&self) -> String {
        version::format_version_info(&self.elf)
    }

    /// GOT section contents (`--got-contents`, binutils 2.46).
    pub fn format_got_contents(&self) -> String {
        got::format_got_contents(&self.elf, self.data)
    }

    /// SFrame dump (`--sframe[=SECTION]`).
    pub fn format_sframe(&self, section: Option<&str>) -> String {
        sframe::format_sframe(&self.elf, self.data, section)
    }

    /// Unwind summary (`.eh_frame` / `.eh_frame_hdr`).
    pub fn format_unwind(&self) -> String {
        unwind::format_unwind(&self.elf, self.data)
    }
}
