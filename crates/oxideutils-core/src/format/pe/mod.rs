//! PE/COFF format helpers (`std` preferred; needs goblin pe features).

pub mod header;
pub mod section;
pub mod symbol;

use crate::error::{OxideError, Result};
use goblin::pe::PE;

pub struct PeFile<'data> {
    pub pe: PE<'data>,
}

impl<'data> PeFile<'data> {
    pub fn parse(path: impl core::fmt::Display, data: &'data [u8]) -> Result<Self> {
        let path = format!("{path}");
        let pe = PE::parse(data)
            .map_err(|e| OxideError::format(path, format!("PE parse error: {e}")))?;
        Ok(Self { pe })
    }

    pub fn format_summary(&self) -> String {
        header::format_pe_header(&self.pe)
    }
}
