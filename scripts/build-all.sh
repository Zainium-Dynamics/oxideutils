#!/usr/bin/env bash
# Optional helper — preferred flow is just:
#   edit oxideutils.toml && cargo build --release
#
# This script only runs cargo (and optional kernel build if noted in TOML).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PROFILE="${1:-release}"
FLAGS=(--release)
if [[ "$PROFILE" == "dev" || "$PROFILE" == "debug" ]]; then
  FLAGS=()
fi

echo "OxideUtils — cargo build ${FLAGS[*]:-(debug)}  (config: oxideutils.toml)"
cargo build --workspace "${FLAGS[@]}"

if grep -qE '^\s*kernel\s*=\s*true' oxideutils.toml 2>/dev/null; then
  echo "build.kernel=true → building no_std core → target-nostd/"
  cargo build -p oxideutils-core "${FLAGS[@]}" \
    --no-default-features \
    --features "alloc,disasm,kernel" \
    --target-dir target-nostd
fi

echo "Done. Plan: target/oxideutils-build-plan.txt"
