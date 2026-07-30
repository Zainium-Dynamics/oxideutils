# OxideUtils Roadmap — Zainium Dynamics

**Product of Zainium Dynamics** · GPLv3 only · *Not a GNU project*  
**Contact:** alizain@zainiumdynamics.tech · **Web:** https://zainiumdynamics.tech  

**Behavioural reference:** GNU binutils **2.46.1**  
(QA only, not linked)

Each phase ships reviewable binaries and tests.  
Full audit: [docs/AUDIT-REPORT-BINUTILS-2.46.1.md](docs/AUDIT-REPORT-BINUTILS-2.46.1.md)

---

## Historical phases (0.1 foundation)

| Phase | Focus | Deliverables | Status |
|-------|--------|--------------|--------|
| **1** | Foundation | Workspace, `oxideutils-core`, CI, CLI, dual `std`/`no_std` | **Done (0.1)** |
| **2** | ELF + objdump MVP | Headers, sections, symbols, hex, archives | **Partial Done** |
| **3** | Disassembly | iced-x86 x86_64; AArch64 next | **Partial** (x86/x64 live) |
| **4** | nm + size | Flag matrix, formats | **Done** (nm `-S` needs GNU align) |
| **5** | readelf deep | Notes done; versioning / compressed / SFrame / GOT | **Partial** |
| **6** | ar complete | Create/replace/delete, thin, ranlib | **Partial** (t/p/x only) |
| **7** | strip + objcopy | ELF mutation paths | **Partial** — **high risk until B** |
| **8** | addr2line | gimli DWARF, inlines, demangle | **Partial** (subset) |
| **9** | PE / Mach-O / Wasm | Deeper host parity | Planned |
| **10** | UX + parallel | JSON, colour, rayon multi-file | Planned |
| **11** | Distro + matrix | Man pages, packages, GNU differential suite | Planned |
| **12** | Hardening 1.0 | Fuzz, audit, v1.0 | Planned |

---

## Next implementation track (post-audit)

| Phase | Version target | Focus | Exit criteria |
|-------|----------------|--------|----------------|
| **A** | **0.1.1** | Correctness gate: golden tests vs 2.46.1, nm `-S` fix, strip fail-loud, docs bump | **In progress / largely done** — nm `-S`, atomic write, fail-loud, phase_abc tests |
| **B** | **0.2** | Mutation hardening: strip/objcopy ELF32+64 verify pipeline | **In progress / largely done** — ELF32+64 strip + re-parse verify |
| **C** | **0.3** | `ar` write + ranlib / symbol index | **In progress / largely done** — `rcs`/`d`/`s` + ArchiveBuilder |
| **D** | **0.4** | readelf/objdump depth: versym, GOT, SFrame awareness | **Done** (+ deep FRE walk in 0.1.3) |
| **E** | **0.5** | Multi-arch disasm + AArch64 | **Done** — bad64 AArch64; `allow_hex_fallback` for honesty |
| **F** | **0.6** | Flag parity packs (nm/strings/size/objdump/addr2line) | Scripted smoke vs GNU |
| **G** | **0.7** | PE/Mach-O real depth (or demote claims) | Feature-gated honesty |
| **H** | **0.8** | Man pages, packaging, multicall polish | Distro-ready install |
| **I** | **1.0** | Fuzz, security policy, API freeze | Audit checklist signed off |

### Immediate backlog (ordered)

1. Integration golden tests vs binutils **2.46.1**
2. strip/objcopy correctness + never silent no-op
3. nm `-S` = GNU print-size
4. `ar rcs` + ranlib
5. Docs/reference version → 2.46.1
6. readelf versym / GOT / SFrame path
7. AArch64 disasm
8. Fuzz ELF/archive/strip

### `oxide-as` / `oxide-ld` (2026-07-27 update)

No longer a blanket non-goal. A real, reduced-scope x86_64 assembler +
ELF64 linker exists and is verified end-to-end: assembled a hand-written
`.s`, linked it dynamically with `oxide-ld` against real
`/usr/lib64/libc.so` (glibc's own `GROUP()` script) +
`/lib64/ld-linux-x86-64.so.2`, and *executed* it — real `puts()`/`exit()`
calls through our own `.plt`/`.got.plt`/`.rela.plt`/`.dynamic`. Archive
`-l`/`-L` extraction verified to pull in only the member that resolves a
still-undefined symbol. Full detail + honest gap list (no lazy PLT, no
GOT-relative data imports, no TLS/versioning/IFUNC, x86_64-only, no
floating point encoding):
[docs/AUDIT-REPORT-BINUTILS-2.46.1.md](docs/AUDIT-REPORT-BINUTILS-2.46.1.md#oxide-as--oxide-ld--status-update-2026-07-27).
Full `gas`/`ld` byte-for-byte flag/behavior parity remains out of scope for
0.1.x — this is a real minimal implementation, not a clone.

### `oxide-as` / `oxide-ld` — grounded completion plan (2026-07-27)

Checked the actual `relibc-zainium` and `musl-zainium` trees (not generic
binutils parity) to ground this. Key findings that reorder priority:

- `relibc.toml`: *"the Zainix kernel's ELF loader currently only supports
  static binaries (no `PT_INTERP`/`PT_DYNAMIC` handling yet)."* Dynamic
  linking (built in Phase 7 of the earlier track) is real and verified
  against glibc, but is *ahead of* the kernel — same status as relibc's own
  `libc.so`/`ld.so` ("staged for forward compatibility"). **Static-path
  correctness is what actually runs on Zainix today.**
- `targets/x86_64-zainium-relibc.json`: `relocation-model: static`,
  `crt-static-default: true`, `tls-model: initial-exec`,
  `relro-level: full`, `has-rpath: true`. Both `-relibc` and `-linux-musl`
  target variants exist for x86_64 **and** aarch64.
- `relibc/src/{crt0,crti,crtn}` and `musl-zainium/crt/{crt1,Scrt1,crti,crtn}.c`
  are standard SysV crt objects that read linker-provided symbols
  (`__init_array_start/end`, `__bss_start`, `_end`, ...) oxide-ld doesn't
  currently synthesize — real gap, higher priority than any dynamic-linking
  depth work.

| Phase | Focus | Priority driver | Reference source |
|-------|-------|------------------|-------------------|
| **0** | Linker-provided symbols + `.init_array`/`.fini_array`/`.preinit_array` collection | **Blocking** — real crt0/crti/crtn silently misbehaves without this | `ld/ldlang.c` default-script symbol logic |
| **1** | TLS: `initial-exec` model, `PT_TLS`, `R_X86_64_TPOFF32`/`R_X86_64_GOTTPOFF` | Target spec requests it explicitly; even single-threaded static bins need it (errno is TLS) | `bfd/elf64-x86-64.c` TLS relocs |
| **2** | `oxide-as`: SSE (float ABI), real expression evaluator, `.macro`/`.if` | Needed for real compiler `.s` output, not just hand-written asm | `gas/config/tc-i386.c`, `gas/expr.c`, `gas/macro.c` |
| **3** | Full linker-script language: `MEMORY{}`, expressions, `PROVIDE`, `KEEP()` | musl/relibc builds may supply custom scripts | `ld/ldgram.y`, `ld/ldlang.c` |
| **4** | `PT_GNU_RELRO` / `-z relro` / `-z now` | Explicitly requested (`relro-level: full`) | `ld/emultempl/elf.em` |
| **5** | Real archive symbol-index use (not full member rescan) | Perf once linking a full `libc.a` | `bfd/archive.c` |
| **6** | GNU hash, lazy PLT, `-rpath` | `has-rpath: true`; deferred like relibc's own dynamic build | `bfd/elfxx-x86.c` |
| **7** | `@GOTPCREL` data-symbol imports | Only matters once dynamic linking is real kernel-side | `bfd/elf64-x86-64.c` |
| **8** | Real `.debug_line`/`.eh_frame` emission | Lower priority — relibc target is `panic-strategy: abort` | `gas/dwarf2dbg.c`, `gas/dw2gencfi.c` |
| **9** | AArch64 backend for both tools | Both `aarch64-zainium-*.json` targets exist | `gas/config/tc-aarch64.c` |
| **10** | Section groups/COMDAT | Only if/when C++ enters the picture | `bfd/elf.c` |
| **11** | `--gc-sections` | Image-size win for kernel/embedded | `ld/ldlang.c` |
| **12** | Self-hosting proof: real `x86_64-zainium-linux-musl-gcc -S` and `x86_64-zainium-relibc-gcc -S` output through `oxide-as`+`oxide-ld` (static), executed | The test that actually matters | — |

Design rule carried through every phase: **no libc-specific hardcoding** —
sysroot/env-driven (`--sysroot`/`$OXIDE_LD_SYSROOT`, already done), and
symbol-resolution-driven rather than assuming musl-vs-relibc by name.

**Phase 0 — done, verified (2026-07-27).** `oxide-ld` now maps
`.init_array`/`.fini_array`/`.preinit_array` (+ legacy `.ctors`/`.dtors`)
into the default script and synthesizes `PROVIDE`-style
`__init_array_start/end`, `__fini_array_start/end`,
`__preinit_array_start/end`, `__bss_start`, `_end`/`end`, `_edata`/`edata`
(only when the program doesn't already define them itself). Verified by
assembling a `.init_array`-registered constructor, linking statically, and
*executing* it — the constructor ran and set a global, confirmed via real
exit code. Also fixed two real bugs surfaced along the way: `oxide-as`'s
`.quad symbol_name` used to silently emit **nothing** (no bytes, no
relocation — any function-pointer table was silently empty), and plain
`.align N` was misinterpreted as power-of-two instead of x86's actual
byte-count semantics (`.align 16` was becoming a 64KiB align). Both fixed.

**Phase 1 — done, verified (2026-07-27).** `oxide-ld` computes Variant II
x86_64 TLS layout (`tpoff(sym) = block_offset - round_up(tls_size, align)`)
from `.tdata`/`.tbss` sizes, emits a real `PT_TLS` phdr, and supports
`R_X86_64_TPOFF32` (local-exec, direct constant) and `R_X86_64_GOTTPOFF`
(initial-exec, link-time-filled `.got.tls` slot — no dynamic reloc needed
at all since the whole computation is a static-link-time constant).
`oxide-as` parses `sym@tpoff`/`sym@gottpoff`. Verified for real: a test
binary calls `arch_prctl(ARCH_SET_FS, ...)` + `mmap` to set up a per-thread
block matching our computed layout, does `movq counter@gottpoff(%rip),
%rax` then a raw-byte `%fs:(%rax)` write/read-back, and exits with the
round-tripped value — confirmed on the real Linux kernel, not just
unit-tested arithmetic.

Both phases matched exactly what relibc's `tls-model: initial-exec` target
spec and musl-zainium's crt0/crti/crtn conventions need.

**Phase 2 — done, verified (2026-07-29).**
- **SSE/SSE2 float encoding**: `movss`/`movsd`/`movaps`/`movapd`/`movups`/
  `movupd`, `addss/subss/mulss/divss` (+`sd` variants), `xorps`/`xorpd`,
  `ucomiss`/`ucomisd`, `cvtsi2sd`/`cvtsi2ss`, `cvttsd2si`/`cvttss2si` — reg-reg
  and reg-mem, `%xmm0`-`%xmm15` fully REX-extended. Cross-checked
  opcode/reg-rm order against real `as`+`objdump` output before writing the
  Rust. Verified end-to-end: `cvtsi2sd` 7 and 35 into xmm regs, `addsd`,
  `cvttsd2si` back to a GPR, `exit()` — **exit code 42**, executed on the
  real kernel.
- **Symbol+offset addends**: `sym+N`/`sym-N` now work as an immediate,
  RIP-relative operand, or in `.quad` lists (`.quad table+8`) — the addend
  an ELF relocation already carries natively, so no general expression
  evaluator was needed for this very common case. Bare `.` (current
  location) also resolves in `.quad` lists. Verified end-to-end: a pointer
  stored via `.quad table+8` correctly resolved to the second element of a
  3-element table — **exit code 222**.
- **Macros + conditional assembly**: `.macro`/`.endm` (named + positional
  `\arg`, `name=default` params), `.rept`/`.endr`, `.if`/`.ifdef`/`.ifndef`/
  `.else`/`.endif` (tracking `.equ`/`.set` names for `.ifdef`). Runs as a
  text-level preprocessing pass before the real parser. Verified
  end-to-end: a macro invoked via `.rept 5`, an `.ifdef`-gated doubling
  step, and a final macro call — **exit code 110**, matching
  `((0+1×5)×2)+100` computed by the actual expanded/executed machine code.
- Documented gaps: no `.elseif`, no general arithmetic/comparison
  expressions in `.if` (bare integer or `.equ` name only), no `.altmacro`/
  `.irp`/`.irpc`, no packed/vector SSE beyond `movaps`-family and
  `xorps`/`xorpd`, no AVX, no x87.

All 39 `oxide-as` unit tests + 8 `oxide-ld` unit tests pass; full workspace
builds clean. Next up: Phase 3 (linker-script depth: `MEMORY{}`,
expressions, `PROVIDE`, `KEEP()`) — see the phase table above.

### Other non-goals until post-1.0

- BFD plugin ABI  
- gprofng / windres / dlltool (unless product demand)

---

## Current product snapshot (0.1.x)

- All listed host tools build and run (`cargo check --workspace` clean aside from minor warnings)
- Unified `make` → std tools + no_std kernel core  
- Docs suite under `docs/`  
- **Gaps:** empty integration tests; strip/objcopy experimental; ar read-only; x86-only real disasm  
- `oxide-as`/`oxide-ld`: real minimal x86_64 assembler + ELF64 linker, verified end-to-end against real glibc (dynamic linking via PLT/GOT, archive `-l` resolution) — see the 2026-07-27 update above for exact scope and gaps

## See also

- [docs/AUDIT-REPORT-BINUTILS-2.46.1.md](docs/AUDIT-REPORT-BINUTILS-2.46.1.md)  
- [docs/gnu-compatibility.md](docs/gnu-compatibility.md)  
- [docs/architecture.md](docs/architecture.md)  
