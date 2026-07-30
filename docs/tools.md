# OxideUtils tools — CLI reference

**Vendor:** Zainium Dynamics · **Licence:** GPLv3 · **Not a GNU project**

All tools print Zainium Dynamics branding on `-v` / `--version` (where implemented).

---

## Exit codes (shared policy)

| Code | Meaning |
|------|---------|
| `0` | Success |
| `1` | Operational error (bad file, parse fail, …) |
| `2` | Usage error (missing flags / args) |

Multi-file tools continue after errors and return the worst status.

---

## oxide-objdump

**Purpose:** Display information from object files and archives.  
**Core:** `OxideObject`, `disasm`, symbols, hex dump.

### Synopsis

```text
oxide-objdump [options] <file>...
```

### Display switches (at least one required)

| Flag | Long | Description |
|------|------|-------------|
| `-a` | `--archive-headers` | Archive member headers |
| `-f` | `--file-headers` | Overall file header |
| `-h` | `--section-headers` | Section headers |
| `-x` | `--all-headers` | Headers + (ELF) dynamic / notes / versions |
| `-d` | `--disassemble[=SYM]` | Disassemble code sections (`--disassemble=SYM` only with `=`) |
| `-D` | `--disassemble-all` | Disassemble all sections |
| `-s` | `--full-contents` | Hex dump of section contents |
| `-t` | `--syms` | Symbol table |
| `-T` | `--dynamic-syms` | Dynamic symbols (subset) |
| `-r` | `--reloc` | Relocations (pretty type + symbol target) |
| `-R` | `--dynamic-reloc` | Dynamic relocations |
| | `--sframe[=SEC]` | SFrame stack-trace section summary |
| `-i` | `--info` | Supported formats |
| `-v` | `--version` | Version |
| `-H` | `--help` | Help (**not** `-h`) |

### Options

| Flag | Description |
|------|-------------|
| `-j`, `--section=NAME` | Limit to section |
| `-C`, `--demangle` | Demangle symbols |
| `-w`, `--wide` | Wide output (reserved) |
| `-z`, `--disassemble-zeroes` | Do not elide zero runs |
| `--start-address=ADDR` | Start |
| `--stop-address=ADDR` | Stop |

### Examples

```bash
oxide-objdump -h -f app.o
oxide-objdump -d /bin/ls | less
oxide-objdump -t -C libfoo.a
```

### Disassembly notes

- **x86 / x86_64:** iced-x86, gas/AT&T-like syntax  
- **AArch64:** bad64 (host `disasm-aarch64` feature; default on)  
- **Other arches:** labeled `.byte` / hex fallback (`DisasmOptions::allow_hex_fallback`)  

### SFrame (`--sframe`)

Dumps section header **and** per-function FDE + FRE rows (start PC, CFA base+offset, FP/RA recovery). GNU 2.46 SFrame v2/v3 header-compatible.

---

## oxide-nm

**Purpose:** List symbols from object files and archives.

### Synopsis

```text
oxide-nm [options] <file>...
```

| Flag | Description |
|------|-------------|
| `-g` | External only |
| `-u` | Undefined only |
| `-U` | Defined only |
| `-C` | Demangle |
| `-n` / `-v` | Sort by address |
| `-S` / `--print-size` | Print size column (**GNU-compatible**) |
| `--size-sort` | Sort by size |
| `-p` | No sort |
| `-r` | Reverse sort |
| `-A` / `-o` | Print file name |
| `-V` | Version |

```bash
oxide-nm -n ./target/debug/oxide-nm
oxide-nm -u app.o
```

---

## oxide-readelf

**Purpose:** Display ELF file structure (GNU readelf subset + Phase D depth).

| Flag | Description |
|------|-------------|
| `-h` | File header (`-H` = help) |
| `-S` | Section headers (lists compressed sections) |
| `-l` | Program headers / segments |
| `-s` | Symbol tables |
| `-d` | Dynamic section (NEEDED/SONAME resolved) |
| `-r` | Relocations (pretty-print + symbol names) |
| `-n` | Notes (build-id, ABI tag, …) |
| `-V` / `--version-info` | Symbol versioning (versym / verneed / verdef) |
| `--got-contents` | GOT dump (binutils **2.46**) |
| `--sframe[=SEC]` | SFrame header summary (default `.sframe`) |
| `-u` / `--unwind` | `.eh_frame` / `.eh_frame_hdr` summary |
| `-a` | `-h -l -S -s -r -d -n -V -u --got-contents` |
| `-v` | Program version banner |

```bash
oxide-readelf -a /bin/ls
oxide-readelf -V --got-contents -u /bin/ls
oxide-readelf -n /lib64/libc.so.6
oxide-readelf --sframe=/path  # when .sframe present
```

---

## oxide-size

**Purpose:** Section size summary.

```bash
oxide-size file
oxide-size -t file              # totals
oxide-size -A berkeley file     # default
oxide-size -A sysv file
```

Berkeley columns: `text data bss dec hex filename`.

---

## oxide-strings

**Purpose:** Print sequences of printable characters.

```bash
oxide-strings file
oxide-strings -n 6 file
oxide-strings -t x file         # hex offset (also o/d)
```

Default minimum length: **4**.

---

## oxide-ar

**Purpose:** Create, modify, list, and extract static archives (GNU ar subset).

```bash
oxide-ar rcs lib.a a.o b.o    # create + symbol index
oxide-ar t lib.a
oxide-ar p lib.a
oxide-ar x lib.a
oxide-ar d lib.a a.o
oxide-ar s lib.a              # ranlib-style index rebuild
oxide-ar -V
```

Keys: `d p q r t x s` · modifiers: `c v D` (deterministic).

---

## oxide-strip

**Purpose:** Discard symbols and debug sections from ELF (and relocatable rebuilds where applicable).

```bash
oxide-strip file                  # default ≈ strip-all
oxide-strip -s file
oxide-strip -g file               # debug
oxide-strip --strip-unneeded file
oxide-strip -o out in
oxide-strip -v -p file            # verbose; preserve dates when possible
```

In-place write uses a temporary then rename.

---

## oxide-objcopy

**Purpose:** Copy and transform object files.

```bash
oxide-objcopy IN OUT
oxide-objcopy --strip-all IN OUT
oxide-objcopy --strip-debug IN OUT
oxide-objcopy -j .text -j .data IN OUT
oxide-objcopy -R .comment IN OUT
oxide-objcopy -O binary -j .text IN OUT.bin
oxide-objcopy -v IN OUT
```

| Flag | Description |
|------|-------------|
| `--strip-all` / `-S` | Strip symbols |
| `--strip-debug` | Drop debug-ish sections |
| `--strip-unneeded` | Keep only needed symbols |
| `-j` | Only section |
| `-R` | Remove section |
| `-O binary` | Raw section extract |
| `-I` | Input target (accepted, currently ignored) |

---

## oxide-addr2line

**Purpose:** Map addresses to source locations (DWARF).

```bash
oxide-addr2line -e binary -f -C -a 0x1234
oxide-addr2line -e binary -p -f -C 0x1234
oxide-addr2line -e binary -i -f 0x1234
oxide-addr2line -e binary -s -f 0x1234     # basenames
```

| Flag | Description |
|------|-------------|
| `-e`, `--exe` | Debug-capable file (default `a.out`) |
| `-f` | Function names |
| `-C` | Demangle |
| `-p` | Pretty print |
| `-s` | Basenames only |
| `-i` | Inlines |
| `-a` | Print address before result |
| (no addrs) | Read addresses from stdin |

Parity with system `addr2line` is good when DWARF line info exists; thin wrappers may show `??:?` (same as GNU).

---

## oxideutils (multicall)

```bash
oxideutils --help
oxideutils --version
oxideutils objdump -d /bin/ls
oxideutils nm -n ./a.out
```

Requires the corresponding `oxide-*` binary on `PATH`.

---

## Environment (planned / partial)

| Variable | Intent |
|----------|--------|
| `NO_COLOR` | Disable colour |
| `OXIDEUTILS_COLOR=always` | Force colour |
| `OXIDEUTILS_JSON` | Prefer JSON (future) |
| `OXIDEUTILS_ENHANCED` | Non-GNU enhanced mode (future) |

---

## See also

- [api-core.md](./api-core.md) — library behind the CLIs  
- [gnu-compatibility.md](./gnu-compatibility.md) — intentional differences  
