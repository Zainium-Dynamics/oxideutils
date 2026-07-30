//! WebAssembly object support (`no_std` + `alloc` OK).

use crate::prelude::*;

pub mod parser;

use crate::error::{OxideError, Result};

pub struct WasmFile<'data> {
    pub data: &'data [u8],
}

impl<'data> WasmFile<'data> {
    pub fn parse(path: impl core::fmt::Display, data: &'data [u8]) -> Result<Self> {
        let path = format!("{path}");
        if !parser::is_wasm(data) {
            return Err(OxideError::format(path, "not a WebAssembly module"));
        }
        Ok(Self { data })
    }

    pub fn format_summary(&self) -> String {
        parser::format_wasm_summary(self.data)
    }
}
