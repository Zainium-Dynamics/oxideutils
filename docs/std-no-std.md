# OxideUtils: `std` + `no_std` dual mode

**Zainium Dynamics** — kernel is `no_std`, userland is `std`.  
`oxideutils-core` supports both from one codebase.

---

## Profiles

| Profile | Cargo features | Target |
|---------|----------------|--------|
| **Userland (default)** | `std`, `disasm`, `dwarf`, … | Host tools (`oxide-objdump`, …) |
| **Kernel** | `alloc` + `disasm` (+ `kernel` alias) | Zainium kernel / freestanding |

---

## Unified build (recommended — both in one command)

Cargo **feature-unification** means a single `cargo build` cannot produce a true
`no_std` rlib while workspace tools also enable `std`.  
Zainium Dynamics solves this with **one script, two target dirs**:

```bash
# From oxideutils/
cargo build --release
# optional kernel core:
cargo build -p oxideutils-core --release --no-default-features \
  --features "alloc,disasm,kernel" --target-dir target-nostd

# Results:
#   target/debug/oxide-*              ← std tools
#   target-nostd/debug/liboxideutils_core-*.rlib  ← no_std kernel core
```

| Command | What |
|---------|------|
| `cargo build --release` | host tools (see oxideutils.toml) |
| `cargo oxide-kernel` | no_std core → `target-nostd/` |
| `cargo oxide` | tools only (alias) |
| `cargo oxide-kernel` | no_std core only (alias) |

---

## Kernel dependency

```toml
# in kernel Cargo.toml
oxideutils-core = {
    path = "../oxideutils/crates/oxideutils-core",
    default-features = false,
    features = ["alloc", "disasm", "kernel"],
}
```

## Kernel usage (byte slices, no files)

```rust
#![no_std]
extern crate alloc;

use oxideutils_core::format::object::OxideObject;
use oxideutils_core::format::elf::ElfFile;
use oxideutils_core::disasm::{disassemble, DisasmOptions};
use object::Architecture;

fn inspect_module(name: &str, bytes: &[u8]) {
    let obj = OxideObject::parse_bytes(name, bytes).unwrap();
    let secs = obj.section_views().unwrap();
    let _ = secs;
    let elf = ElfFile::parse(name, bytes).unwrap();
    let _ = elf.format_elf_header();
}
```

---

## What works where

| Capability | `no_std` + `alloc` | `std` |
|------------|--------------------|-------|
| Parse ELF/PE/Mach-O/Wasm from `&[u8]` | yes (via `object`) | yes |
| Detailed ELF (goblin readelf-style) | yes | yes |
| Symbols | yes | yes |
| Archives (in-memory) | yes | yes |
| Disasm x86/x86_64 (`iced-x86`) | yes (`disasm`) | yes |
| File I/O / mmap | no | yes |
| CLI / clap tools | no | yes |
| strip / objcopy write | no | yes |
| addr2line DWARF load from path | no | yes (`dwarf`) |

---

## Separate build commands

```bash
# Userland tools (default)
cargo build --release

# Core only — kernel style (always use --target-dir target-nostd if tools were built too)
cargo build -p oxideutils-core --no-default-features \
  --features "alloc,disasm,kernel" --target-dir target-nostd
```

---

## Design rules

1. Core parsers take **`&[u8]` + label string**, never require `std::fs` on the kernel path.  
2. Host tools stay thin CLIs that map files → slices → core.  
3. Do not pull `std` into kernel crates transitively — always `default-features = false` for kernel deps.  

See also: [kernel-integration.md](./kernel-integration.md), [building.md](./building.md).
