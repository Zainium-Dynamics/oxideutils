use crate::prelude::*;
use goblin::pe::PE;

pub fn format_sections(pe: &PE<'_>) -> String {
    let mut s = String::from("\nSections:\n");
    for sec in &pe.sections {
        let name = sec.name().unwrap_or("<invalid>");
        s.push_str(&format!(
            "  {:8} VA=0x{:08x} size=0x{:x} raw=0x{:x}\n",
            name, sec.virtual_address, sec.virtual_size, sec.size_of_raw_data
        ));
    }
    s
}
