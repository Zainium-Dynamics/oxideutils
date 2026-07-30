//! Generic object-file wrapper over the `object` crate (BFD-like façade).
//! Works with `no_std` + `alloc` (kernel) and full `std` (userland).

use crate::error::{OxideError, Result};
use crate::format::traits::{ObjectView, SectionFlags, SectionKindView, SectionView};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Display;
use object::{
    Architecture, BinaryFormat, Endianness, File, Object, ObjectSection, SectionKind,
};

/// Opened object file with a path/label for diagnostics.
pub struct OxideObject<'data> {
    pub path: String,
    pub file: File<'data>,
}

impl<'data> OxideObject<'data> {
    /// Parse object bytes. `path` is only a label (file path or kernel module name).
    pub fn parse(path: impl Display, data: &'data [u8]) -> Result<Self> {
        let path = format!("{path}");
        let file = File::parse(data).map_err(|e| {
            OxideError::format(path.clone(), format!("file format not recognized: {e}"))
        })?;
        Ok(Self { path, file })
    }

    /// Kernel-friendly alias: same as [`parse`].
    pub fn parse_bytes(label: &str, data: &'data [u8]) -> Result<Self> {
        Self::parse(label, data)
    }

    pub fn format_name(&self) -> &'static str {
        match self.file.format() {
            BinaryFormat::Elf => "elf",
            BinaryFormat::Pe => "pe",
            BinaryFormat::MachO => "mach-o",
            BinaryFormat::Wasm => "wasm",
            BinaryFormat::Coff => "coff",
            BinaryFormat::Xcoff => "xcoff",
            _ => "unknown",
        }
    }

    pub fn architecture_name(&self) -> String {
        arch_name(self.file.architecture())
    }

    pub fn is_little_endian(&self) -> bool {
        matches!(self.file.endianness(), Endianness::Little)
    }

    pub fn is_64(&self) -> bool {
        self.file.is_64()
    }

    pub fn entry(&self) -> u64 {
        self.file.entry()
    }

    pub fn section_views(&self) -> Result<Vec<SectionView>> {
        let mut out = Vec::new();
        for (i, sec) in self.file.sections().enumerate() {
            let name = sec.name().unwrap_or("<unknown>").to_string();
            let file_offset = sec.file_range().map(|(o, _)| o);
            let flags = SectionFlags {
                alloc: matches!(
                    sec.kind(),
                    SectionKind::Text
                        | SectionKind::Data
                        | SectionKind::ReadOnlyData
                        | SectionKind::ReadOnlyString
                        | SectionKind::ReadOnlyDataWithRel
                        | SectionKind::UninitializedData
                        | SectionKind::Tls
                        | SectionKind::UninitializedTls
                ),
                write: matches!(
                    sec.kind(),
                    SectionKind::Data
                        | SectionKind::UninitializedData
                        | SectionKind::Tls
                        | SectionKind::UninitializedTls
                ),
                exec: matches!(sec.kind(), SectionKind::Text),
                tls: matches!(sec.kind(), SectionKind::Tls | SectionKind::UninitializedTls),
                compressed: {
                    #[cfg(feature = "std")]
                    {
                        sec.compressed_file_range()
                            .map(|r| r.format != object::CompressionFormat::None)
                            .unwrap_or(false)
                    }
                    #[cfg(not(feature = "std"))]
                    {
                        false
                    }
                },
            };
            let kind = map_section_kind(sec.kind());
            out.push(SectionView {
                index: i,
                name,
                address: sec.address(),
                size: sec.size(),
                file_offset,
                align: sec.align(),
                flags,
                kind,
            });
        }
        Ok(out)
    }

    pub fn section_data_by_name(&self, name: &str) -> Result<Option<Vec<u8>>> {
        for sec in self.file.sections() {
            if sec.name().ok() == Some(name) {
                #[cfg(feature = "std")]
                {
                    let data = sec
                        .uncompressed_data()
                        .map_err(|e| OxideError::format(self.path.clone(), e.to_string()))?;
                    return Ok(Some(data.into_owned()));
                }
                #[cfg(not(feature = "std"))]
                {
                    let data = sec
                        .data()
                        .map_err(|e| OxideError::format(self.path.clone(), e.to_string()))?;
                    return Ok(Some(data.to_vec()));
                }
            }
        }
        Ok(None)
    }

    pub fn format_file_header(&self) -> String {
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(
            s,
            "{}:     file format {}-{}",
            self.path,
            self.format_name(),
            if self.is_64() { "64" } else { "32" },
        );
        let _ = writeln!(
            s,
            "architecture: {}, flags 0x00000000:",
            self.architecture_name()
        );
        let _ = writeln!(s, "start address 0x{:016x}", self.entry());
        s
    }

    pub fn format_section_headers(&self) -> Result<String> {
        let secs = self.section_views()?;
        let mut s = String::new();
        use core::fmt::Write;
        let _ = writeln!(s);
        let _ = writeln!(s, "Sections:");
        let _ = writeln!(
            s,
            "{:3} {:18} {:10} {:8} {:8} {:8}  Algn",
            "Idx", "Name", "Size", "VMA", "LMA", "File off"
        );
        for sec in secs {
            let mut flag_parts = Vec::new();
            if sec.flags.alloc {
                flag_parts.push("CONTENTS");
                flag_parts.push("ALLOC");
            }
            if sec.flags.exec {
                flag_parts.push("LOAD");
                flag_parts.push("READONLY");
                flag_parts.push("CODE");
            } else if sec.flags.write {
                flag_parts.push("LOAD");
                flag_parts.push("DATA");
            } else if sec.flags.alloc {
                flag_parts.push("LOAD");
                flag_parts.push("READONLY");
                flag_parts.push("DATA");
            }
            let flags = join_flags(&flag_parts);
            let _ = writeln!(
                s,
                "{:3} {:18} {:08x}  {:08x}  {:08x}  {:08x}  2**{}",
                sec.index,
                truncate(&sec.name, 18),
                sec.size,
                sec.address,
                sec.address,
                sec.file_offset.unwrap_or(0),
                log2_align(sec.align),
            );
            let _ = writeln!(s, "                  {flags}");
        }
        Ok(s)
    }
}

impl ObjectView for OxideObject<'_> {
    fn path(&self) -> &str {
        &self.path
    }
    fn format_name(&self) -> &str {
        OxideObject::format_name(self)
    }
    fn architecture(&self) -> String {
        self.architecture_name()
    }
    fn entry(&self) -> u64 {
        OxideObject::entry(self)
    }
    fn is_64(&self) -> bool {
        OxideObject::is_64(self)
    }
    fn is_little_endian(&self) -> bool {
        OxideObject::is_little_endian(self)
    }
    fn sections(&self) -> Result<Vec<SectionView>> {
        self.section_views()
    }
    fn section_by_name(&self, name: &str) -> Result<Option<SectionView>> {
        Ok(self.section_views()?.into_iter().find(|s| s.name == name))
    }
}

fn join_flags(parts: &[&str]) -> String {
    let mut s = String::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            s.push_str(", ");
        }
        s.push_str(p);
    }
    s
}

fn map_section_kind(k: SectionKind) -> SectionKindView {
    match k {
        SectionKind::Text => SectionKindView::Text,
        SectionKind::Data => SectionKindView::Data,
        SectionKind::ReadOnlyData | SectionKind::ReadOnlyString | SectionKind::ReadOnlyDataWithRel => {
            SectionKindView::ReadOnlyData
        }
        SectionKind::UninitializedData => SectionKindView::UninitializedData,
        SectionKind::Tls => SectionKindView::Tls,
        SectionKind::UninitializedTls => SectionKindView::UninitializedTls,
        SectionKind::Debug => SectionKindView::Debug,
        SectionKind::Metadata => SectionKindView::Metadata,
        SectionKind::Note => SectionKindView::Note,
        _ => SectionKindView::Other,
    }
}

fn arch_name(a: Architecture) -> String {
    String::from(match a {
        Architecture::X86_64 => "i386:x86-64",
        Architecture::I386 => "i386",
        Architecture::Aarch64 => "aarch64",
        Architecture::Arm => "arm",
        Architecture::Riscv64 => "riscv:rv64",
        Architecture::Riscv32 => "riscv:rv32",
        Architecture::PowerPc64 => "powerpc:common64",
        Architecture::PowerPc => "powerpc:common",
        Architecture::Mips64 => "mips:4000",
        Architecture::Mips => "mips",
        Architecture::S390x => "s390:64-bit",
        Architecture::Wasm32 => "wasm32",
        Architecture::Unknown => "unknown",
        _ => "unknown",
    })
}

fn log2_align(align: u64) -> u32 {
    if align == 0 {
        0
    } else {
        align.trailing_zeros()
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}
