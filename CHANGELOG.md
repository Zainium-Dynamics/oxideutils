# Changelog

All notable changes to OxideUtils are documented here. Format loosely
follows [Keep a Changelog](https://keepachangelog.com/). Behavioural
reference is GNU binutils **2.46.1** (QA comparison only — no GNU code
is linked; OxideUtils is not a GNU project).

## [0.1.0-alpha] - 2026-08-13

Initial public release, tagged as an **alpha**: a working Rust
binutils-suite foundation, not a drop-in GNU replacement yet — see
"Known gaps" below.

### Added

- Workspace with 14 crates: `oxide-objdump`, `oxide-nm`, `oxide-readelf`,
  `oxide-size`, `oxide-strings`, `oxide-ar`, `oxide-strip`,
  `oxide-objcopy`, `oxide-addr2line`, `oxide-cxxfilt`, `oxide-elfedit`,
  and `oxideutils-core`, plus a real (scoped) `oxide-as` / `oxide-ld`
  x86_64 assembler and ELF64 linker.
- Dual `std` / `no_std` `oxideutils-core`, usable directly from a
  `no_std` kernel context (`alloc` + `disasm` + `kernel` feature set).
- x86/x86_64 disassembly via `iced-x86`; early AArch64 decode via `bad64`
  (falls back to hex when real decode isn't available, and says so,
  rather than guessing).
- `oxide-as` / `oxide-ld` verified end-to-end against real glibc:
  assembled, statically/dynamically linked, and *executed* real
  binaries — including `.init_array`/`.fini_array` symbol synthesis,
  initial-exec TLS (`PT_TLS`, `R_X86_64_TPOFF32`/`R_X86_64_GOTTPOFF`),
  SSE/SSE2 float encoding, `sym+N` addends, and a `.macro`/`.rept`/`.if`
  preprocessor.
- `oxide-ar` archive create/list/delete round-trip (`t`/`p`/`x` read
  paths plus `rcs`-style write path and symbol index).
- GitLab CI: build + test + clippy/fmt gate, fuzz stage scaffold,
  benchmark stage, and a tag-triggered release job that publishes a
  checksummed workspace source tarball.
- Docs suite: `ROADMAP.md`, `docs/AUDIT-REPORT-BINUTILS-2.46.1.md`,
  `docs/gnu-compatibility.md`, `docs/architecture.md`,
  `docs/kernel-integration.md`, `docs/release-process.md`.

### Known gaps (by design — do not treat this as GNU-drop-in-ready)

- `strip`/`objcopy`: ELF32 strip can silently no-op instead of failing
  loud; ELF64 strip is a hand-rolled rewrite without a full validation
  pass. Do not use as a default system `strip`/`objcopy` yet.
- Disassembly: only x86_64 has full real-decode confidence; other
  architectures may fall back to hex.
- `readelf`: notes are complete; versym, compressed-section detail,
  SFrame, and GOT depth are partial.
- PE / Mach-O / Wasm support is stub/summary-level only — do not rely on
  it for real parity.
- `oxide-as`/`oxide-ld`: x86_64-only; no lazy PLT, no GOT-relative data
  imports, no full TLS/IFUNC matrix, no AVX/x87, no general expression
  evaluator in `.if`.
- Integration/golden-test coverage against a real binutils 2.46.1 tree
  is still thin.

See [ROADMAP.md](ROADMAP.md) and
[docs/AUDIT-REPORT-BINUTILS-2.46.1.md](docs/AUDIT-REPORT-BINUTILS-2.46.1.md)
for the full gap list and phased plan toward 1.0.
