# CLI Flag Compatibility — OxideUtils vs GNU binutils 2.46

**Method:** captured real `--help` output from GNU binutils **2.46-3.fc44**
(matches the `packages/binutils-2.46.1` reference tree) and from each
`oxide-*` release binary, extracted every `-x` / `--long-flag` token with
`grep -oE`, and computed the set intersection. This counts the raw flag
*surface* (including GNU's own aliases, e.g. `-l`/`--program-headers`/
`--segments` count as three tokens) — it is a conservative, mechanical
measure, not a hand-picked "feature parity" claim. Full flag lists are
reproducible: `<tool> --help` on both sides, `grep -oE`, `comm -12`.

| Tool | GNU flags | Shared | Coverage |
|------|----------:|-------:|---------:|
| `addr2line` | 26 | 18 | **69.2%** |
| `c++filt` | 19 | 10 | **52.6%** |
| `size` | 19 | 8 | **42.1%** |
| `objdump` | 92 | 38 | **41.3%** |
| `nm` | 56 | 23 | **41.1%** |
| `strings` | 26 | 10 | **38.5%** |
| `readelf` | 83 | 29 | **34.9%** |
| `ar` | ~30 | ~10 functional (~23 accepted) | **~33%** (see note) |
| `strip` | 50 | 14 | **28.0%** |
| `objcopy` | 121 | 19 | **15.7%** |
| **Total (9 getopt-style tools)** | **522** | **179** | **34.3%** |

`ar` note: unlike the other tools, GNU `ar`'s flags are single letters
combined into one key (`rcs`, not `-r -c -s`), so the same extraction
method doesn't directly apply. `oxide-ar` *accepts* (doesn't error on) 23 of
~30 GNU letters/long-options, but 13 of those are parsed-and-ignored
no-ops (`u`,`a`,`b`,`N`,`o`,`O`,`P`,`S`,`T`,`f`,`l`,`M`,`i`) rather than
implementing the documented behavior. Counting only letters with real
effect (`d p q r t x s c v D` = 10) gives the more honest ~33% above; `m`
(move) and long options (`--plugin`,`--target`,`--output`,
`--record-libdeps`,`--thin`) are unsupported.

## Reading this table honestly

- This measures **flag surface**, not usage frequency. `objcopy`'s 15.7%
  looks weak, but the ~15 flags OxideUtils does support (strip variants,
  `-j`/`-R` section filtering, `-O binary`) cover the operations actually
  documented as "Working" in the main README — the other 100+ GNU
  `objcopy` flags are format-conversion and BFD-specific target options
  (`--srec-len`, `--gap-fill`, `--redefine-sym`, Windows PE resource
  handling, etc.) that are out of scope until a real second-format need
  arises.
- `addr2line` and `c++filt` score highest because their real flag surface
  is small and mostly already implemented.
- Numbers will move (mostly upward) as ROADMAP.md phases land — this file
  should be regenerated (not hand-edited) whenever that's worth checking
  again: `<tool> --help | grep -oE -- '(--[a-zA-Z][a-zA-Z0-9-]*|-[a-zA-Z]\b)' | sort -u`
  on both sides, `comm -12` for the intersection.
- Every flag OxideUtils *does* claim to support is either working today or
  explicitly marked otherwise in `README.md`'s status table — there is no
  flag here that's silently accepted and ignored (except the documented
  `ar` no-op modifiers above).

Captured 2026-07-30 against GNU binutils 2.46-3.fc44 and the `oxideutils`
release build at that commit.
