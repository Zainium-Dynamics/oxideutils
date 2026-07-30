//! Unix `ar` archive handling — `no_std` + `alloc` OK.

use crate::error::{OxideError, Result};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::fmt::Display;
use object::read::archive::{ArchiveFile, ArchiveKind};

/// One member of a static archive.
#[derive(Debug, Clone)]
pub struct ArchiveMember {
    pub name: String,
    pub offset: u64,
    pub size: u64,
    pub data_range: (usize, usize),
}

/// Parsed archive view over borrowed bytes.
#[derive(Debug)]
pub struct OxideArchive<'data> {
    pub path: String,
    pub kind: ArchiveKind,
    pub members: Vec<ArchiveMember>,
    data: &'data [u8],
}

impl<'data> OxideArchive<'data> {
    pub fn parse(path: impl Display, data: &'data [u8]) -> Result<Self> {
        let path = format!("{path}");
        let archive = ArchiveFile::parse(data).map_err(|e| {
            OxideError::format(path.clone(), format!("failed to parse archive: {e}"))
        })?;

        let kind = archive.kind();
        let mut members = Vec::new();
        for member in archive.members() {
            let member =
                member.map_err(|e| OxideError::format(path.clone(), e.to_string()))?;
            let name = String::from_utf8_lossy(member.name()).into_owned();
            let offset = member.file_range().0;
            let size = member.file_range().1;
            let start = offset as usize;
            let end = start.saturating_add(size as usize).min(data.len());
            members.push(ArchiveMember {
                name,
                offset,
                size,
                data_range: (start, end),
            });
        }

        Ok(Self {
            path,
            kind,
            members,
            data,
        })
    }

    pub fn member_data(&self, member: &ArchiveMember) -> &'data [u8] {
        let (s, e) = member.data_range;
        &self.data[s..e]
    }

    pub fn kind_str(&self) -> &'static str {
        match self.kind {
            ArchiveKind::Gnu | ArchiveKind::Gnu64 => "gnu",
            ArchiveKind::Bsd | ArchiveKind::Bsd64 => "bsd",
            ArchiveKind::Coff => "coff",
            ArchiveKind::AixBig => "aixbig",
            _ => "unknown",
        }
    }
}

/// Detect whether bytes look like an `ar` archive.
pub fn is_archive(data: &[u8]) -> bool {
    data.starts_with(b"!<arch>\n") || data.starts_with(b"!<thin>\n")
}
