#!/bin/bash
# deploy/build.sh — build-ul de producție pentru box (aarch64 Linux), rulat PE STAȚIE (ADR-0010).
# O singură comandă dx: clientul wasm + public/ ca de obicei, iar serverul cu `@server --platform server
# --features server --target aarch64-unknown-linux-gnu`, cross-compilat cu zig drept compilator C și
# linker (wrapper-ele din deploy/cross/, prin shim-ul cargo-zigbuild, care adaptează argumentele rustc).
# Serverul TREBUIE construit de dx: asset!() lasă în binar un placeholder pe care dx îl înlocuiește
# după link cu calea hash-uită; un server construit cu cargo direct servește <link href> cu textul
# placeholder și pagina rămâne fără stiluri.
# Ieșire: target/deploy/{server,public}, exact layout-ul din /opt/urban.
#
# Prerechizite (o dată): rustup target add aarch64-unknown-linux-gnu; winget install zig.zig;
#                        cargo install cargo-zigbuild --locked
set -euo pipefail
cd "$(dirname "$0")/.."

TARGET="aarch64-unknown-linux-gnu"
OUT="target/deploy"
DX_OUT="target/dx/urban-app/release/web"

for t in dx cargo-zigbuild zig cygpath powershell; do
  command -v "$t" >/dev/null 2>&1 || { echo "missing tool: $t (see the header of deploy/build.sh)" >&2; exit 1; }
done
rustup target list --installed | grep -qx "$TARGET" || { echo "missing rustup target: $TARGET (rustup target add $TARGET)" >&2; exit 1; }

# căi Windows cu /, pe care le acceptă și rustc, și cc-rs; linkerul trece prin ziglink.ps1 (vezi acolo de ce)
CROSS="$(cygpath -m "$PWD/deploy/cross")"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_GNU_LINKER="$CROSS/ziglink.bat"
export CC_aarch64_unknown_linux_gnu="$CROSS/zigcc.bat"
export CXX_aarch64_unknown_linux_gnu="$CROSS/zigcxx.bat"
export AR_aarch64_unknown_linux_gnu="$CROSS/zigar.bat"
export RANLIB_aarch64_unknown_linux_gnu="$CROSS/zigranlib.bat"

echo "▶ dx bundle --release --platform web, server for $TARGET (zig as C compiler and linker)"
# @server primește explicit platforma, feature-ul și target-ul: doar cu `@server --target`, dx 0.7.10
# construiește pentru aarch64 un binar cu feature-urile clientului (fără `server`), care moare la pornire.
dx bundle --release -p urban-app --platform web @server --platform server --features server --target "$TARGET"

rm -rf "$OUT" && mkdir -p "$OUT"
cp -r "$DX_OUT/public" "$OUT/public"
cp "$DX_OUT/server" "$OUT/server"
echo "✓ Built $OUT ($(du -sh "$OUT" | cut -f1)): $(file -b "$OUT/server" | cut -d, -f1-2)"
