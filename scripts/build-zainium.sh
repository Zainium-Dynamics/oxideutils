#!/bin/sh
# build-zainium.sh — cross-compile OxideUtils for ZainiumOS's musl target
# and install it into the syshub, as a (parallel, non-destructive) binutils
# replacement.
#
# Matches the convention of packages/musl-zainium/build-zainium.sh and
# packages/gcc-15.3.0-zainium/build-gcc-zainium.sh: musl + gcc + oxideutils
# all live under one syshub/x86_64-zainium-linux-musl/ tree.
#
# musl-zainium is SHARED-only (--disable-static) — see packages/musl-zainium
# — so this cross-compiles OxideUtils as a dynamically-linked musl binary,
# not the fully-static default Rust gives you for *-linux-musl targets.
#
# Usage: ./scripts/build-zainium.sh [x86_64] [DESTDIR]
#   CC=...             override the musl cross-gcc (default: the zainium one)
set -e

ARCH="${1:-x86_64}"
case "$ARCH" in
  x86_64) TARGET_TRIPLET=x86_64-zainium-linux-musl ; RUST_TARGET=x86_64-unknown-linux-musl ;;
  *) echo "usage: $0 x86_64 [DESTDIR]" >&2; exit 1 ;;
esac

DESTDIR="${2:-/run/media/alizain/ZAINIUM_DRIVE/zairoot}"
SYSHUB="/overlayer/syshub"
PREFIX="${DESTDIR}${SYSHUB}/${TARGET_TRIPLET}"
SYSHUB_BIN="${DESTDIR}${SYSHUB}/bin"

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

# The real target cross-gcc (built by packages/gcc-15.3.0-zainium). Override
# with CC=... if it's not built/working yet on this machine.
CC="${CC:-${PREFIX}/bin/${TARGET_TRIPLET}-gcc}"
if [ ! -x "$CC" ]; then
    ALT="${SYSHUB_BIN}/${TARGET_TRIPLET}-gcc"
    [ -x "$ALT" ] && CC="$ALT"
fi
if [ ! -x "$CC" ]; then
    echo "[ERROR] no working ${TARGET_TRIPLET}-gcc found (checked ${PREFIX}/bin and ${SYSHUB_BIN})." >&2
    echo "        Build packages/gcc-15.3.0-zainium first, or pass CC=<musl-gcc>." >&2
    exit 1
fi

echo "[*] target      = ${RUST_TARGET}  (triple: ${TARGET_TRIPLET})"
echo "[*] CC           = ${CC}"
echo "[*] install to   = ${PREFIX}/bin"

export CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$CC"
# Rust's *-linux-musl targets default to fully-static (crt-static). musl-zainium
# is shared-only, so force a dynamically-linked musl binary to match.
export RUSTFLAGS="${RUSTFLAGS:-} -C target-feature=-crt-static"

cargo build --workspace --release --target "${RUST_TARGET}"

BIN_SRC="target/${RUST_TARGET}/release"
mkdir -p "${PREFIX}/bin"

# Every oxide-* tool binary + the oxideutils multicall dispatcher, whichever
# ones oxideutils.toml enabled (disabled ones still build as tiny stub bins —
# install whatever actually got produced).
installed=""
for bin in oxideutils oxide-ar oxide-ranlib oxide-nm oxide-objdump oxide-objcopy oxide-readelf \
           oxide-size oxide-strings oxide-strip oxide-addr2line oxide-cxxfilt oxide-elfedit; do
    if [ -x "${BIN_SRC}/${bin}" ]; then
        install -m 755 "${BIN_SRC}/${bin}" "${PREFIX}/bin/${bin}"
        installed="${installed} ${bin}"
    fi
done

# `+` isn't a valid rustc crate name, so oxide-cxxfilt is the buildable
# target — symlink GNU's actual binary name to it.
if [ -x "${PREFIX}/bin/oxide-cxxfilt" ]; then
    ln -sfn oxide-cxxfilt "${PREFIX}/bin/oxide-c++filt"
    installed="${installed} oxide-c++filt(symlink)"
fi

echo
echo "installed under ${PREFIX}/bin/:${installed}"
echo
echo "NOT linked into ${SYSHUB_BIN}/${TARGET_TRIPLET}-{ar,nm,...} — GNU binutils"
echo "there is left untouched on purpose. Once you're happy oxideutils is a"
echo "correct drop-in, point packages/gcc-15.3.0-zainium's build at it with:"
echo "  OXIDEUTILS_BIN=${PREFIX}/bin ./build-gcc-zainium.sh configure"
