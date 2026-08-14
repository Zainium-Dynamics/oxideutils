# OxideUtils — Architecture Overview

> **Product of [Zainium Dynamics](https://zainiumdynamics.tech)** · GPLv3

This document provides a quick architectural overview for new
contributors. For the full deep-dive, see
[docs/architecture.md](docs/architecture.md).

## Workspace layout

```
oxideutils/
├── crates/
│   ├── oxideutils-core/      ← shared library (std + no_std)
│   ├── oxide-objdump/        ← binary: section/symbol/disasm viewer
│   ├── oxide-nm/             ← binary: symbol lister
│   ├── oxide-readelf/        ← binary: ELF structure dump
│   ├── oxide-size/           ← binary: section size summary
│   ├── oxide-strings/        ← binary: printable string finder
│   ├── oxide-ar/             ← binary: archive create/list/extract
│   ├── oxide-strip/          ← binary: symbol/debug stripper
│   ├── oxide-objcopy/        ← binary: object copy/transform
│   ├── oxide-addr2line/      ← binary: DWARF address lookup
│   ├── oxide-cxxfilt/        ← binary: C++/Rust demangler
│   ├── oxide-elfedit/        ← binary: ELF header editor
│   ├── oxide-as/             ← binary: x86_64 assembler (subset)
│   └── oxide-ld/             ← binary: x86_64 ELF64 linker (subset)
├── build/oxide_build.rs      ← shared build.rs logic (reads TOML config)
├── scripts/build-all.sh      ← optional build wrapper
├── tests/integration/        ← integration test suite
└── docs/                     ← full documentation
```

## Design principles

1. **Thin binaries, fat core** — tool crates are CLI wrappers; all
   parsing, inspection, and mutation logic lives in `oxideutils-core`.

2. **Dual runtime** — `oxideutils-core` compiles under both `std` (host
   tools) and `no_std + alloc` (Zainium kernel). Feature gates:
   `default = ["std"]`, kernel builds use `--no-default-features
   --features alloc,disasm,kernel`.

3. **TOML-driven builds** — `oxideutils.toml` controls which tools are
   built, link style (static/dynamic), feature toggles, and runtime
   defaults. No Makefile; `build/oxide_build.rs` reads the TOML at
   compile time and emits `rustc-cfg` flags.

4. **GNU-compatible, not GNU** — CLI flags and exit codes match GNU
   binutils 2.46.1 where practical, but branding is always Zainium
   Dynamics. See [docs/gnu-compatibility.md](docs/gnu-compatibility.md).

## Data flow (typical tool)

```
CLI args (clap)
   → oxideutils-core::format::object::OxideObject::parse_file(path)
   → format-specific parser (ELF / PE / Mach-O / Wasm / Archive)
   → inspection / mutation API
   → formatted output (text / JSON)
```

## CI pipeline

GitHub Actions (`.github/workflows/ci.yml`):

| Stage        | Trigger              | What it does                          |
|--------------|----------------------|---------------------------------------|
| **test**     | push / PR            | fmt + clippy + `cargo test`           |
| **build**    | push / PR            | workspace release + `no_std` kernel   |
| **fuzz**     | weekly / manual      | `cargo-fuzz` on all targets           |
| **bench**    | weekly / manual      | smoke benchmark vs GNU                |
| **release**  | tag `v*`             | source tarball + GitHub Release       |
