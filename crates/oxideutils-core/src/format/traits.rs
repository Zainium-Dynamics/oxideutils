//! Common traits for object-file format backends (`no_std` + `alloc` OK).

use crate::error::Result;
use alloc::string::String;
use alloc::vec::Vec;

/// High-level object file capabilities used by tools.
pub trait ObjectView {
    fn path(&self) -> &str;
    fn format_name(&self) -> &str;
    fn architecture(&self) -> String;
    fn entry(&self) -> u64;
    fn is_64(&self) -> bool;
    fn is_little_endian(&self) -> bool;
    fn sections(&self) -> Result<Vec<SectionView>>;
    fn section_by_name(&self, name: &str) -> Result<Option<SectionView>>;
}

#[derive(Debug, Clone)]
pub struct SectionView {
    pub index: usize,
    pub name: String,
    pub address: u64,
    pub size: u64,
    pub file_offset: Option<u64>,
    pub align: u64,
    pub flags: SectionFlags,
    pub kind: SectionKindView,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SectionFlags {
    pub alloc: bool,
    pub write: bool,
    pub exec: bool,
    pub tls: bool,
    pub compressed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKindView {
    Text,
    Data,
    ReadOnlyData,
    UninitializedData,
    Tls,
    UninitializedTls,
    Debug,
    Other,
    Metadata,
    Note,
    Elf(u32),
}
