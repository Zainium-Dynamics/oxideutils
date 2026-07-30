# Changelog

All notable changes to **OxideUtils** (Zainium Dynamics) are documented here.

## [0.1.5] — 2026-07-15 (TOML-only build — no Makefile)

### Build system
- **Makefile removed** — configure with **`oxideutils.toml`**, then `cargo build --release`
- Shared **`build/oxide_build.rs`** read by every crate’s `build.rs`
- New sections: `[build]`, `[tools]`, `[features]` (`true`/`false`)
- `standalone` / `static` / `dynamic` / per-tool enable
- Writes **`target/oxideutils-build-plan.txt`** after build
- Disabled tools exit with a clear hint (enable TOML or use multicall)
- Docs: [docs/building.md](docs/building.md), configuration.md, README

## [0.1.4] — 2026-07-15 (Beginner-friendly visual help)

### Help UX
- Shared `cli::help` module with **box titles**, **flag tables**, **BEGINNER START**, **EXAMPLES**
- Coloured clap styles (cyan headers, yellow options)
- All tools: `oxide-objdump`, `readelf`, `nm`, `ar`, `strip`, `objcopy`, `size`, `strings`, `addr2line`
- Multicall `oxideutils --help` cheat-sheet of every tool
- Clearer option one-liners; friendlier “nothing to do” / missing-args messages

## [0.1.3] — 2026-07-15 (Phase E + deep SFrame)

### Phase E — multi-arch disasm
- **AArch64** disassembly via **bad64** (pure Rust, `no_std` decode)
- Feature `disasm-aarch64` (on by default with host `std`; **not** in `kernel` profile — avoids bindgen)
- `has_real_backend()` + `DisasmOptions::allow_hex_fallback`
- Tests: AArch64 `nop` decode

### SFrame FRE walk (deepens Phase D)
- Parse **FDE table** (v2 entry + v3 index/attr)
- Walk **Frame Row Entries** (ADDR1/2/4, data words, RA undefined)
- Per-function dump: start PC, size, CFA/FP/RA (AMD64 / AArch64 heuristics)
- Synthetic v2 unit test

## [0.1.2] — 2026-07-15 (Phase D — readelf/objdump depth)

### Phase D
- **readelf `-V` / `--version-info`**: versym, verneed, verdef
- **readelf `--got-contents`**: GOT / `.got.plt` dump (binutils 2.46)
- **readelf `--sframe[=SEC]`**: SFrame v2/v3 **header** summary (no full FRE walk yet)
- **readelf `-u` / `--unwind`**: `.eh_frame` / `.eh_frame_hdr` summary
- **readelf `-r`**: pretty relocs with x86_64/AArch64 type names + symbol names
- **readelf `-d`**: resolve `NEEDED` / `SONAME` / `RPATH` strings
- **readelf `-S`**: list compressed sections
- **objdump `-r`**: reloc pretty-print (kind + symbol target + addend)
- **objdump `--sframe`**: SFrame dump for ELF
- **objdump `-x`**: also dynamic / notes / version info on ELF
- Docs: README, tools, api-core, architecture, gnu-compatibility, man page, CHANGELOG

## [0.1.1] — 2026-07-15 (Phases A–C start)

### Phase A — Correctness
- Reference version docs: **GNU binutils 2.46.1**
- **nm `-S`**: GNU **print-size** (was size-sort); size-sort is `--size-sort`
- strip/objcopy: **atomic write** + mode preserve
- strip: fail-loud on truncated/unsupported ELF; verify re-parse after strip
- Integration helpers + core regression tests (`phase_abc`)

### Phase B — Mutation
- strip: unified **ELF32 + ELF64** section drop / repack
- `sh_link` / `sh_info` remap for REL/RELA/GROUP
- never silent no-op on truncated ELF

### Phase C — Archives
- `oxide-ar rcs` create with GNU symbol index
- `d` delete, `q` append, `s` ranlib-style, `t/p/x` retained
- `ArchiveBuilder` in `oxideutils-core::archive_write`

## [0.1.0] — 2026

### Branding

- Product of **Zainium Dynamics** (not a GNU project)
- Homepage: https://zainiumdynamics.tech  
- Contact: alizain@zainiumdynamics.tech  
- Licence: **GPLv3 only**

### Dual mode

- `oxideutils-core` supports **`std`** (host) and **`no_std` + `alloc`** (kernel)
- Unified build: `make` / `scripts/build-all.sh` → `target/` + `target-nostd/`

### Tools

- **oxide-objdump** — headers, symbols, hex, archives, x86/x64 disasm (iced-x86)
- **oxide-nm** — symbol listing and filters
- **oxide-readelf** — ELF `-h/-S/-l/-s/-d/-r/-n/-a`
- **oxide-size** — Berkeley / SysV
- **oxide-strings** — printable strings
- **oxide-ar** — `t` / `p` / `x`
- **oxide-strip** — ELF strip-all / debug / unneeded
- **oxide-objcopy** — copy, strip, sections, `-O binary`
- **oxide-addr2line** — DWARF via addr2line/gimli
- **oxideutils** — multicall dispatcher

### Documentation

- Full README + `docs/` suite (architecture, tools, API, kernel, building, FAQ, man stub)

### Infrastructure

- CI workflows (fmt, clippy, test, unified build)
- Makefile + cargo aliases
