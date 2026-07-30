# OxideUtils

<p align="center">
  <strong>Memory-safe binary utilities in Rust</strong><br/>
  A product of <a href="https://zainiumdynamics.tech">Zainium Dynamics</a>
</p>

<p align="center">
  GNU-binutils-<em>compatible</em> · <strong>Not a GNU project</strong> · GPLv3 · <code>std</code> + <code>no_std</code>
</p>

---

## Table of contents

1. [Overview](#overview)
2. [Features](#features)
3. [Tool map](#tool-map)
4. [Status](#status)
5. [Quick start](#quick-start)
6. [Build (unified std + no_std)](#build-unified-std--no_std)
7. [Installation](#installation)
8. [Usage by tool](#usage-by-tool)
9. [Multicall](#multicall)
10. [Kernel (`no_std`)](#kernel-no_std)
11. [Project layout](#project-layout)
12. [Documentation index](#documentation-index)
13. [Compatibility policy](#compatibility-policy)
14. [Roadmap](#roadmap)
15. [Development](#development)
16. [Licence & contact](#licence--contact)

---

## Overview

**OxideUtils** is a modern, memory-safe suite of binary inspection and transform tools written in **Rust**, developed and owned by **[Zainium Dynamics](https://zainiumdynamics.tech)**.

It provides drop-in-oriented replacements for classic binutils-style programs (`objdump`, `nm`, `readelf`, `size`, `ar`, `strings`, `strip`, `objcopy`, `addr2line`) with:

- **Safety** — Rust ownership model; no C heap corruption class bugs in OxideUtils itself  
- **Familiar CLI** — GNU-style flags where it matters for scripts and muscle memory  
- **Dual runtime** — full **`std`** host tools **and** **`no_std` + `alloc`** core for the **Zainium kernel**  
- **Clear branding** — every `--version` banner says *Zainium Dynamics*, never “GNU”

| | |
|--|--|
| **Vendor** | Zainium Dynamics |
| **Product** | OxideUtils |
| **Version** | 0.1.0 |
| **Licence** | [GNU GPLv3 only](LICENSE) |
| **Language** | Rust **1.85+** / latest stable (edition **2024**) |
| **Contact** | [alizain@zainiumdynamics.tech](mailto:alizain@zainiumdynamics.tech) |
| **Web** | [zainiumdynamics.tech](https://zainiumdynamics.tech) |

> **Legal / branding note**  
> OxideUtils is **not** affiliated with, endorsed by, or part of the GNU Project or the Free Software Foundation.  
> GNU binutils may be used only as a *behavioural reference* for compatibility testing.  
> Copyright © 2026 **Zainium Dynamics**.

---

## Features

| Area | What you get |
|------|----------------|
| **Object formats** | ELF (primary), PE/COFF, Mach-O, Wasm, static `ar` archives |
| **Disassembly** | **x86/x86_64** (iced-x86 gas) + **AArch64** (bad64); other arches hex fallback |
| **SFrame** | Header + FDE/FRE walk (`readelf`/`objdump --sframe`) |
| **Symbols** | `nm`-style listing, filters, demangle (Rust + C++ on host) |
| **ELF deep dive** | Headers, sections, segments, dynamic, relocs, notes, build-id |
| **DWARF** | `addr2line` with functions, demangle, inlines, pretty print |
| **Mutation** | `strip`, `objcopy` (copy / strip / section filter / `-O binary`) |
| **Kernel** | Parse `&[u8]` without `std` — see [docs/std-no-std.md](docs/std-no-std.md) |
| **Unified build** | `make` → host tools **and** `no_std` rlib in one command |

---

## Tool map

| Familiar name | OxideUtils binary | Role |
|---------------|-------------------|------|
| `objdump` | `oxide-objdump` | Headers, sections, symbols, hex, disassembly, archives |
| `nm` | `oxide-nm` | Symbol table |
| `readelf` | `oxide-readelf` | ELF structure dump |
| `size` | `oxide-size` | Section size summary (Berkeley / SysV) |
| `ar` | `oxide-ar` | Archive create / list / extract / ranlib |
| `strings` | `oxide-strings` | Printable strings |
| `strip` | `oxide-strip` | Remove symbols / debug sections |
| `objcopy` | `oxide-objcopy` | Copy & transform objects |
| `addr2line` | `oxide-addr2line` | Address → file:line / function |
| `as` | `oxide-as` | x86_64 AT&T assembler (subset) → ET_REL ELF64 |
| `ld` | `oxide-ld` | x86_64 ELF64 linker (subset) — static + dynamic (PLT/GOT) |
| multicall | `oxideutils` | `oxideutils <tool> …` dispatcher |

Shared logic lives in **`oxideutils-core`** (library).

---

## Status

| Component | Status | Notes |
|-----------|--------|--------|
| `oxideutils-core` | **Stable API (0.1)** | `std` + `no_std` dual mode |
| `oxide-objdump` | **Working** | `-h -f -t -s -d/-D`, archives, iced-x86 disasm |
| `oxide-nm` | **Working** | `-n -u -g -C -S` (print-size, GNU) `--size-sort` -p -r … |
| `oxide-readelf` | **Working** | `-h -S -l -s -d -r -n -V -u --got-contents --sframe -a` |
| `oxide-size` | **Working** | Berkeley + SysV (`-A`) |
| `oxide-strings` | **Working** | `-n`, `-t` |
| `oxide-ar` | **Working** | `t`/`p`/`x`/`r`/`q`/`d`/`s` (`rcs` create + index) |
| `oxide-strip` | **Working** | ELF32/64 strip-all / debug / unneeded (verified) |
| `oxide-objcopy` | **Working** | strip, `-j`/`-R`, `-O binary` (atomic write) |
| `oxide-addr2line` | **Working** | DWARF; aligns with GNU on line info |
| Full GNU flag parity | **In progress** | See [ROADMAP.md](ROADMAP.md) |
| AArch64 disasm | **Working** | bad64 (`disasm-aarch64`) |
| SFrame FRE walk | **Working** | v2/v3 default FDE |
| `oxide-as` | **Minimal, verified** | AT&T x86_64 + SSE/SSE2 float, SIB addressing, `sym+N` addends, `.macro`/`.rept`/`.if`, `.equ`/`.set`; errors (not silent NOP) on unknown instructions |
| `oxide-ld` | **Minimal, verified** | Static + dynamic (eager-bound PLT/GOT) linking, TLS (initial-exec), crt symbols (`__init_array_start`/…), `-l`/`-L` + archive/`GROUP()` resolution, real section headers/symtab; verified end-to-end against real glibc — see [AUDIT-REPORT §8](docs/AUDIT-REPORT-BINUTILS-2.46.1.md#oxide-as--oxide-ld--status-update-2026-07-27) for exact scope/gaps |

**Measured, not claimed:** [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) —
**34.3%** of the real GNU binutils CLI flag surface across 9 tools is
covered today (ranges from 15.7% on `objcopy` to 69.2% on `addr2line`).
[docs/BENCHMARKS.md](docs/BENCHMARKS.md) — up to **13x faster** and using
**82% less memory** on some operations (`readelf -a`, `nm`), reported
alongside a genuine weakness (`objdump -d` currently uses ~7.6x more
memory than GNU's). Both documents are reproducible from the commands
they show, not hand-picked numbers.

---

## Quick start

### Requirements

- **Rust 1.85+** (MSRV) — recommend **latest stable** (`rustup update stable`)
- **Cargo**
- Optional: system `objdump` / `nm` for side-by-side comparison
- Optional: `make` for unified build targets

### Clone / enter tree

This package often lives next to a binutils-2.42 source tree (reference only):

```bash
cd oxideutils
```

### Build (config = `oxideutils.toml` only — **no Makefile**)

```bash
# 1) Edit true/false switches
$EDITOR oxideutils.toml

# 2) Build
cargo build --release

# 3) Optional: see what was selected
cat target/oxideutils-build-plan.txt
```

| TOML | Meaning |
|------|---------|
| `[build] standalone = true` | One binary: `oxideutils <tool> …` |
| `[build] static = true` | Prefer static link (use musl target for fully static) |
| `[tools] objdump = false` | That tool bin is disabled until re-enabled |

Details: [docs/building.md](docs/building.md)

### Run

```bash
./target/release/oxide-objdump -H
./target/release/oxide-objdump -h -f /bin/ls
./target/release/oxide-nm -n ./target/release/oxide-nm
./target/release/oxide-readelf -h /bin/ls
./target/release/oxideutils --help
```

Expected version style:

```text
oxide-objdump (OxideUtils — Zainium Dynamics) 0.1.0
Copyright (C) 2026 Zainium Dynamics.
...
Project: https://zainiumdynamics.tech
```

---

## Build (TOML-driven — no Makefile)

```bash
cargo build --release          # host tools (see oxideutils.toml)
cargo test --workspace
cargo clippy --workspace --all-targets
```

| Artifact | Location |
|----------|----------|
| Tools | `target/release/oxide-*` and `oxideutils` |
| Build plan | `target/oxideutils-build-plan.txt` |
| Kernel rlib | `target-nostd/…` (only if you run the kernel cargo line) |

Kernel / `no_std` (optional, when `[build] kernel = true`):

```bash
cargo build -p oxideutils-core --release \
  --no-default-features --features "alloc,disasm,kernel" \
  --target-dir target-nostd
```

Details: [docs/building.md](docs/building.md) · [docs/std-no-std.md](docs/std-no-std.md)

---

## Installation

```bash
# From source (host tools)
cargo install --path crates/oxide-objdump
# …or install each crate, or copy binaries from target/release/

# After: cargo build --release
cp target/release/oxide-* /usr/local/bin/   # adjust prefix as needed
```

Optional symlinks for muscle memory (you choose names):

```bash
ln -s oxide-objdump ~/bin/objdump   # only if you intentionally override PATH
```

---

## Usage by tool

### oxide-objdump

```bash
oxide-objdump -h -f FILE          # section + file headers (GNU -h is sections; -H help)
oxide-objdump -t FILE             # symbol table
oxide-objdump -s FILE             # full section contents (hex)
oxide-objdump -d FILE             # disassemble executable sections
oxide-objdump -D FILE             # disassemble all sections
oxide-objdump -d --disassemble=main FILE
oxide-objdump -a FILE.a           # archive headers (+ member dumps with other flags)
```

| Common flags | Meaning |
|--------------|---------|
| `-h` | Section headers |
| `-H` / `--help` | Help (GNU-style; not `-h`) |
| `-f` | File header |
| `-t` / `-T` | Symbols / dynamic symbols |
| `-s` | Full contents |
| `-d` / `-D` | Disassemble / all sections |
| `-j NAME` | Restrict to section |
| `-C` | Demangle |
| `-z` | Do not skip zero blocks |
| `--start-address` / `--stop-address` | Range |

### oxide-nm

```bash
oxide-nm FILE
oxide-nm -n FILE          # numeric sort by address
oxide-nm -u FILE          # undefined only
oxide-nm -g FILE          # external only
oxide-nm -C FILE          # demangle
oxide-nm -A FILE          # print file name
```

### oxide-readelf

```bash
oxide-readelf -h FILE              # ELF header (-H is help)
oxide-readelf -S FILE              # section headers
oxide-readelf -l FILE              # program headers
oxide-readelf -s FILE              # symbols
oxide-readelf -d FILE              # dynamic
oxide-readelf -r FILE              # relocations (named types + symbols)
oxide-readelf -n FILE              # notes (build-id, …)
oxide-readelf -V FILE              # symbol versioning (versym/verneed)
oxide-readelf --got-contents FILE  # GOT dump (GNU 2.46)
oxide-readelf -u FILE              # unwind (.eh_frame) summary
oxide-readelf --sframe FILE        # SFrame header (if present)
oxide-readelf -a FILE              # all of the above (except --sframe)
```

### oxide-size

```bash
oxide-size FILE
oxide-size -t FILE                 # with totals
oxide-size -A sysv FILE            # SysV style
```

### oxide-strings

```bash
oxide-strings FILE
oxide-strings -n 8 FILE
oxide-strings -t x FILE            # hex offsets
```

### oxide-ar

```bash
oxide-ar rcs libfoo.a a.o b.o      # create + symbol index
oxide-ar t libfoo.a                # list
oxide-ar p libfoo.a                # print members to stdout
oxide-ar x libfoo.a                # extract
oxide-ar d libfoo.a a.o            # delete member
oxide-ar s libfoo.a                # rebuild symbol index
```

### oxide-strip

```bash
oxide-strip -s FILE                # strip all (default if no mode)
oxide-strip -g FILE                # strip debug
oxide-strip --strip-unneeded FILE
oxide-strip -o OUT IN
oxide-strip -v FILE
```

### oxide-objcopy

```bash
oxide-objcopy IN OUT
oxide-objcopy --strip-all IN OUT
oxide-objcopy -j .text -O binary IN OUT.bin
oxide-objcopy -R .comment IN OUT
```

### oxide-addr2line

```bash
oxide-addr2line -e BINARY -f -C -a 0x401234
oxide-addr2line -e BINARY -p -f -C 0x401234
oxide-addr2line -e BINARY -i -f 0x401234
# addresses from stdin if none given
echo 0x401234 | oxide-addr2line -e BINARY -f -C
```

Full per-tool notes: [docs/tools.md](docs/tools.md)

---

## Multicall

```bash
oxideutils --help
oxideutils objdump -h /bin/ls
oxideutils nm -n ./a.out
```

Dispatches to `oxide-*` binaries on `PATH` (or same install prefix).

---

## Kernel (`no_std`)

Zainium kernel builds are **`no_std`**. Depend on the core library only:

```toml
# kernel Cargo.toml
[dependencies]
oxideutils-core = {
    path = "path/to/oxideutils/crates/oxideutils-core",
    default-features = false,
    features = ["alloc", "disasm", "kernel"],
}
```

```rust
#![no_std]
extern crate alloc;

use oxideutils_core::format::object::OxideObject;
use oxideutils_core::format::elf::ElfFile;

fn inspect_module(name: &str, image: &[u8]) {
    let obj = OxideObject::parse_bytes(name, image).expect("object");
    let _sections = obj.section_views();
    if let Ok(elf) = ElfFile::parse(name, image) {
        let _ = elf.format_elf_header();
    }
}
```

| In kernel | On host |
|-----------|---------|
| Parse ELF/object from `&[u8]` | Same + files |
| Symbols, archives (memory) | + CLI tools |
| x86/x64 disasm (`disasm`) | + iced-x86 |
| No `std::fs` / clap | strip, objcopy, addr2line paths |

Guide: [docs/kernel-integration.md](docs/kernel-integration.md)

---

## Project layout

```text
oxideutils/
├── README.md                 ← you are here
├── LICENSE                   ← GPLv3 only
├── Cargo.toml                ← workspace
├── oxideutils.toml           ← **only** config (build + tools + runtime)
├── build/oxide_build.rs      ← build.rs shared logic (reads TOML)
├── scripts/build-all.sh      ← optional wrapper → cargo build
├── ROADMAP.md
├── CONTRIBUTING.md
├── CODE_OF_CONDUCT.md
├── CHANGELOG.md
├── docs/                     ← full documentation set
│   ├── README.md             ← docs index
│   ├── architecture.md
│   ├── building.md
│   ├── tools.md
│   ├── api-core.md
│   ├── std-no-std.md
│   ├── kernel-integration.md
│   ├── gnu-compatibility.md
│   ├── faq.md
│   └── man/                  ← man page sources
├── crates/
│   ├── oxideutils-core/      ← shared library (std + no_std)
│   ├── oxide-objdump/
│   ├── oxide-nm/
│   ├── oxide-readelf/
│   ├── oxide-size/
│   ├── oxide-ar/
│   ├── oxide-strings/
│   ├── oxide-strip/
│   ├── oxide-objcopy/
│   └── oxide-addr2line/
├── bin/                      ← reserved / multicall notes
└── tests/integration/
```

Architecture deep-dive: [docs/architecture.md](docs/architecture.md)

---

## Documentation index

| Document | Description |
|----------|-------------|
| **[docs/README.md](docs/README.md)** | Documentation hub |
| [docs/architecture.md](docs/architecture.md) | Crates, layers, data flow |
| [docs/building.md](docs/building.md) | Build via oxideutils.toml + cargo |
| [docs/tools.md](docs/tools.md) | Full CLI reference per tool |
| [docs/api-core.md](docs/api-core.md) | `oxideutils-core` library API |
| [docs/std-no-std.md](docs/std-no-std.md) | Dual-mode design |
| [docs/kernel-integration.md](docs/kernel-integration.md) | Using core in Zainium kernel |
| [docs/gnu-compatibility.md](docs/gnu-compatibility.md) | Compatibility policy vs GNU binutils **2.46.1** |
| [docs/COMPATIBILITY.md](docs/COMPATIBILITY.md) | Measured CLI flag coverage % vs real GNU binutils (reproducible method) |
| [docs/BENCHMARKS.md](docs/BENCHMARKS.md) | Speed + memory benchmarks vs GNU binutils (wins **and** a documented weakness) |
| [docs/AUDIT-REPORT-BINUTILS-2.46.1.md](docs/AUDIT-REPORT-BINUTILS-2.46.1.md) | Architecture audit + risk + roadmap |
| [docs/faq.md](docs/faq.md) | FAQ |
| [docs/configuration.md](docs/configuration.md) | **TOML config** (`true` / `false`) |
| [ROADMAP.md](ROADMAP.md) | 12-phase plan |
| [CONTRIBUTING.md](CONTRIBUTING.md) | How to contribute |
| [CHANGELOG.md](CHANGELOG.md) | Release notes |

---

## Compatibility policy

- **Goal:** behave like GNU binutils **2.46.1** for common flags and exit codes (`0` / `1` / `2`).
- **Not a goal:** byte-identical output in every corner case on day one.
- **Branding:** version strings always **Zainium Dynamics / OxideUtils**.
- Tree `packages/binutils-2.46.1` (if present) is a **reference source for tests**, not linked code.

See [docs/gnu-compatibility.md](docs/gnu-compatibility.md).

---

## Roadmap

12 phases from foundation → 1.0 (disasm, full ar, packaging, fuzz, …).  
Summary: [ROADMAP.md](ROADMAP.md).

**Today (0.1.x):** foundation, working tool suite, real disasm, strip/objcopy/addr2line, unified `std`/`no_std` build.

---

## Configuration (TOML)

We use **TOML** only — not `.configuration` / ini. Values are mostly **`true` / `false`**.

```bash
# project / cwd
cp oxideutils.toml ./oxideutils.toml   # already in tree — edit booleans

# or user global
mkdir -p ~/.config/oxideutils
cp oxideutils.toml ~/.config/oxideutils/config.toml

# show effective config
./target/debug/oxide-objdump --print-config
```

```toml
[oxideutils]
gnu_compatible = true
json = false
demangle = false

[color]
enabled = true
auto = true
always = false

[disasm]
show_raw_insn = true
disassemble_zeroes = false
```

Full schema: [docs/configuration.md](docs/configuration.md) · template: [`oxideutils.toml`](oxideutils.toml)

## Development

```bash
cargo fmt
cargo clippy --workspace --all-targets
cargo test --workspace
cargo check --workspace
```

- Licence of contributions: **GPLv3 only** (Zainium Dynamics product)  
- Code of conduct: [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md)  
- Details: [CONTRIBUTING.md](CONTRIBUTING.md)

---

## Licence & contact

```text
Copyright (C) 2026 Zainium Dynamics
Licence: GNU General Public License v3.0 only  (see LICENSE)
Web:     https://zainiumdynamics.tech
Email:   alizain@zainiumdynamics.tech
```

**OxideUtils** and **Zainium Dynamics** are product/vendor names of Zainium Dynamics.

---

<p align="center">
  Built with Rust · Shipped under GPLv3 · Powered by Zainium Dynamics
</p>
