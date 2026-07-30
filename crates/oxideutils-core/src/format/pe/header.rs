use crate::prelude::*;
use goblin::pe::PE;

pub fn format_pe_header(pe: &PE<'_>) -> String {
    let mut s = String::new();
    s.push_str("PE File Header:\n");
    s.push_str(&format!("  Entry point:           0x{:x}\n", pe.entry));
    s.push_str(&format!("  Image base:            0x{:x}\n", pe.image_base));
    if let Some(oh) = pe.header.optional_header {
        s.push_str(&format!(
            "  Size of image:         0x{:x}\n",
            oh.windows_fields.size_of_image
        ));
        s.push_str(&format!(
            "  Subsystem:             {}\n",
            oh.windows_fields.subsystem
        ));
    }
    s.push_str(&format!("  Sections:              {}\n", pe.sections.len()));
    s
}
