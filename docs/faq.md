# FAQ — OxideUtils

**Zainium Dynamics** · [zainiumdynamics.tech](https://zainiumdynamics.tech) · alizain@zainiumdynamics.tech

---

### Is this a GNU project?

**No.** OxideUtils is a **Zainium Dynamics** product. It is only *compatible* with common GNU binutils behaviours. It is not affiliated with the GNU Project or FSF.

---

### Why GPLv3 if it is not GNU?

GPLv3 is a **licence** choice for Free Software distribution and copyleft. Ownership and branding remain Zainium Dynamics. See [LICENSE](../LICENSE).

---

### Does it work in our no_std kernel?

**Yes** — use `oxideutils-core` with:

```toml
default-features = false
features = ["alloc", "disasm", "kernel"]
```

See [kernel-integration.md](./kernel-integration.md).

---

### How do I build without a Makefile?

```bash
# edit true/false in oxideutils.toml, then:
cargo build --release
```

See [building.md](./building.md). There is **no Makefile** anymore.

### Why two target directories for the kernel?

Cargo feature unification would force `std` into the library if tools and a pure `no_std` build shared one graph.  
`target/` = host tools; `target-nostd/` = pure kernel core (separate cargo invocation).

### Standalone vs many binaries?

```toml
[build]
standalone = true   # one binary: oxideutils <tool>
```

```toml
[build]
standalone = false  # oxide-objdump, oxide-nm, … each their own file
```

---

### Why does `oxide-objdump -h` not show help?

On purpose (GNU-like): **`-h`** = section headers, **`-H` / `--help`** = help.

Same for **readelf**: **`-h`** = ELF file header, **`-H` / `--help`** = help.

### How do beginners learn the commands?

Every tool has a visual help screen:

```bash
oxide-objdump --help    # or -H
oxide-readelf --help    # or -H
oxide-ar --help
oxideutils --help       # multicall overview of all tools
```

Help includes a **flag map table**, **beginner start** lines, and **examples**.

---

### Disassembly looks different from GNU objdump

- **x86/x86_64:** **iced-x86** (gas-like). Mnemonics can differ from libopcodes.  
- **AArch64:** **bad64** (host default). Not bit-identical to GNU.  
- Other arches: hex / `.byte` fallback unless you disable fallback in library options.

### Does SFrame dump show FRE rows?

**Yes.** `oxide-readelf --sframe` / `oxide-objdump --sframe` print the header **and** per-function FRE lines (CFA / FP / RA). Flexible FDE semantics are still partial vs full libsframe.

---

### addr2line prints `??:?`

Often the address has **no line info** (same as GNU), or only a symbol. Try mid-function addresses with DWARF present (`cargo build` debug profiles include rich DWARF).

---

### Is `ar r` / `ranlib` supported?

**Yes (Phase C):** `oxide-ar rcs lib.a a.o b.o`, plus `t` / `p` / `x` / `d` / `s`. Thin archives are still read-only.

---

### Does oxide-readelf support GOT / SFrame / versions?

**Yes (Phase D):**
- `-V` / `--version-info` — versym / verneed / verdef  
- `--got-contents` — GNU 2.46-style GOT dump  
- `--sframe` — SFrame **header** summary (full FRE walk later)  
- `-u` — `.eh_frame` summary  

Reference: GNU binutils **2.46.1**.

---

### Will you rewrite `as` and `ld`?

Not in the 0.1 utility scope. Assembler/linker may be separate future Zainium projects.

---

### How do I compare with system binutils?

```bash
make
./target/debug/oxide-objdump -d /bin/ls | head
objdump -d /bin/ls | head
./target/debug/oxide-readelf -V --got-contents /bin/ls | head
readelf -V --got-contents /bin/ls | head
```

---

### Who do I contact?

- **Email:** alizain@zainiumdynamics.tech  
- **Web:** https://zainiumdynamics.tech  

---

### Where is the full tool flag list?

[tools.md](./tools.md)
