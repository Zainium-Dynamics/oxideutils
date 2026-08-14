# Alpha-Omega Application — OxideUtils (full detailed answers)

Copy each section below into the matching form field. These are the long
versions — trim if a field enforces a character limit.

---

## Country
Pakistan

## Contact Email
heyalizain@gmail.com

## Organization or Project
OxideUtils

## Is your project open source licensed?
Yes — GNU GPLv3 only.

---

## Describe the project's current state *

OxideUtils is a memory-safe Rust reimplementation of the GNU Binutils
tool surface, developed by Zainium Dynamics originally for the ZainiumOS
toolchain and released as a general-purpose, GPLv3, standalone project.
Repository: https://gitlab.com/alizain.arch/oxideutiles

**Workspace shape (14 crates, ~14,500 lines of Rust):**

- **`oxideutils-core`** (~6,400 lines) — the shared library every tool is
  built on, compiled in two modes from the same source: `std` (host tools)
  and `no_std` + `alloc` (linked directly into the ZainiumOS kernel to
  parse ELF images with no operating system underneath it at all). Internal
  layout:
  - `format/elf/` — ELF header, sections, segments, symbols, relocations,
    dynamic section, notes, SFrame unwind info, symbol versioning, GOT —
    each a separate module, each with its own parser independent of the
    others so a malformed one section can't corrupt parsing of another.
  - `format/{pe,macho,wasm,archive}/` — PE/COFF, Mach-O, WebAssembly, and
    `ar` archive parsing behind the same `format::traits` abstraction, so
    every tool gets multi-format support "for free" rather than each tool
    reimplementing format detection.
  - `disasm.rs` — x86/x86_64 disassembly via `iced-x86`, AArch64 via
    `bad64`, both wrapped behind one interface so `objdump` doesn't care
    which architecture it's looking at.
  - `archive.rs` / `archive_write.rs` — read and write paths for `ar`
    archives, kept as separate modules since read-only inspection and
    mutation have very different safety requirements (mutation can corrupt
    a file; inspection cannot).
  - `strip.rs` / `objcopy.rs` — ELF mutation logic (the highest-risk code
    in the project, since it rewrites files in place) — kept isolated so
    it can be reviewed and fuzzed as its own unit, with atomic-write
    semantics (write to a temp file, verify by re-parsing, then rename)
    so a bug here produces a loud failure, never a silently corrupted
    binary.
  - `cli/` — shared argument-parsing, multicall dispatch (`oxideutils nm
    ...` style), and TOML-based configuration (`oxideutils.toml`) so every
    tool behaves consistently instead of re-implementing flag parsing.

- **9 host inspection/transform tool crates** — thin `clap`-based binaries
  over `oxideutils-core`: `oxide-readelf`, `oxide-objdump`, `oxide-nm`,
  `oxide-size`, `oxide-ar` (also provides `oxide-ranlib`), `oxide-strings`,
  `oxide-strip`, `oxide-objcopy`, `oxide-addr2line`, plus `oxide-cxxfilt`
  and `oxide-elfedit`. All working and tested against real ELF binaries
  today (not prototypes).

- **`oxide-as`** (~3,260 lines) — a from-scratch x86_64 AT&T-syntax
  assembler: full SIB-byte addressing, SSE/SSE2 scalar float encoding,
  symbol+offset relocation addends, `.macro`/`.rept`/`.if` conditional
  assembly, TLS relocation support. Emits real ET_REL ELF64 objects.

- **`oxide-ld`** (~2,630 lines) — a from-scratch ELF64 linker: static and
  dynamic linking (`.dynsym`/`.hash`/`.plt`/`.got.plt`/`.dynamic`,
  eager-bound PLT/GOT), TLS (initial-exec model, real `PT_TLS` layout),
  crt-startup symbol synthesis (`__init_array_start`/`end`, `__bss_start`,
  `_end`), archive and `-l`/`-L`/`GROUP()`-script resolution, real section
  headers and `.symtab` in the output.

**Verification discipline — every claim below has been proven by actually
executing the produced binary, not just asserted from parsed bytes:**
- Static + dynamic linking: assembled a program with `oxide-as`, linked it
  with `oxide-ld` against real `/usr/lib64/libc.so` + the real
  `/lib64/ld-linux-x86-64.so.2`, and ran it — confirmed real `puts()`/
  `exit()` calls through our own PLT/GOT implementation.
- TLS: used `arch_prctl(ARCH_SET_FS)` + `mmap` to build a real per-thread
  block matching our computed layout, read and wrote through the
  linker-computed offset, and checked the round-tripped value via the
  process's actual exit code.
- Floating point: computed `7.0 + 35.0` through real `cvtsi2sd`/`addsd`/
  `cvttsd2si` machine code and confirmed the process exits with `42`.

**Measured, not estimated, compatibility and performance**
(`docs/COMPATIBILITY.md`, `docs/BENCHMARKS.md` in the repo — both
reproducible from the exact commands they document):
- 34.3% of the real GNU binutils 2.46 CLI flag surface covered across 9
  tools today (mechanically measured from real `--help` output, tool by
  tool, not a single hand-picked aggregate).
- Up to 13x faster and 82% less memory than GNU binutils on common
  operations (`readelf -a`, `nm`) — reported alongside a genuine known
  weakness (`objdump -d` currently uses ~7.6x more memory on large
  binaries, not hidden from this report).

---

## Describe the desired outcomes that you expect the grant to lead to. *

GNU Binutils' C parsers handle untrusted, attacker-influenceable input by
design — ELF headers, archives, relocations, DWARF debug info — on nearly
every build machine and CI runner in existence. This combination
(ubiquitous + written in C + parses hostile input) has produced a long,
recurring history of memory-corruption CVEs (buffer overflows,
out-of-bounds reads, use-after-free, integer overflow), discovered by
fuzzing and exploitable in supply-chain contexts: a malicious `.o`/`.a`
handed to `ar`, `objdump`, or a linker running unattended in CI is a real
attack, not a theoretical one.

OxideUtils removes that entire vulnerability class by construction —
memory corruption is unrepresentable in the safe-Rust code paths that make
up the overwhelming majority of this codebase. The outcomes this grant
would fund:

1. **Close the remaining GNU compatibility gap safely**, prioritized by
   real-world flag usage, so OxideUtils becomes viable as an actual
   drop-in replacement in security-conscious build environments —
   not just an internal ZainiumOS dependency.
2. **Prove the memory-safety claim adversarially**, not just by
   construction — continuous fuzzing across every format parser,
   integrated with OSS-Fuzz, seeded from both real-world and
   deliberately-malformed corpora.
3. **Independent third-party review** of the (currently small,
   concentrated) `unsafe` surface before any 1.0 / production-readiness
   claim is made publicly.
4. **Reduce ecosystem-wide supply-chain risk**, not just one vendor's:
   every tool ships under GPLv3 and is usable by any Linux distribution,
   embedded toolchain, or CI pipeline that wants a memory-safe binutils
   without adopting an entire new OS.

---

## Describe how you intend to achieve those outcomes? *

We commit to a 12-month funded plan, building directly on the
already-published, already-executing roadmap (`ROADMAP.md` in the repo —
this is not a plan starting from zero; Phases 0–2 of the assembler/linker
track above were completed and verified in the current development cycle):

**Months 1–3 — Adversarial testing infrastructure (the core security ask)**
`cargo-fuzz` targets for every format parser (ELF header/section/symbol/
relocation, archive index, DWARF, PE/Mach-O/Wasm), seeded from a corpus of
real-world binaries plus deliberately-malformed inputs; OSS-Fuzz
integration for continuous execution; a differential-testing harness
running every tool against GNU binutils 2.46 as a golden reference on the
same corpus, surfacing both correctness bugs and crashes automatically.

**Months 4–6 — Close the highest-value compatibility gaps**
Use the differential-testing data from months 1–3 to prioritize real
GNU flag/behavior parity work on `readelf`, `objdump`, and `objcopy`
(currently the lowest-coverage tools per `docs/COMPATIBILITY.md`), plus
`PT_GNU_RELRO`/`-z now` and real GNU-hash support in `oxide-ld`.

**Months 7–9 — Assembler/linker depth**
Linker-script expression language (`MEMORY{}`, `PROVIDE`, `KEEP()`),
lazy PLT binding, and an AArch64 backend for both `oxide-as` and
`oxide-ld` (multi-architecture support, not x86_64-only).

**Months 10–12 — Independent security audit and hardening**
A third-party audit of the full `unsafe` surface area, remediation of any
findings, a published audit report, and a signed-off 1.0 hardening
checklist (fuzz-clean corpus, no known crashes, documented threat model)
before any production-readiness claim.

Every milestone in this plan ends with the same verification standard
already used throughout this project: prove it by executing the produced
output, not by asserting correctness from parsed bytes alone. Progress
against each phase is tracked publicly in `ROADMAP.md` as it lands, the
same way the assembler/linker phases already completed were documented.
