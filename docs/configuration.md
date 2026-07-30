# Configuration (TOML)

**Zainium Dynamics · OxideUtils**

**One file rules both build and runtime:** [`oxideutils.toml`](../oxideutils.toml).  
Booleans only: **`true` / `false`**. No Makefile, no `.ini`.

---

## Workflow

```bash
$EDITOR oxideutils.toml    # set true/false
cargo build --release      # applies [build] + [tools]
./target/release/oxide-objdump --help
cat target/oxideutils-build-plan.txt
```

---

## File locations

### Build-time (compile)

Search order for **build.rs**:

1. `$OXIDEUTILS_CONFIG`
2. Workspace root `oxideutils.toml`
3. `config/oxideutils.toml`

### Runtime (when you run a binary)

| Priority | Path |
|----------|------|
| 1 | `$OXIDEUTILS_CONFIG` |
| 2 | `./oxideutils.toml` |
| 3 | `./config/oxideutils.toml` |
| 4 | `$XDG_CONFIG_HOME/oxideutils/config.toml` |
| 5 | `~/.config/oxideutils/config.toml` |

---

## Build schema (`true` / `false`)

```toml
[build]
standalone = false   # one binary: oxideutils <tool>
static     = false   # static link preference (use musl for full static)
dynamic    = true
kernel     = false   # remind / script to build no_std core

[features]
disasm = true
disasm_aarch64 = true
dwarf = true
color = true
json = true

[tools]
objdump = true
nm = true
readelf = true
size = true
strings = true
ar = true
strip = true
objcopy = true
addr2line = true
multicall = true
```

| Goal | TOML |
|------|------|
| Single binary | `standalone = true` → run `oxideutils objdump …` |
| Disable a tool | `[tools] strip = false` |
| Static | `static = true` + musl target (see [building.md](./building.md)) |

---

## Runtime schema

```toml
[oxideutils]
gnu_compatible = true
json = false
demangle = false
continue_on_error = true
verbose = false
quiet = false

[color]
enabled = true
auto = true
always = false

[disasm]
show_raw_insn = true
disassemble_zeroes = false
gas_syntax = true
uppercase_hex = false
```

| Section | Role |
|---------|------|
| `[build]` / `[tools]` / `[features]` | **Compile-time** (build.rs) |
| `[oxideutils]` / `[color]` / … | **Runtime** defaults when tools run |
| `[log]` | verbosity |

CLI flags always **override** TOML for that invocation.

---

## Environment overrides

Applied **after** the TOML file:

| Variable | Effect |
|----------|--------|
| `OXIDEUTILS_CONFIG` | Path to TOML file |
| `NO_COLOR` | `color.enabled = false` |
| `OXIDEUTILS_COLOR` | `always` / `never` / `auto` / `true` / `false` |
| `OXIDEUTILS_JSON` | `true` if `1`/`true`/`yes`/`on` |
| `OXIDEUTILS_ENHANCED` | sets `gnu_compatible = false` |
| `OXIDEUTILS_DEMANGLE` | demangle defaults on |
| `OXIDEUTILS_VERBOSE` / `OXIDEUTILS_QUIET` | log flags |

---

## Rust API

```rust
use oxideutils_core::cli::config::{OxideToml, RuntimeConfig};

// Discover file + env
let cfg = OxideToml::load();
assert!(cfg.oxideutils.gnu_compatible);

// Explicit file
let cfg = OxideToml::load_file(std::path::Path::new("oxideutils.toml"))?;

// Façade used by tools
let rt = RuntimeConfig::load();
if rt.use_color() { /* … */ }
if rt.toml.disasm.show_raw_insn { /* … */ }
```

---

## Examples

### Always demangle, no colour

```toml
[oxideutils]
demangle = true

[color]
enabled = false
auto = false
always = false
```

### Aggressive disasm (show zeroes)

```toml
[disasm]
show_raw_insn = true
disassemble_zeroes = true
```

### JSON + enhanced (non-strict GNU) mode

```toml
[oxideutils]
json = true
gnu_compatible = false
```

### User global config

```bash
mkdir -p ~/.config/oxideutils
cp oxideutils.toml ~/.config/oxideutils/config.toml
# edit true/false as needed
```

---

## Design rules

1. **TOML only** for on-disk config.  
2. Prefer **`true` / `false`** over `yes`/`no` or `1`/`0` in files.  
3. No secret keys in config (public tool settings only).  
4. Kernel (`no_std`) does not load TOML files — host tools only.  

See also: [building.md](./building.md), [tools.md](./tools.md).
