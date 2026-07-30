# Case Study: Rewriting a Security-Critical C Toolchain in Rust — OxideUtils

## Background

GNU Binutils has accumulated dozens of CVEs over its lifetime, almost all
memory-safety bugs (heap/stack buffer overflows, out-of-bounds reads,
use-after-free) triggered by malformed ELF, archive, or debug-info input.
Because binutils tools run on build machines and inside CI pipelines —
often against untrusted or third-party input (a downloaded `.a`, a build
artifact from an external contributor, a fuzzing corpus) — this is a real
supply-chain attack surface, not a theoretical one.

OxideUtils is a ground-up Rust reimplementation of the binutils tool
surface (`readelf`, `nm`, `objdump`, `size`, `ar`, `strings`, `strip`,
`objcopy`, `addr2line`) plus a new x86_64 assembler (`oxide-as`) and ELF64
linker (`oxide-ld`), built for the ZainiumOS project and released as a
general-purpose GPLv3 replacement.

## What we found doing this

Rewriting a mature C tool surface in Rust from scratch surfaces exactly
the class of bug Alpha-Omega funds discovery of — except *before* it ships,
because the type system rejects it at compile time instead of needing a
fuzzer to find it at runtime. Concretely, during development:

- Every relocation-application path, section-merge routine, and ELF
  header parser in `oxide-ld`/`oxide-as` is bounds-checked by construction
  (Rust slice indexing panics rather than reading adjacent memory) —
  the exact bug class (`bfd/elfxx-x86.c`-style out-of-bounds relocation
  application) that has produced real binutils CVEs historically.
- We caught and fixed *our own* real bugs this way during development —
  e.g. a mutual-recursion stack overflow in the assembler's operand
  parser, and a silent-corruption bug where unrecognized instructions
  used to emit a NOP instead of erroring — both found because Rust's
  strictness (and our own verification discipline) made the wrong
  behavior visible immediately, not after a downstream user hit it.

## Verification discipline

A rewrite's memory-safety claim is only as credible as its correctness
testing. Every non-trivial capability in this project is verified by
**actually executing the produced output**, not just asserting on parsed
bytes:

- Static and dynamic linking verified by assembling a real program,
  linking it with `oxide-ld` against real `glibc` (`/usr/lib64/libc.so`,
  `/lib64/ld-linux-x86-64.so.2`), and running the resulting binary —
  checking real `puts()`/`exit()` behavior through our own from-scratch
  PLT/GOT/`.dynamic` implementation.
- TLS (thread-local storage, initial-exec model) verified by using
  `arch_prctl(ARCH_SET_FS)` to set up a real per-thread block matching
  our computed layout, then reading and writing through the
  linker-computed offset and checking the round-tripped value via the
  process's real exit code.
- Floating point (SSE/SSE2) codegen verified by computing `7.0 + 35.0`
  through real `cvtsi2sd`/`addsd`/`cvttsd2si` machine code and checking
  the process exits with `42`.

This "prove it by running it" standard — documented per-phase in
`ROADMAP.md` — is what should give reviewers confidence that "memory-safe"
here means something more than "written in Rust."

## Honest limitations (what we are *not* claiming)

- CLI flag-surface compatibility with GNU binutils is **34.3%** today,
  measured mechanically against real `--help` output — not "mostly
  compatible," a specific number, tool-by-tool, reproducible
  (`docs/COMPATIBILITY.md`).
- Performance is not a uniform win: `oxide-objdump -d` currently uses
  ~7.6x more memory than GNU `objdump` on a large binary — reported
  alongside the wins (13x faster `readelf -a`, 82% less memory on `nm`)
  because a security-funding review deserves the unflattering numbers
  too (`docs/BENCHMARKS.md`).
- No fuzzing infrastructure exists yet — this is the primary gap this
  proposal asks Alpha-Omega to help close.

## Why this is the right shape for Alpha-Omega funding

The highest-leverage security work left is not more features — it's
**adversarial testing** (fuzzing untrusted input across every parser) and
**closing the remaining compatibility gap safely** so this can realistically
displace GNU binutils in security-conscious build environments, not just
ZainiumOS's own toolchain.
