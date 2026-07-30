//! Lightweight shared parse helpers (clap is used per-tool).

use crate::error::{OxideError, Result};
use std::path::PathBuf;

/// Require at least one path operand.
pub fn require_inputs(files: &[PathBuf]) -> Result<()> {
    if files.is_empty() {
        Err(OxideError::NoInputFiles)
    } else {
        Ok(())
    }
}

/// Parse optional hex/dec address list.
pub fn parse_optional_address(s: &Option<String>) -> Result<Option<u64>> {
    match s {
        None => Ok(None),
        Some(v) => crate::utils::parse_address(v).map(Some),
    }
}
