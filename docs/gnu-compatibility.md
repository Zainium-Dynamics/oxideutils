# GNU compatibility policy

**OxideUtils** (Zainium Dynamics) targets behavioural compatibility with **GNU binutils 2.46.1** for everyday workflows.

This document is a **compatibility contract**, not a claim of GNU affiliation.

---

## Identity

| | GNU binutils | OxideUtils |
|--|--------------|------------|
| Owner | FSF / GNU Project | **Zainium Dynamics** |
| Licence | GPLv3 | **GPLv3 only** |
| Language | C | **Rust** |
| Version banner | GNU | **OxideUtils — Zainium Dynamics** |

OxideUtils is **not** a GNU package.

---

## Reference version

- **Primary reference:** GNU binutils **2.46.1** (e.g. `packages/binutils-2.46.1` on the Zainium drive for QA).  
- We do **not** compile or link `libbfd` / `libopcodes` from that tree into OxideUtils.

---

## Shared policies

| Topic | Policy |
|-------|--------|
| Exit codes | `0` ok, `1` soft error, `2` usage |
| Multi-file | Continue on error; max status wins |
| Addresses | Accept `0x…`, decimal, octal where tools do |
| `@file` | Supported in core utils (host) |
| Help vs `-h` | **objdump/readelf:** `-h` = headers, `-H` = help (GNU-like) |

---

## Tool matrix (0.1.x — Phases A–C)

| Feature | GNU | Oxide 0.1.x |
|---------|-----|-------------|
| objdump `-h -f -t -s` | yes | yes |
| objdump `-d` x86_64 | yes | yes (iced-x86) |
| objdump full arch matrix | yes | partial |
| objdump archives | yes | yes (list + open members) |
| nm common filters | yes | yes |
| nm `-S` / `--print-size` | print-size | **print-size** (GNU-aligned); size-sort is `--size-sort` |
| readelf `-a` deep | yes | solid subset + Phase D |
| readelf `-V` versioning | yes | **yes** (versym / verneed / verdef) |
| readelf `--got-contents` | yes (2.46) | **yes** |
| readelf `--sframe` | yes (2.46) | **header + FDE/FRE walk** (default FDE; flex partial) |
| objdump `-d` AArch64 | yes | **yes** (bad64) |
| readelf `-u` unwind | yes | **summary** (not full CIE/FDE) |
| size Berkeley/SysV | yes | yes |
| strings `-n -t` | yes | yes |
| ar create/replace/delete/ranlib | yes | **rcs / d / t / p / x / s** (Phase C) |
| strip ELF32+64 | yes | yes (verify re-parse; fail-loud on trunc) |
| objcopy full targets | yes | subset + binary extract + atomic write |
| addr2line DWARF | yes | yes (gimli/addr2line) |

---

## Intentional differences

1. **Version / copyright strings** — always Zainium Dynamics.  
2. **Error wording** — may be clearer; scripts should not parse error English.  
3. **Disassembly formatting** — gas-like via iced-x86; not bit-identical to libopcodes.  
4. **JSON / colour** — Oxide extensions will be opt-in (`--oxide` / env), not default when GNU scripts matter.  
5. **Missing obscure flags** — tracked on the roadmap; prefer documenting over silent wrong behaviour.

---

## Testing strategy

```bash
# Smoke vs system tools when installed
objdump -h /bin/ls > /tmp/gnu.txt
./target/debug/oxide-objdump -h /bin/ls > /tmp/ox.txt
# compare shapes, not necessarily full diff

addr2line -e ./binary -f -C 0xADDR
./target/debug/oxide-addr2line -e ./binary -f -C 0xADDR
```

Future: golden corpus under `tests/integration/` with optional `OXIDE_COMPARE_GNU=1`.

---

## Reporting incompatibilities

Email **alizain@zainiumdynamics.tech** with:

1. GNU binutils version  
2. Exact command line  
3. Input file class (ELF ET_DYN, .o, archive, …)  
4. Expected vs actual (exit code + relevant stdout)

---

## See also

- [tools.md](./tools.md)  
- [ROADMAP.md](../ROADMAP.md)  
