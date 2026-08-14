# OpenSSF Alpha-Omega Funding Proposal — OxideUtils

**Applicant:** Zainium Dynamics
**Project:** OxideUtils — memory-safe binutils suite in Rust
**Repository:** https://gitlab.com/alizain.arch/oxideutiles
**Licence:** GPLv3 only
**Date:** 2026-07-30

---

## 1. Problem statement

GNU Binutils — `readelf`, `nm`, `objdump`, `objcopy`, `strip`, `ar`, `as`,
`ld`, and the shared `libbfd`/`libopcodes` C libraries underneath them —
sits on nearly every build machine, CI runner, and embedded toolchain in
existence. It is also written in C and parses **untrusted, attacker-
influenceable input by design**: object files, archives, ELF headers,
relocations, DWARF debug info. This combination (ubiquitous + C + hostile-
input parsing) has produced a long, recurring history of CVEs in binutils
— buffer overflows, out-of-bounds reads, heap corruption, integer overflow
— discovered by fuzzing and exploited in supply-chain contexts (a
malicious `.o`/`.a` file handed to `ar`, `objdump`, or a linker running in
CI is a realistic attack surface).

OxideUtils removes that entire vulnerability class by construction: it is
a from-scratch Rust reimplementation of the same tool surface. Memory
corruption bugs (the majority of historical binutils CVEs) are not
merely "less likely" — they are unrepresentable in safe Rust for the code
paths that don't use `unsafe`.

## 2. What exists today (not a proposal for future work — this is built and verified)

| Component | Status |
|---|---|
| `oxide-readelf`, `oxide-nm`, `oxide-objdump`, `oxide-size`, `oxide-ar`, `oxide-strings`, `oxide-strip`, `oxide-objcopy`, `oxide-addr2line` | Working, tested against real ELF binaries |
| `oxide-as` (x86_64 assembler) | AT&T syntax, SSE/SSE2 float, SIB addressing, macros/conditional assembly — verified by assembling and *executing* real machine code |
| `oxide-ld` (ELF64 linker) | Static + dynamic linking (PLT/GOT, TLS), archive/`-l`/`-L` resolution — verified end-to-end against real glibc (assembled → linked → executed, correct exit codes) |
| `oxideutils-core` | Shared `std` + `no_std` library, used directly by the ZainiumOS kernel to parse ELF without `std` at all |

Every claim of "working" in this project has a corresponding execution
test — not just unit tests on parsing logic, but the produced binary
actually run and its behavior checked (see `ROADMAP.md` phase log for
the specific verification of each capability, e.g. TLS verified via a
real `arch_prctl`+`mmap` thread-local read/write round-trip).

Measured (not claimed) compatibility and performance:
- **34.3%** of the real GNU binutils CLI flag surface covered across 9
  tools today, reproducibly measured — [docs/COMPATIBILITY.md](COMPATIBILITY.md)
- Up to **13x faster** and **82% less memory** than GNU binutils on
  common operations, reported alongside a genuine known weakness
  (`objdump -d` currently uses more memory, not hidden) —
  [docs/BENCHMARKS.md](BENCHMARKS.md)

## 3. What the grant funds

1. **Fuzzing infrastructure** — `cargo-fuzz` targets for every format
   parser (ELF header/section/symbol/relocation, archive index, DWARF)
   seeded from a corpus of real-world and adversarially-malformed
   binaries, run continuously (OSS-Fuzz integration).
2. **GNU behavioral parity** — systematic differential testing against
   GNU binutils 2.46 as a golden reference to close the compatibility
   gap tracked in [docs/COMPATIBILITY.md](COMPATIBILITY.md), prioritized
   by real-world flag usage frequency.
3. **Assembler/linker hardening** — completing `oxide-as`/`oxide-ld`
   toward safe drop-in status: linker-script expression depth, GNU hash,
   lazy PLT binding, `PT_GNU_RELRO`, AArch64 backend (full phase list in
   `ROADMAP.md`).
4. **Independent security review** — a third-party audit of the `unsafe`
   surface area (currently minimal, concentrated in a few
   performance-sensitive parsing paths) before any 1.0 claim.

## 4. Why this matters to the ecosystem, not just one vendor

OxideUtils is developed for ZainiumOS's toolchain, but every tool ships
under GPLv3 and is a general-purpose binutils replacement usable by any
Linux distribution, embedded toolchain, or CI pipeline wanting to remove
C-parser memory-corruption risk from their build supply chain — the exact
threat model Alpha-Omega exists to reduce at ecosystem scale.

## 5. Team & sustainability

Zainium Dynamics maintains this as part of its OS toolchain effort;
funding converts an internal dependency into a properly resourced,
independently auditable public-good replacement for a component nearly
every other piece of software supply chain relies on transitively.
