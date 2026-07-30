# oxideutils-core API guide

Library used by all OxideUtils tools and by the **Zainium kernel**.

```toml
# Host / tools (default features)
oxideutils-core = { path = "crates/oxideutils-core" }

# Kernel
oxideutils-core = {
    path = "crates/oxideutils-core",
    default-features = false,
    features = ["alloc", "disasm", "kernel"],
}
```

---

## Constants

```rust
oxideutils_core::VERSION;    // "0.1.0"
oxideutils_core::HAS_STD;    // bool
oxideutils_core::HAS_DISASM; // bool
```

---

## Errors

```rust
use oxideutils_core::{OxideError, Result};

fn demo() -> Result<()> {
    Err(OxideError::InvalidArgument("bad addr".into()))
}
```

| Variant | When |
|---------|------|
| `Format { path, message }` | Parse/format failure |
| `UnrecognizedFormat` | Unknown object |
| `InvalidArgument` | Bad CLI/value |
| `SectionNotFound` / `SymbolNotFound` | Lookup miss |
| `NotImplemented` | Scaffold path |
| `Io { .. }` | **std only** |

`exit_code()` → `1` or `2` for process status.

---

## Parse objects (std + no_std)

```rust
use oxideutils_core::format::object::OxideObject;

// label can be a path string or module name
let obj = OxideObject::parse("vmlinux", image_bytes)?;
// alias
let obj = OxideObject::parse_bytes("module.ko", image_bytes)?;

obj.format_name();           // "elf", "pe", …
obj.architecture_name();
obj.entry();
obj.section_views()?;
obj.format_file_header();
obj.format_section_headers()?;
obj.section_data_by_name(".text")?;
```

`OxideObject::path` is a **`String` label**, not `PathBuf`.

---

## ELF detail (readelf-style)

```rust
use oxideutils_core::format::elf::ElfFile;

let elf = ElfFile::parse("file", bytes)?;
print!("{}", elf.format_elf_header());
print!("{}", elf.format_section_headers());
print!("{}", elf.format_program_headers());
print!("{}", elf.format_dynamic());
print!("{}", elf.format_relocs());
print!("{}", elf.format_symbols());
print!("{}", elf.format_notes());
// Phase D depth (GNU binutils 2.46-oriented)
print!("{}", elf.format_version_info());   // -V / --version-info
print!("{}", elf.format_got_contents());   // --got-contents
print!("{}", elf.format_unwind());         // -u / --unwind
print!("{}", elf.format_sframe(Some(".sframe"))); // header + FDE/FRE walk
```

| Method | GNU switch |
|--------|------------|
| `format_version_info` | `-V` / `--version-info` |
| `format_got_contents` | `--got-contents` |
| `format_sframe` | `--sframe[=NAME]` (deep FRE walk) |
| `format_unwind` | `-u` / `--unwind` |

```rust
// Disasm backends
use oxideutils_core::disasm::{disassemble, has_real_backend, DisasmOptions};
use object::Architecture;

assert!(has_real_backend(Architecture::X86_64));
assert!(has_real_backend(Architecture::Aarch64)); // with disasm-aarch64

let mut opts = DisasmOptions::default();
opts.allow_hex_fallback = false; // error instead of .byte dump
let insns = disassemble(Architecture::Aarch64, 0x1000, code, &opts)?;
```

---

## Archives (write — Phase C)

```rust
#[cfg(feature = "std")]
use oxideutils_core::archive_write::{ArchiveBuilder, run_ar, ArOperation};

let mut b = ArchiveBuilder::new().deterministic(true).with_symbol_index(true);
b.replace_or_add("a.o".into(), object_bytes);
b.write_to(std::path::Path::new("lib.a"))?;
```

---

## Strip / objcopy (std)

```rust
#[cfg(feature = "std")]
use oxideutils_core::strip::{strip_file, StripOptions};
#[cfg(feature = "std")]
use oxideutils_core::objcopy::{objcopy_file, ObjcopyOptions};

strip_file(in_path, out_path, StripOptions { strip_all: true, ..Default::default() })?;
```

---

## Symbols

```rust
use oxideutils_core::symbols::{list_symbols, SymbolFilter, SymbolSort};

let filter = SymbolFilter {
    demangle: true,
    numeric_sort: true,
    ..Default::default()
};
let syms = list_symbols(&obj, &filter)?;
for s in syms {
    // s.nm_type_char(), s.address, s.name, …
}
```

---

## Archives

```rust
use oxideutils_core::archive::{is_archive, OxideArchive};

if is_archive(bytes) {
    let ar = OxideArchive::parse("lib.a", bytes)?;
    for m in &ar.members {
        let member = ar.member_data(m);
        let _ = OxideObject::parse(&m.name, member);
    }
}
```

---

## Disassembly (`disasm` feature)

```rust
use oxideutils_core::disasm::{disassemble, format_disassembly_with_labels, DisasmOptions};
use object::Architecture;

let opts = DisasmOptions::default();
let insns = disassemble(Architecture::X86_64, 0x1000, text, &opts)?;
let listing = format_disassembly_with_labels(
    ".text",
    Architecture::X86_64,
    base,
    text,
    &symbols, // Vec<(u64, String)>
    &opts,
)?;
```

---

## Utilities

```rust
use oxideutils_core::utils::{parse_address, demangle_symbol, hex_dump};

let a = parse_address("0x401000")?;
let name = demangle_symbol("_ZN3foo3barE");
let dump = hex_dump(0, data, 16);
```

File helpers (`read_file`, `map_file`) exist **only** with `std`.

---

## std-only modules

| Module | Entry points |
|--------|----------------|
| `strip` | `strip_file`, `strip_bytes`, `StripOptions` |
| `objcopy` | `objcopy_file`, `objcopy_bytes`, `ObjcopyOptions` |
| `addr2line_util` | `Addr2LineContext::open`, `resolve`, `format_gnu` |
| `cli` | multicall, help, config, `Status` |

```rust
#[cfg(feature = "std")]
use oxideutils_core::strip::{strip_file, StripOptions};
```

---

## ObjectView trait

```rust
use oxideutils_core::format::traits::ObjectView;

fn show(o: &dyn ObjectView) {
    println!("{} {}", o.path(), o.format_name());
}
```

---

## Feature matrix (API surface)

| API | `alloc` | `disasm` | `std` | `dwarf` |
|-----|---------|----------|-------|---------|
| `OxideObject` | ✓ | | | |
| `ElfFile` | ✓ | | | |
| `list_symbols` | ✓ | | | |
| `OxideArchive` | ✓ | | | |
| `disassemble` | ✓ | ✓ | | |
| `read_file` / `map_file` | | | ✓ | |
| `strip_*` / `objcopy_*` | | | ✓ | |
| `Addr2LineContext` | | | ✓ | ✓ |
| `cli::*` | | | ✓ | |

---

## Versioning

- **0.1.x** — public API may evolve; pin path deps in kernel  
- Breaking changes will be noted in [CHANGELOG.md](../CHANGELOG.md)  
