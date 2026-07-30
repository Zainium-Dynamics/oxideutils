//! Minimal Wasm magic/version parser.

use crate::prelude::*;
pub fn is_wasm(data: &[u8]) -> bool {
    data.len() >= 8 && data[0..4] == [0x00, 0x61, 0x73, 0x6d]
}

pub fn format_wasm_summary(data: &[u8]) -> String {
    if data.len() < 8 {
        return "Wasm: truncated\n".into();
    }
    let version = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
    format!(
        "Wasm module\n  magic: \\0asm\n  version: {version}\n  size: {} bytes\n",
        data.len()
    )
}
