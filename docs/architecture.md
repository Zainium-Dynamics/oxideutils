# Architecture

**OxideUtils** · **Zainium Dynamics** · GPLv3 · Not a GNU project

---

## Goals

1. Host tools that feel like classic binutils CLIs  
2. A **kernel-safe** library path (`no_std` + `alloc`) for Zainium  
3. Clear module boundaries; thin binaries  
4. No accidental dependency on libbfd / linking GNU code  

---

## High-level diagram

```text
┌─────────────────────────────────────────────────────────────┐
│  Host (std)                                                 │
│  oxide-objdump  oxide-nm  oxide-readelf  oxide-size  …      │
│  oxide-strip  oxide-objcopy  oxide-addr2line  oxideutils    │
└────────────────────────────┬────────────────────────────────┘
                             │  uses
┌────────────────────────────▼────────────────────────────────┐
│  oxideutils-core                                            │
│  ┌────────────── std-only ──────────────┐                   │
│  │ cli  strip  objcopy  addr2line_util  │                   │
│  └──────────────────────────────────────┘                   │
│  ┌────────── std + no_std (+alloc) ─────┐                   │
│  │ error  format  symbols  archive      │                   │
│  │ utils (pure)  disasm (optional)      │                   │
│  └──────────────────────────────────────┘                   │
└────────────────────────────┬────────────────────────────────┘
                             │
        object · goblin · iced-x86 · gimli/addr2line(std)
                             │
┌────────────────────────────▼────────────────────────────────┐
│  Zainium kernel (no_std)                                    │
│  depends: oxideutils-core (alloc + disasm + kernel)         │
│  feeds: ELF / module images as &[u8]                        │
└─────────────────────────────────────────────────────────────┘
```

---

## Crates

| Crate | Type | Features |
|-------|------|----------|
| `oxideutils-core` | `rlib` (+ multicall bin when `std`) | Dual mode |
| `oxide-objdump` | bin | `std` |
| `oxide-nm` | bin | `std` |
| `oxide-readelf` | bin | `std` |
| `oxide-size` | bin | `std` |
| `oxide-ar` | bin | `std` |
| `oxide-strings` | bin | `std` |
| `oxide-strip` | bin | `std` |
| `oxide-objcopy` | bin | `std` |
| `oxide-addr2line` | bin | `std` |

Workspace root: `oxideutils/Cargo.toml` (`resolver = "2"`).

---

## Core modules

| Module | `no_std` | Role |
|--------|----------|------|
| `error` | yes | `OxideError`, exit codes |
| `format::object` | yes | `OxideObject` over `object` crate |
| `format::elf` | yes | goblin-backed ELF detail (**version / GOT / SFrame / unwind** Phase D) |
| `format::pe` / `macho` | std | Host extras |
| `format::wasm` | yes | Magic/version summary |
| `symbols` | yes | Portable symbol list / nm codes |
| `archive` | yes | `ar` parse from bytes |
| `archive_write` | std | `ar` create / delete / symbol index |
| `disasm` | yes* | iced-x86 when `disasm` feature |
| `utils` | mixed | Pure helpers + `atomic_write` under `std` |
| `cli` | std | clap helpers, multicall, config |
| `strip` | std | ELF32/64 strip + re-parse verify |
| `objcopy` | std | Copy / filter / binary extract |
| `addr2line_util` | std | DWARF via `addr2line` Loader |

\* requires `alloc` + feature `disasm`.

---

## Object pipeline (host tool)

```text
path → map/read → is_archive?
         │              │
         │ yes          │ no
         ▼              ▼
   OxideArchive    OxideObject::parse
         │              │
         └─ member ─────┘
                    │
         ┌──────────┼──────────┐
         ▼          ▼          ▼
      headers   symbols    disasm/hex
```

## Kernel pipeline

```text
&[u8] + label → OxideObject::parse_bytes / ElfFile::parse
                      │
              section_views / symbols / disasm
```

No filesystem, no threads required.

---

## Error model

- Type: `oxideutils_core::error::OxideError`  
- `Display` works on `no_std`  
- `std::error::Error` only with `std`  
- I/O variants only with `std`  
- Paths in errors are **`String` labels** (kernel-friendly)

---

## Disassembly

| Backend | Feature | Arches |
|---------|---------|--------|
| **iced-x86** (gas) | `disasm` | i386, x86_64 |
| **bad64** | `disasm-aarch64` | AArch64 |
| hex / `.byte` fallback | always (if `allow_hex_fallback`) | others |

- Host `default` features enable **both** x86 and AArch64.
- Kernel profile (`kernel` / `alloc+disasm`) keeps **iced only** (no bad64-sys bindgen).
- GNU opcodes/`libopcodes` are **not** linked.

## SFrame

| Layer | Status |
|-------|--------|
| Header (v2/v3) | yes |
| FDE table walk | yes (v2 + v3 index/attr) |
| FRE rows (CFA/FP/RA) | yes (default FDE) |
| Flexible FDE full semantics | partial / raw words |

---

## Mapping to classic binutils concepts

| Classic | OxideUtils |
|---------|------------|
| `bucomm` / getopt | `cli` (std) |
| BFD open | `format::object` |
| `readelf` internals | `format::elf` + goblin |
| `nm` | `symbols` + `oxide-nm` |
| `opcodes` | `disasm` |
| DWARF consumers | `addr2line_util` |

This is a **conceptual** map, not an ABI or source port.

---

## Design rules

1. Binaries stay thin; logic in core.  
2. Kernel path never requires `std`.  
3. Documented `unsafe` only (e.g. mmap).  
4. Prefer ecosystem crates (`object`, `goblin`, `iced-x86`, `gimli`) over NIH.  
5. Branding and licence: Zainium Dynamics, GPLv3 only.

---

## Future architecture notes

- Trait object for multi-arch disasm plugins  
- JSON emitter shared across tools  
- Rayon parallel multi-file on host  
- Optional `oxide-as` / linker experiments stay **out** of 0.1 scope  
