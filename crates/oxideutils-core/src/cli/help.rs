//! Shared help, version banners, and beginner-friendly CLI help for OxideUtils.

use clap::builder::styling::{AnsiColor, Effects, Styles};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const PACKAGE: &str = "OxideUtils";
pub const VENDOR: &str = "Zainium Dynamics";
pub const HOMEPAGE: &str = "https://zainiumdynamics.tech";

pub fn clap_styles() -> Styles {
    Styles::styled()
        .header(AnsiColor::Cyan.on_default() | Effects::BOLD)
        .usage(AnsiColor::Green.on_default() | Effects::BOLD)
        .literal(AnsiColor::Yellow.on_default() | Effects::BOLD)
        .placeholder(AnsiColor::BrightBlue.on_default())
        .error(AnsiColor::Red.on_default() | Effects::BOLD)
        .valid(AnsiColor::Green.on_default())
        .invalid(AnsiColor::Red.on_default())
}

pub fn version_line(tool: &str) -> String {
    format!("{tool} (OxideUtils — {VENDOR}) {VERSION}")
}

pub fn print_version(tool: &str) {
    println!("{}", version_line(tool));
    println!("Copyright (C) 2026 {VENDOR}.");
    println!("License GPLv3: GNU General Public License version 3 only.");
    println!("This is free software: you are free to change and redistribute it.");
    println!("There is NO WARRANTY, to the extent permitted by law.");
    println!();
    println!("OxideUtils is a product of {VENDOR}.");
    println!("It is NOT a GNU project; it is a memory-safe rewrite compatible with GNU binutils.");
    println!("Project: {HOMEPAGE}");
}

pub fn bug_report_footer() -> &'static str {
    "OxideUtils (Zainium Dynamics) — https://zainiumdynamics.tech"
}

// oxide-objdump

pub mod objdump {
    pub const ABOUT: &str = "Show headers, symbols, hex dump, and disassembly of object files";

    pub const LONG_ABOUT: &str = "\
oxide-objdump — peek inside compiled files

WHAT IS THIS?
  Looks inside .o files, executables, shared libraries, and .a archives.
  Prints headers, symbols, hex bytes, or assembly.

BEGINNER START
  oxide-objdump -h -f myprog          # sections + file type
  oxide-objdump -d myprog | less      # disassembly
  oxide-objdump -t -C myprog          # symbols (demangled)
";

    pub const AFTER_HELP: &str = "\
EXAMPLES
  oxide-objdump -f /bin/ls
  oxide-objdump -h ./target/debug/myapp
  oxide-objdump -d ./a.out | less
  oxide-objdump -s -j .text ./a.out
";
}

// oxide-readelf

pub mod readelf {
    pub const ABOUT: &str =
        "Display ELF structure: headers, symbols, dynamic linking, versions, GOT, SFrame";

    pub const LONG_ABOUT: &str = "\
oxide-readelf — the “ELF X-ray” for Linux binaries

WHAT IS THIS?
  Prints the internal structure of ELF files (.o, .so, executables).

BEGINNER START
  oxide-readelf -h /bin/ls            # file header
  oxide-readelf -S /bin/ls            # sections
  oxide-readelf -d /bin/ls            # dynamic section
  oxide-readelf -a /bin/ls | less     # full view
";

    pub const AFTER_HELP: &str = "\
EXAMPLES
  oxide-readelf -h -S ./myapp
  oxide-readelf -d /lib64/libc.so.6
  oxide-readelf -a ./myapp | less
";
}

// oxide-nm

pub mod nm {
    pub const ABOUT: &str =
        "List symbols (functions and variables) from object files and libraries";

    pub const LONG_ABOUT: &str = "\
oxide-nm — the “phone book” of a program

WHAT IS THIS?
  Prints the symbol table: names of functions and data.

BEGINNER START
  oxide-nm ./a.out
  oxide-nm -n -C ./a.out              # by address, demangled
";
}

// oxide-ar

pub mod ar {
    pub const ABOUT: &str = "Create, list, extract, and index static libraries (.a archives)";

    pub const LONG_ABOUT: &str = "\
oxide-ar — pack .o files into a static library (.a)

MOST COMMON COMMAND
  oxide-ar rcs libfoo.a a.o b.o
";
}

// oxide-ranlib

pub mod ranlib {
    pub const ABOUT: &str = "Generate/refresh the symbol index of a static library (.a)";

    pub const LONG_ABOUT: &str = "\
oxide-ranlib — rebuild .a archive symbol index
";
}

// oxide-strip

pub mod strip {
    pub const ABOUT: &str = "Remove symbols and debug info from ELF binaries";

    pub const LONG_ABOUT: &str = "\
oxide-strip — make binaries smaller by dropping symbols
";
}

// oxide-objcopy

pub mod objcopy {
    pub const ABOUT: &str = "Copy and transform object files";

    pub const LONG_ABOUT: &str = "\
oxide-objcopy — copy and transform object files
";
}

// oxide-size

pub mod size {
    pub const ABOUT: &str = "Show code (text) / data / bss sizes of object files";

    pub const LONG_ABOUT: &str = "\
oxide-size — how big is my code and data?
";
}

// oxide-strings

pub mod strings {
    pub const ABOUT: &str = "Print readable text strings found inside a binary";

    pub const LONG_ABOUT: &str = "\
oxide-strings — find text hidden inside a binary
";
}

// oxide-addr2line

pub mod addr2line {
    pub const ABOUT: &str = "Map code addresses to source file:line";

    pub const LONG_ABOUT: &str = "\
oxide-addr2line — “where in the source is this address?”
";
}

// oxide-c++filt

pub mod cxxfilt {
    pub const ABOUT: &str = "Demangle C++ and Rust symbol names";

    pub const LONG_ABOUT: &str = "\
oxide-c++filt — turn _ZN3Foo3barEv back into Foo::bar()

Demangles symbols given as arguments, or scans stdin line by line and
demangles any mangled-looking identifier it finds, passing everything
else through unchanged (so it works on raw linker/compiler output).
";
}

// oxide-elfedit

pub mod elfedit {
    pub const ABOUT: &str = "Patch ELF header fields (machine, type, OSABI, ABI version) in place";

    pub const LONG_ABOUT: &str = "\
oxide-elfedit — rewrite a few ELF header bytes without relinking

Give it one or more --output-* fields to change, plus optional
--input-* filters so it only touches files that already match. Refuses
files that aren't ELF, and files that don't match the input filters.
";
}

// Multicall (oxideutils)

pub mod multicall {
    pub fn usage() -> String {
        String::from(
            "oxideutils — run any OxideUtils tool from one command

Usage: oxideutils <tool> [options...]

Available tools:
  objdump     Show headers, symbols, disassembly
  readelf     Display ELF structure and sections
  nm          List symbols from object files
  size        Show text, data, and bss sizes
  strings     Extract printable strings from binaries
  ar          Create and manage static libraries (.a)
  ranlib      Rebuild archive symbol index
  strip       Remove symbols and debug info
  objcopy     Copy and transform object files
  addr2line   Convert addresses to source lines
  c++filt     Demangle C++/Rust symbol names
  elfedit     Patch ELF header fields in place

Examples:
  oxideutils objdump -d ./myapp
  oxideutils readelf -a /bin/ls
  oxideutils nm -C ./target/release/app

OxideUtils is a product of Zainium Dynamics.
It is a memory-safe rewrite compatible with GNU binutils.
Project: https://zainiumdynamics.tech
",
        )
    }
}
