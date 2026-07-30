# Speed & Memory Benchmarks — OxideUtils vs GNU binutils 2.46

**Method:** wall-clock time via repeated `bash time` runs (N shown per
row), peak RSS via `/usr/bin/time -v` (single run — RSS is stable
run-to-run for these tools). Target file: `/usr/lib64/libc.so.6` (2.4MB,
real glibc — substantial symbol/section/reloc/dynamic content, not a
toy binary). Host: Linux x86_64, GNU binutils 2.46-3.fc44, release
(`--release`) OxideUtils build. Output line counts are included so the
comparison is checked for doing comparable work, not just being faster at
doing less.

No `hyperfine` on this host — numbers are simple wall-clock sums over N
runs, not statistically rigorous distributions. Treat as directional, not
publication-grade.

## Wall-clock time

| Operation | Runs | GNU total | GNU/run | Oxide total | Oxide/run | Result |
|-----------|-----:|----------:|--------:|-------------:|----------:|--------|
| `readelf -a` | 10 | 1.896s | 189.6ms | 0.146s | 14.6ms | **oxide ~13.0x faster** |
| `nm` (no flags) | 10 | 0.374s | 37.4ms | 0.216s | 21.6ms | **oxide ~1.7x faster** |
| `objdump -d` | 5 | 3.291s | 658.2ms | 2.011s | 402.2ms | **oxide ~1.6x faster** |
| `size` | 20 | 0.068s | 3.4ms | 0.083s | 4.2ms | **GNU ~1.2x faster** |

`size` is the one operation where GNU wins — it's cheap enough that
process-startup overhead (Rust binary size, dynamic loader work) dominates
over actual work done, and a small statically-optimized C binary starts
marginally faster than a larger Rust one for near-instant tasks.

## Output-volume sanity check (are they doing comparable work?)

| Operation | GNU output lines | Oxide output lines |
|-----------|------------------:|---------------------:|
| `readelf -a` | 14,320 | 12,718 (88.8%) |
| `nm` | 8,122 | 8,123 (100%) |
| `objdump -d` | 380,519 | 364,173 (95.7%) |

`nm`'s output is essentially identical; `readelf -a`/`objdump -d` are in
the same ballpark (oxide's `-a` omits a few sub-reports like
`--got-contents`, and disassembly coverage isn't 100% for every opcode —
both documented elsewhere). The speed numbers above are meaningful, not
an artifact of oxide doing dramatically less work.

## Peak memory (RSS)

| Operation | GNU peak RSS | Oxide peak RSS | Result |
|-----------|-------------:|----------------:|--------|
| `readelf -a` | 9,588 KB | 7,356 KB | **oxide uses 23% less** |
| `nm` | 42,328 KB | 7,392 KB | **oxide uses 82% less (5.7x)** |
| `objdump -d` | 12,864 KB | 98,036 KB | **oxide uses 7.6x more** — real weakness |

`objdump -d`'s memory result is a genuine, currently-unaddressed weakness,
not an oversight in this report: `oxide-objdump` appears to buffer
substantially more disassembly output/state in memory than GNU's
streaming approach. Worth profiling before claiming a memory-efficiency
win across the board — the honest picture is "much better on
metadata-heavy operations (readelf, nm), worse on bulk disassembly."

## Binary size

| Binary | Size |
|--------|-----:|
| GNU `nm` | 50 KB |
| Oxide `nm` | 1,093 KB (22x larger) |
| GNU `readelf` | 853 KB |
| Oxide `readelf` | 748 KB (12% smaller) |
| GNU `objdump` | 470 KB |
| Oxide `objdump` | 3,839 KB (8.2x larger) |

GNU binutils tools dynamically link against shared `libbfd`/`libopcodes`
(not counted above — the real footprint across many tools sharing one
`.so` is smaller than these numbers suggest in aggregate). OxideUtils
binaries statically link their dependencies (`object`, `iced-x86`, `clap`,
`gimli`), which is why single-binary size looks worse for `nm`/`objdump`
specifically (their real work is small; the dependency closure isn't).

## Reproducing this

```bash
# time
time ( for i in $(seq 1 10); do <tool> <args> FILE > /dev/null 2>&1; done )
# memory
/usr/bin/time -v <tool> <args> FILE > /dev/null 2> time.txt
grep "Maximum resident" time.txt
```

Captured 2026-07-30 against GNU binutils 2.46-3.fc44 and the `oxideutils`
release build at that commit.
