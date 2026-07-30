# Kernel integration (Zainium Dynamics)

How to use **OxideUtils core** inside a **`no_std`** Zainium kernel / boot environment.

---

## Principles

1. Link **`oxideutils-core` only** — never host CLI crates.  
2. Always `default-features = false`.  
3. Feed **byte slices** (mapped modules, initrd blobs, firmware).  
4. Build kernel core with **`target-nostd`** when validating next to host tools (`cargo oxide-kernel`).

---

## Cargo dependency

```toml
[dependencies]
oxideutils-core = {
    path = "../../oxideutils/crates/oxideutils-core",  # adjust
    default-features = false,
    features = ["alloc", "disasm", "kernel"],
}
```

| Feature | Kernel need |
|---------|-------------|
| `alloc` | **Required** (String, Vec) |
| `disasm` | Optional; x86/x64 decode |
| `kernel` | Convenience = alloc + disasm |
| `std` | **Do not enable** |

Ensure the kernel crate provides a global allocator (`#[global_allocator]`) if you use `alloc` types.

---

## Minimal example

```rust
#![no_std]

extern crate alloc;

use oxideutils_core::format::object::OxideObject;
use oxideutils_core::format::elf::ElfFile;
use oxideutils_core::symbols::{list_symbols, SymbolFilter};

/// Inspect a loaded ELF image (e.g. kernel module).
pub fn describe_elf(name: &str, image: &[u8]) -> Result<(), oxideutils_core::OxideError> {
    let obj = OxideObject::parse_bytes(name, image)?;

    // Sections
    for sec in obj.section_views()? {
        // log: sec.name, sec.address, sec.size, sec.flags.exec
        let _ = sec;
    }

    // Symbols
    let syms = list_symbols(&obj, &SymbolFilter::default())?;
    let _ = syms;

    // ELF header text (for early debug consoles)
    if let Ok(elf) = ElfFile::parse(name, image) {
        let _hdr = elf.format_elf_header();
        let _notes = elf.format_notes();
    }

    Ok(())
}
```

---

## Disassembly in kernel (optional)

```rust
use oxideutils_core::disasm::{disassemble, DisasmOptions};
use object::Architecture;

fn disasm_text(base: u64, text: &[u8]) {
    let opts = DisasmOptions {
        show_raw_insn: true,
        disassemble_zeroes: false,
        ..Default::default()
    };
    if let Ok(insns) = disassemble(Architecture::X86_64, base, text, &opts) {
        for i in insns.iter().take(32) {
            // print to serial: i.address, i.text
            let _ = i;
        }
    }
}
```

---

## What not to call under `no_std`

| API | Reason |
|-----|--------|
| `utils::read_file` / `map_file` | filesystem |
| `strip::strip_file` | host mutation |
| `objcopy::objcopy_file` | host mutation |
| `addr2line_util::Addr2LineContext::open` | path + std DWARF loader |
| `cli::*` | clap / process |

These modules are **not compiled** without `std`.

---

## Build verification

From OxideUtils tree:

```bash
cargo oxide-kernel
# → target-nostd/debug/liboxideutils_core-*.rlib
```

From kernel tree:

```bash
cargo build --target x86_64-unknown-none   # example
```

If the kernel build pulls `std`, check that no dependency re-enabled `oxideutils-core/std`.

---

## Suggested use cases

| Use case | APIs |
|----------|------|
| Module loader validation | `OxideObject`, `ElfFile` |
| Symbol resolve for backtrace | `list_symbols` |
| Panic / oops disassembly | `disassemble` |
| Build-id logging | `ElfFile::format_notes` |
| Initrd object walk | `OxideArchive` |

---

## Support

- Docs: [std-no-std.md](./std-no-std.md), [api-core.md](./api-core.md)  
- Contact: **alizain@zainiumdynamics.tech**  
- Web: **https://zainiumdynamics.tech**  
