# OxideUtils Technical Audit Report

**Role lens:** Senior Rust engineer · GNU binutils principal architecture · GNU toolchain developer  
**Product:** OxideUtils 0.1.0 (Zainium Dynamics)  
**Reference tree:** `/run/media/alizain/ZAINIUM_DRIVE/packages/binutils-2.46.1` (GNU binutils **2.46.1**)  
**Audit date:** 2026-07-15  
**Scope:** All workspace crates, architecture, risk, GNU comparison, next implementation roadmap  

> OxideUtils is **not** a GNU project. Comparison is behavioural/capability only.  
> No libbfd / libopcodes / gas / ld code is linked.

---

## 1. Executive summary

| Dimension | Verdict |
|-----------|---------|
| **Maturity** | Early **0.1 MVP** — solid workspace skeleton, real host tools, dual `std`/`no_std` core |
| **vs binutils 2.46.1** | Covers a **small slice** of the binutils *userland inspect/transform* tools, plus a minimal (not full-parity) `oxide-as`/`oxide-ld` verified working end-to-end against real glibc — see §8 below for exact scope; still **no** BFD, opcodes matrix, gprof, CTF, plugins |
| **Code scale** | ~**5.7k** Rust LOC total vs GNU **readelf alone ~25k**, **objdump ~6.4k**, **BFD ~558k**, **opcodes ~632k**, **gas ~421k**, **ld ~65k** |
| **Build health** | `cargo check --workspace` **passes** (2 minor unused-import warnings) |
| **Test health** | **Critical gap** — almost no golden/integration tests; empty `tests/integration/{nm,objdump,readelf}/` |
| **Safety** | No project `unsafe` found in crates; risk is **semantic/correctness** (strip/objcopy ELF mutation), not memory corruption in OxideUtils itself |
| **Production readiness as GNU drop-in** | **No** — useful developer inspect tools today; **unsafe as default `strip`/`objcopy` for release pipelines** until hardened |
| **Kernel path** | Architecturally sound (`alloc` + `disasm` + `kernel`); needs consumer validation |

**Bottom line:** OxideUtils is a credible **Rust reimplementation *strategy*** for classic *read-mostly* binutils utilities, with good branding, dual-mode core, and a thin multicall layout. Against binutils **2.46.1** it is still a **compatibility subset** (~10–20% of everyday CLI surface for the mirrored tools; **≪5%** of the full binutils *project*). Next work must prioritise **correctness tests**, **mutation safety**, **ar write path**, and **reference bump 2.42 → 2.46.1**.

---

## 2. Inventory — all crates

| Crate | Kind | ~LOC | Role | Status (honest) |
|-------|------|------|------|-----------------|
| `oxideutils-core` | lib + multicall bin | ~4.2k | Shared parse/symbols/disasm/strip/objcopy/CLI | **Core of product** — API 0.1, uneven depth |
| `oxide-objdump` | bin | ~460 | Headers, hex, symbols, disasm, archives | **Strongest tool** — x86/x64 iced-x86 |
| `oxide-nm` | bin | ~150 | Symbol list/filters | **Working** — flag semantics diverge on `-S` |
| `oxide-readelf` | bin | ~122 | ELF structure dump | **Working subset** — far from GNU depth |
| `oxide-size` | bin | ~124 | Berkeley / SysV sizes | **Working** for common ELF |
| `oxide-strings` | bin | ~82 | Printable scan | **Working minimal** |
| `oxide-ar` | bin | ~123 | Archive t/p/x | **Read-only subset** — no create/delete/ranlib |
| `oxide-strip` | bin | ~134 | Strip symbols/debug | **High risk** mutation path |
| `oxide-objcopy` | bin | ~118 | Copy/filter/`-O binary` | **High risk** / incomplete targets |
| `oxide-addr2line` | bin | ~124 | DWARF line maps | **Working** via `addr2line`/`gimli` (std) |

**Workspace:** resolver 2, edition 2024, MSRV 1.85, licence **GPL-3.0-only**, version **0.1.0**.

### Core module map

| Module | `no_std` | Depth |
|--------|----------|-------|
| `error` | yes | Good exit-code model |
| `format::object` | yes | Good façade over `object` |
| `format::elf/*` | yes | Headers/sections/segments/dyn/relocs/notes — **read-path OK** |
| `format::pe` / `macho` | std | **Stubs / summary only** |
| `format::wasm` | yes | Magic/version only |
| `symbols` | yes | nm-style codes |
| `archive` | yes | Parse + member slices only |
| `disasm` | yes* | iced-x86 for i386/x86_64; else hex |
| `strip` / `objcopy` | std | Custom ELF64 rebuild; ELF32 weak |
| `addr2line_util` | std | Loader-based DWARF |
| `cli::config` | std | TOML config (large for stage) |

\* with `alloc` + `disasm`.

---

## 3. Architecture assessment (principal view)

### What is well designed

1. **BFD-like façade without BFD** — `OxideObject` / `SectionView` over the `object` crate is the right Rust approach; avoids C ABI and decades of target-specific BFD debt.
2. **Thin binaries, fat core** — mirrors GNU split (`bucomm` + tool mains) without duplicating parsers.
3. **Dual runtime** — separate `target` vs `target-nostd` avoids Cargo feature unification poisoning kernel builds. Correct and documented.
4. **Error model** — path-as-label strings work for kernel; exit codes 0/1/2 documented GNU-like.
5. **Disasm boundary** — iced-x86 (gas style) instead of inventing decoders; hex fallback is honest.
6. **No accidental GPL linking of GNU objects** — clean-room behavioural reference policy is sound.

### Architectural debts

| Debt | Impact |
|------|--------|
| Docs target **binutils 2.42**; reference tree is **2.46.1** | Drift on SFrame v3, GOT dump, plugin/target strictness, etc. |
| PE/Mach-O modules mostly **placeholders** | Feature matrix overclaims “PE/COFF, Mach-O” depth |
| Custom **hand-rolled ELF64 strip** instead of a single well-tested writer pipeline | Hardest correctness risk in the tree |
| **Empty integration harness** | No regression gate vs GNU |
| Multicall **re-execs** `oxide-*` on PATH | Fine for 0.1; not true busybox-style single binary |
| `capstone` listed in workspace deps but **unused** (iced-x86 chosen) | Cleanup / decision needed for multi-arch |
| Heavy `cli/config.rs` (~518 LOC) early | Config before parity is inverted priority |

### Conceptual map vs GNU

| GNU subsystem | OxideUtils analogue | Parity |
|---------------|---------------------|--------|
| `binutils/*.c` tools | `oxide-*` crates | Partial tool set |
| BFD | `object` + `goblin` + `format::*` | Partial formats |
| `opcodes` / libopcodes | `disasm` + iced-x86 | x86 only |
| libiberty demangle | `rustc-demangle` + `cpp_demangle` | Host only |
| DWARF in `dwarf.c` / addr2line | `gimli` / `addr2line` | Good for line maps |
| gas | — | **Out of scope** |
| ld | — | **Out of scope** |
| libctf | — | Missing |
| libsframe | — | Missing (2.46 emphasis) |
| gprof / gprofng | — | Missing |
| elfedit, cxxfilt, windres, dlltool… | — | Missing |

---

## 4. Comparison matrix — OxideUtils 0.1 vs binutils 2.46.1

### 4.1 Project surface

| Area | GNU 2.46.1 | OxideUtils 0.1 |
|------|------------|----------------|
| Assembler (`gas`) | Full multi-target | **None** |
| Linker (`ld` / gold-era code still present) | Full | **None** |
| Object library (BFD) | ~300 C files | Rust crates |
| Disassembler backends | Huge arch matrix | **x86/x86_64** + hex |
| Userland tools | 20+ (ar, nm, objdump, readelf, strip, objcopy, strings, size, addr2line, elfedit, cxxfilt, …) | **9** mirrors + multicall |
| Profiling | gprof, gprofng | **None** |
| CTF / SFrame | First-class in 2.45–2.46 | **None** |
| Plugins (LTO, etc.) | BFD plugins | **None** |
| Test suite | Massive DejaGnu trees | **~1 smoke test** |

### 4.2 Tool-by-tool CLI / capability

Legend: **Y** = usable everyday · **P** = partial · **N** = missing/unsafe-for-prod · **—** = N/A

| Capability | GNU 2.46.1 | Oxide | Risk if used as drop-in |
|------------|------------|-------|-------------------------|
| **objdump** `-h -f -t -s -d/-D` | Y | **Y** (x86) | Medium — format/arch gaps |
| objdump multi-arch disasm | Y | **P** hex fallback | High for ARM/RISC-V scripts |
| objdump CTF / SFrame / DWARF dump | Y (2.46 SFrame v3) | **N** | High for modern toolchains |
| objdump `--disassemble=SYM`, ranges | Y | **P** | Medium |
| **nm** filters `-n -u -g -C` | Y | **Y** | Low–medium |
| nm `-S` = **print size** (GNU) | Y | **Incompatible** (`-S` = size-sort; print-size is `-s`) | **High for scripts** |
| **readelf** `-a` deep (versym, unwind, GOT 2.46 `--got-contents`, …) | Y | **P** basic set | High |
| **size** Berkeley/SysV | Y | **Y** | Low |
| **strings** encodings, sections, filenames | Y | **P** whole-file ASCII | Medium |
| **ar** `t p x` | Y | **Y** | Low |
| ar `q r c d m` + thin + symbol index | Y | **N** | Blocks static lib workflows |
| **ranlib** | Y | **N** | Same |
| **strip** exec/so/reloc | Y | **P** ELF64 custom; ELF32 weak | **Critical** |
| **objcopy** targets, redefine, wildcard, gap-fill… | Y (~146 long opts) | **P** (~9 long opts) | **Critical** |
| **addr2line** `-f -C -i -p -a` | Y | **Y** | Low–medium (debuglink, split DWARF) |
| **cxxfilt** | Y | **N** (demangle only as flags) | Low |
| **elfedit** | Y | **N** | Medium for packaging |
| PE/Mach-O deep dump | Y | **P** summary stubs | Medium |

### 4.3 Scale comparison (order of magnitude)

| Artifact | LOC (approx) |
|----------|----------------|
| OxideUtils all Rust | **~5.7k** |
| GNU `addr2line.c` | ~0.6k |
| GNU `size.c` | ~0.7k |
| GNU `strings.c` | ~1.4k |
| GNU `ar.c` | ~1.7k |
| GNU `nm.c` | ~2.2k |
| GNU `objcopy.c` | ~6.3k |
| GNU `objdump.c` | ~6.4k |
| GNU `dwarf.c` (shared) | ~13k |
| GNU `readelf.c` | **~25k** |
| GNU BFD | **~558k** |
| GNU opcodes | **~632k** |

Oxide “wins” on size by **narrowing scope** and **reusing crates**, not by matching BFD.

### 4.4 2.46.1 features Oxide must track (docs still say 2.42)

From GNU NEWS / tree:

1. **SFrame v3** dump in objdump/readelf; RA-undefined FRE representation  
2. **readelf `--got-contents`**  
3. Stricter **objcopy `--target=`** vs `--output-target=` behaviour  
4. **libsframe.so.3** / versioned API  
5. NaCl removed (irrelevant)  
6. Ongoing CTF / plugin strictness  

**Action:** rebase compatibility policy and golden tests to **2.46.1** (matches local packages + system tools on audit host: `GNU readelf 2.46-3.fc44`).

---

## 5. Per-crate risk register

Severity: **C** critical · **H** high · **M** medium · **L** low

| ID | Crate / area | Sev | Finding | Consequence | Mitigation |
|----|--------------|-----|---------|-------------|------------|
| R1 | `strip` ELF64 | **C** | Hand-rolled SHDR rewrite; non-alloc repack; `sh_link` remap incomplete for some types; ALLOC offsets assumed valid; no full validation pass | Corrupt shared objects / silent runtime breakage | Golden strip tests; prefer `object::write` / proven ELF mutator; never default install over `/usr/bin/strip` |
| R2 | `strip` ELF32 | **C** | Falls back to relocatable writer or **returns original unchanged** | False sense of “stripped” binaries | Explicit error if cannot strip; implement real ELF32 path |
| R3 | `objcopy` | **C** | Subset only; `-I` ignored; limited `-O`; section filter rebuild fragile | Broken firmware images / wrong extract | Restrict documented support matrix; integration tests vs GNU |
| R4 | Tests | **C** | Integration dirs empty; one optional `/bin/ls` parse test | Regressions ship unnoticed | Phase A: GNU differential suite (below) |
| R5 | `oxide-nm` `-S` | **H** | GNU: print size; Oxide: size-sort | Silent script breakage | Align to GNU **or** hard error + docs; add alias for size-sort under long-only |
| R6 | `oxide-ar` | **H** | No create/replace/delete/index | Cannot replace `ar`/`ranlib` in build systems | Implement GNU letter set + symbol index |
| R7 | Disasm arch | **H** | Only x86/x86_64 real disasm | AArch64/RISC-V “looks like disasm” but is hex | Capstone/iced multi-arch or refuse `-d` with clear error |
| R8 | readelf depth | **H** | No versym, unwind, GOT, CTF, SFrame, compressed section detail parity | Kernel/debug workflows incomplete | Prioritise notes/dynamic/versym/SFrame |
| R9 | PE/Mach-O | **M** | Stubs / summary | Marketing overclaim | Gate behind feature flags; document “experimental” |
| R10 | strings | **M** | ASCII only; no encoding / data-section modes | Misses UTF-16 etc. | Match GNU `-e` encodings |
| R11 | addr2line | **M** | Depends on debug in binary; split DWARF / debuginfod not first-class | `??:?` more often than expected | debuglink + `.dwo` + optional debuginfod later |
| R12 | Config surface | **L** | TOML complexity early | Distraction | Freeze config schema until parity milestones |
| R13 | Dependency supply chain | **M** | `object`, `goblin`, `iced-x86`, `gimli` own parsing complexity | Vulnerabilities / panics on malicious objects | Fuzzing (Phase 12); size limits; `catch_unwind` at CLI boundary optional |
| R14 | Reference version drift | **M** | Docs 2.42 vs tree 2.46.1 | Wrong golden expectations | Bump all docs to 2.46.1 |
| R15 | In-place strip write | **M** | temp+rename path exists but strip_file uses direct write in core | Partial write on crash can trash output | Atomic replace always; preserve mode/owner |

### Risk heatmap (summary)

```text
                 Likelihood →
              Low        Med         High
Impact High |          | R7,R8     | R1,R2,R3,R4
      Med   | R12      | R9-R11,R13| R5,R6,R14
      Low   |          | R15       |
```

---

## 6. Safety & quality notes (Rust senior view)

| Check | Result |
|-------|--------|
| Project-local `unsafe` | **None found** in `crates/**/*.rs` |
| `forbid(unsafe_op_in_unsafe_fn)` | Present on core |
| Panic policy | Release profile `panic = "abort"` — good for small tools; ensure CLI maps parse errors → exit, not panic |
| Clippy/fmt | Documented; not fully enforced by this audit run |
| Mutation tools | **Highest residual risk** despite memory safety — **logic bugs corrupt objects** |
| Malicious input | Parser crates may panic/OOM; no fuzz harness yet |
| Licence | GPLv3-only consistent; clean-room vs GNU sources OK if no copied non-trivial C |

---

## 7. What OxideUtils is *good for* today

**Appropriate uses (0.1):**

- Host inspection of ELF (headers, symbols, basic dynamic/relocs/notes)
- x86_64 disassembly exploration
- size / strings / nm in developer workflows (with `-S` caveat)
- DWARF addr2line when full debug info is present
- Kernel/module **read-only** parse via `no_std` core
- Teaching / product foundation for Zainium toolchain story

**Not ready for:**

- Default system `strip` / `objcopy` in packaging
- Multi-arch embedded bring-up (AArch64 disasm missing)
- Static library production (`ar rcs` / ranlib)
- Full GNU script compatibility (flag matrix, SFrame/CTF)
- Claiming PE/Mach-O parity

---

## 8. Next implementation roadmap

Aligned to **binutils 2.46.1** as the behavioural oracle. Phases are sequenced by **risk reduction first**, then **parity**, then **breadth**.

### Phase A — Correctness gate (0.1.1) — **NOW**

**Goal:** Stop shipping blind.

| Deliverable | Detail |
|-------------|--------|
| A1 | Fix docs: reference version **2.46.1** everywhere (README, gnu-compatibility, ROADMAP) |
| A2 | GNU differential harness: `tests/integration/` compare exit codes + structural output vs system/`packages/binutils-2.46.1` build |
| A3 | Corpus: ET_REL `.o`, ET_DYN `.so`, ET_EXEC, static `.a`, stripped, with DWARF, PE/Mach-O samples optional |
| A4 | `OXIDE_COMPARE_GNU=1` CI job (optional skip if no GNU) |
| A5 | Fix **nm `-S`** GNU semantics (print size); move size-sort to long option only if still desired |
| A6 | strip/objcopy: **fail loud** when ELF32/unsupported instead of no-op identity |
| A7 | Atomic write + mode preserve for strip/objcopy |
| A8 | Clear unused imports; `cargo clippy -D warnings` in CI |

**Exit criteria:** CI fails if nm/objdump/readelf/size smoke diffs break on golden corpus.

### Phase B — Mutation hardening (0.2)

**Goal:** Make strip/objcopy trustworthy for ELF64 host objects.

| Deliverable | Detail |
|-------------|--------|
| B1 | Rewrite strip pipeline around validated stages: parse → plan keep-set → rewrite → re-parse verify |
| B2 | Full ELF32 path parity with ELF64 |
| B3 | Preserve PHDRs, dynamic table, GNU hash, build-id notes under strip-all |
| B4 | objcopy: document supported matrix; implement `-O binary` multi-section rules matching GNU for common cases |
| B5 | Round-trip tests: `strip` then `readelf`/`ldd`/`file` still sane |
| B6 | Never claim “GNU complete”; version banner stays Zainium |

**Exit criteria:** strip-all on `/bin/ls` copy still runs; relocatable strip keeps linkable objects for a sample `.c` program.

### Phase C — Archive completeness (0.3)

**Goal:** Real `ar` for build systems.

| Deliverable | Detail |
|-------------|--------|
| C1 | Operations: `r`/`q` (insert), `d` (delete), `c` (create), `x` with member list, `t` verbose |
| C2 | Symbol index (`/` / `__.SYMDEF`) + **`oxide-ranlib`** or `ar s` |
| C3 | Thin archives read (detect already) + careful write policy |
| C4 | Deterministic archives (`D`) for reproducible builds |

**Exit criteria:** `oxide-ar rcs libfoo.a a.o b.o` works with `oxide-nm` and system `ld`.

### Phase D — readelf / objdump depth (0.4)

**Goal:** Daily driver for ELF reverse engineering on Linux.

| Deliverable | Detail |
|-------------|--------|
| D1 | readelf: version info, symbol versions, unwind (eh_frame summary), compressed sections |
| D2 | readelf: **`--got-contents`** (2.46 parity item) |
| D3 | objdump/readelf: **SFrame** dump (v2 read + v3 if present) — libsframe semantics without linking GNU |
| D4 | objdump: reloc pretty-print, file offsets, richer `-x` |
| D5 | DWARF dump subset (`--dwarf=info/line` lite) *or* explicit “use llvm-dwarfdump” deferral |

**Exit criteria:** `readelf -a` shape covers 80% of fields engineers grep for on glibc-linked x86_64 bins.

### Phase E — Multi-arch disassembly (0.5)

**Goal:** Zainium multi-arch honesty.

| Deliverable | Detail |
|-------------|--------|
| E1 | AArch64 backend (capstone *or* dedicated crate; decide single plugin trait) |
| E2 | RISC-V 64 as stretch |
| E3 | Trait `DisasmBackend` in core; hex fallback only when `--allow-hex-fallback` |
| E4 | Default: error on `-d` for unsupported arch (no silent wrong asm) |

### Phase F — Flag parity push (0.6)

**Goal:** Script compatibility for common flags only (not 100% of 146 objcopy opts).

Priority flag packs:

1. nm: BSD/POSIX formats, `-f`, radix  
2. strings: `-e` encodings, `-f` filename  
3. size: common `-d/-o/-x`  
4. objdump: `-j`, `--start-address`, source intermix later  
5. addr2line: `--section`, recursive debuglink  

### Phase G — Host format depth (0.7)

| Deliverable | Detail |
|-------------|--------|
| G1 | PE: sections + exports summary in objdump/nm paths |
| G2 | Mach-O: fat/slice headers, basic sections |
| G3 | Feature-gate experimental formats |

### Phase H — UX / packaging (0.8)

| Deliverable | Detail |
|-------------|--------|
| H1 | Man pages from `docs/man/` |
| H2 | Optional JSON (`--oxide-json`) opt-in only |
| H3 | Colour policy: respect `NO_COLOR` |
| H4 | Distro packages / install prefixes |
| H5 | True multicall single binary option (compile-time) |

### Phase I — Hardening → 1.0

| Deliverable | Detail |
|-------------|--------|
| I1 | cargo-fuzz targets: ELF parse, archive, strip plan |
| I2 | Security policy doc (malicious object handling) |
| I3 | Performance: mmap defaults, multi-file rayon (host) |
| I4 | Audit pass; freeze API; 1.0 release criteria checklist |

### `oxide-as` / `oxide-ld` — status update (2026-07-27)

Previously listed below as an explicit non-goal ("gas is a multi-decade
project" / "different product class"). That framing undersold what's
actually achievable at reduced scope: a real (not full-parity) x86_64
AT&T assembler and ELF64 linker now exist and are **verified working
end-to-end against real system glibc** — assembled with `oxide-as`, linked
with `oxide-ld` against `/usr/lib64/libc.so` (glibc's `GROUP()` script) and
`/lib64/ld-linux-x86-64.so.2`, and *actually executed*, printing output via
a real dynamically-resolved `puts()` call and exiting via `exit()`. Archive
`-l`/`-L` resolution was verified to extract only the archive member that
defines a still-undefined symbol (fixpoint over `undefined_refs`), not
every member.

What's real now:
- `oxide-as`: AT&T x86_64 subset with SIB-byte addressing
  (`disp(base,index,scale)`), `.equ`/`.set`, hard error (not silent NOP) on
  any unrecognized instruction/operand shape.
- `oxide-ld`: section/symbol merge, iterative archive member extraction,
  real `-lNAME`/`-LDIR` search (`.so` then `.a`, following GNU ld
  `GROUP()`/`AS_NEEDED()` scripts like glibc's `libc.so`), real section
  headers + `.symtab`/`.strtab` in the output (readable by real `readelf`/
  `nm`), and eager-bound (`DF_BIND_NOW`) dynamic linking: `.dynsym`/
  `.dynstr`/`.hash`/`.plt`/`.got.plt`/`.rela.plt`/`.rela.dyn`/`.dynamic` +
  `PT_DYNAMIC`, with `R_X86_64_RELATIVE` self-relocations for PIE/shared
  absolute-64 relocations against locally-defined symbols.

Known, documented gaps (not silently claimed as done):
- No lazy PLT0/`_dl_runtime_resolve` trampoline — binding is always eager
  (`DF_BIND_NOW`); structurally different from GNU ld's PLT but verified to
  work with glibc's real `ld.so`.
- Dynamic **data** symbol imports (GOT-relative, e.g. `extern int errno`)
  are rejected with a clear error — only **function calls** via PLT are
  supported; the assembler doesn't yet emit `@GOTPCREL` addressing to ask
  for one anyway.
- `-shared` output's own exported `.dynsym` entries use `st_shndx=SHN_ABS`
  (a documented simplification) — correct when another module only calls
  *into* it as a plain `DT_NEEDED`, not verified correct if that address is
  expected to be ASLR-slid when the `.so` itself is loaded PIE-style by a
  chain of re-exports.
- No TLS, no symbol versioning (`.gnu.version`), no IFUNC, no linker-script
  expression language beyond `ENTRY()`/`SECTIONS{}`/glob patterns, x86_64
  only (no i386/AArch64), no macro assembler, no floating point/SSE/AVX
  encoding.
- `bfd`/`gas`/`ld`'s full flag matrices are not replicated — this is a
  from-scratch minimal implementation verified for the paths above, not a
  byte-for-byte behavioural clone.

### Other explicit non-goals until post-1.0

| Item | Why defer |
|------|-----------|
| BFD plugin ABI | C world; not needed for Rust-native |
| windres / dlltool / Windows resource chain | Niche unless Windows is a product target |
| gprofng | Separate performance product |

---

## 9. Recommended 90-day plan (practical)

| Weeks | Focus | Outcomes |
|-------|--------|----------|
| 1–2 | Phase A | GNU golden tests, nm `-S` fix, strip fail-loud, docs → 2.46.1 |
| 3–5 | Phase B | Safe ELF strip/objcopy + round-trip CI |
| 6–8 | Phase C | `ar rcs` + ranlib |
| 9–10 | Phase D (partial) | versym + notes polish + GOT dump |
| 11–12 | Phase E start | AArch64 disasm trait + one backend |

**Success metric at day 90:** Oxide can replace GNU **nm, size, strings, addr2line, readelf (-hSladrn), objdump (-hftd on x86_64), ar (t/p/x/rcs)** in a Zainium CI job with differential tests green; strip/objcopy usable on **ELF64** with verified corpus (still opt-in, not system default).

---

## 10. Priority backlog (ordered)

1. **Integration / golden tests vs 2.46.1** (R4)  
2. **strip/objcopy correctness + no silent no-op** (R1–R3)  
3. **nm `-S` GNU alignment** (R5)  
4. **ar write + ranlib** (R6)  
5. **Bump compatibility target to 2.46.1** (R14)  
6. **readelf depth + SFrame/GOT awareness** (R8, 2.46)  
7. **AArch64 disasm** (R7)  
8. **Fuzz + malicious object policy** (R13, Phase I)  
9. PE/Mach-O real depth or demote claims (R9)  
10. Packaging / man / 1.0 polish  

---

## 11. Final verdict

OxideUtils is a **well-started Rust binutils *utilities* suite**, not a binutils *replacement project* yet. The architecture (dual `std`/`no_std`, core façade, iced-x86, gimli addr2line) is the right long-term shape for Zainium. The gap to GNU **2.46.1** is primarily:

1. **breadth** (no as/ld/opcodes matrix/CTF/SFrame/tools),  
2. **depth** (readelf/objcopy flag surface),  
3. **mutation correctness**, and  
4. **test discipline**.

Treat **0.1** as a **developer inspection toolkit + kernel parse library**.  
Treat **strip/objcopy** as **experimental** until Phase B exits.  
Drive the next two quarters with **tests-first parity** against the **2.46.1** tree you already have on disk — that tree is the best oracle you own.

---

## 12. Appendix — crate status scorecard

| Crate | Completeness | Correctness confidence | GNU-drop-in readiness |
|-------|--------------|------------------------|------------------------|
| oxideutils-core | 45% | Medium | N/A (lib) |
| oxide-objdump | 35% | Medium-High (x86) | Low-Med |
| oxide-nm | 40% | Medium (`-S` issue) | Low-Med |
| oxide-readelf | 20% | Medium | Low |
| oxide-size | 55% | High | Medium |
| oxide-strings | 30% | High (narrow) | Low-Med |
| oxide-ar | 15% | High for t/p/x | Low |
| oxide-strip | 25% | **Low** | **No** |
| oxide-objcopy | 15% | **Low** | **No** |
| oxide-addr2line | 50% | Medium-High | Medium |

---

*Report prepared for Zainium Dynamics engineering. Contact: alizain@zainiumdynamics.tech*
