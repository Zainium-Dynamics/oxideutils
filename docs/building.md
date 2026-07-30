# Building OxideUtils

**No Makefile.** Configuration is **only** via **`oxideutils.toml`** (`true` / `false`), then normal Cargo.

---

## Quick start

```bash
# 1) Edit config (build + tools + runtime defaults)
$EDITOR oxideutils.toml

# 2) Build
cargo build --release

# 3) Run
./target/release/oxide-objdump --help
./target/release/oxideutils --help          # multicall
```

After each build, open:

```text
target/oxideutils-build-plan.txt
```

That file shows which tools/features the TOML selected.

---

## `oxideutils.toml` — build section

```toml
[build]
standalone = false   # true → use only oxideutils <tool> …
static     = false   # true → prefer static link (best with musl target)
dynamic    = true
kernel     = false   # true → also build no_std core (see below)

[tools]
objdump = true
nm = true
# … set any tool to false to disable that binary

[features]
disasm = true
disasm_aarch64 = true
dwarf = true
```

| Setting | Effect |
|---------|--------|
| `standalone = true` | Separate `oxide-*` bins print a hint; use **`oxideutils <tool>`** |
| `static = true` | Sets build cfg; full static → `cargo build --release --target x86_64-unknown-linux-musl` |
| `tools.X = false` | That tool’s binary exits with “disabled by oxideutils.toml” |
| `kernel = true` | Reminds you to build the no_std core (command in the plan file) |

---

## Standalone (one binary)

```toml
[build]
standalone = true
```

```bash
cargo build --release
./target/release/oxideutils objdump -d ./a.out
./target/release/oxideutils readelf -a /bin/ls
./target/release/oxideutils ar rcs lib.a a.o b.o
```

---

## Static binary (fully static)

```toml
[build]
static = true
dynamic = false
```

```bash
rustup target add x86_64-unknown-linux-musl
cargo build --release --target x86_64-unknown-linux-musl
# → target/x86_64-unknown-linux-musl/release/oxideutils
```

---

## Kernel / `no_std` core

```toml
[build]
kernel = true
```

```bash
cargo build --release   # host tools (if enabled)

cargo build -p oxideutils-core --release \
  --no-default-features \
  --features "alloc,disasm,kernel" \
  --target-dir target-nostd
```

Host tools and kernel core use **different** target dirs so Cargo does not unify `std` into the kernel build.

---

## Runtime config (same file)

`[oxideutils]`, `[color]`, `[disasm]`, … are **runtime** defaults. Tools load them when you run the binary (see [configuration.md](./configuration.md)).

---

## Requirements

- Rust **1.85+** (see `rust-toolchain.toml`)
- `cargo build --release` only — **Makefile removed**

---

## Old vs new

| Old | New |
|-----|-----|
| `make` / `make release` | `cargo build --release` |
| `scripts/build-all.sh` | optional; same as cargo + kernel line |
| Guessing flags | Edit **`oxideutils.toml`** |
