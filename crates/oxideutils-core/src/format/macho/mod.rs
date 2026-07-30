//! Mach-O format helpers (`std` preferred; needs goblin mach features).

pub mod header;
pub mod load_command;
pub mod section;

use crate::error::{OxideError, Result};
use goblin::mach::Mach;

pub struct MachOFile<'data> {
    pub mach: Mach<'data>,
}

impl<'data> MachOFile<'data> {
    pub fn parse(path: impl core::fmt::Display, data: &'data [u8]) -> Result<Self> {
        let path = format!("{path}");
        let mach = Mach::parse(data)
            .map_err(|e| OxideError::format(path, format!("Mach-O parse error: {e}")))?;
        Ok(Self { mach })
    }

    pub fn format_summary(&self) -> String {
        header::format_mach_header(&self.mach)
    }
}
