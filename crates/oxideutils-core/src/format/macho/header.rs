use crate::prelude::*;
use goblin::mach::Mach;

pub fn format_mach_header(mach: &Mach<'_>) -> String {
    match mach {
        Mach::Binary(o) => {
            format!(
                "Mach-O\n  filetype: {}\n  cputype:  {}\n  ncmds:    {}\n  entry:    0x{:x}\n",
                o.header.filetype, o.header.cputype, o.header.ncmds, o.entry
            )
        }
        Mach::Fat(f) => format!("Mach-O fat binary with {} arches\n", f.narches),
    }
}
