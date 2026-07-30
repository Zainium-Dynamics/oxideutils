//! Archive (`.a`) member loading + needed-symbol-driven extraction.
//!
//! Mirrors GNU ld's archive handling (`ld/ldlang.c` lang_gc_sections /
//! `bfd/archive.c` symbol index scan) at a reduced scale: rather than reading
//! the archive's own `/`/`//` symbol index, we just parse every member's ELF
//! symbol table directly (cheap enough at our scale) and extract a member
//! only once something still-undefined actually needs it.

use crate::objload::LoadedObject;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub struct ArchiveMember {
    pub name: String,
    pub object: LoadedObject,
}

pub struct LoadedArchive {
    pub path: PathBuf,
    pub members: Vec<ArchiveMember>,
}

pub fn load_archive(bytes: &[u8], path: &Path) -> Result<LoadedArchive> {
    use object::read::archive::ArchiveFile;
    let archive =
        ArchiveFile::parse(bytes).with_context(|| format!("{}: bad archive", path.display()))?;
    let mut members = Vec::new();
    for member in archive.members() {
        let m = member.map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let data = m.data(bytes).map_err(|e| anyhow::anyhow!("{e}"))?;
        if data.len() < 4 {
            continue;
        }
        let name = String::from_utf8_lossy(m.name()).to_string();
        // Skip GNU/SysV symbol-index and string-table pseudo-members.
        if name == "/" || name == "//" {
            continue;
        }
        if let Ok(object) = crate::objload::parse_one_object(data, path) {
            members.push(ArchiveMember { name, object });
        }
    }
    Ok(LoadedArchive {
        path: path.to_path_buf(),
        members,
    })
}
