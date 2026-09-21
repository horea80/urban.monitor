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
JOBS="${BUILD_JOBS:-8}"   # nuclee folosite de cargo/rustc (și de codegen-ul LLVM, prin jobserver); BUILD_JOBS=32 just build-cross

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
export CARGO_BUILD_JOBS="$JOBS"
# dx nu raportează un link eșuat: ziglink.ps1 scrie aici codul de ieșire al fiecărui link, îl verificăm la final
export URBAN_LINK_MARKER="$PWD/target/deploy-link.log"
rm -f "$URBAN_LINK_MARKER"

# Profiluri: dx impune `lto = true` prin `--config` pentru ambele jumătăți și bate manifestul, așa că
# le relaxăm tot prin `--config`, mai târziu pe linia de comandă (dx desparte --cargo-args ca un shell).
# Fără LTO și cu 16 unități de codegen, un build incremental (o schimbare în crates/app) durează ~19 s
# în loc de ~90 s: wasm 22 s → 4 s, server 80 s → 11 s. Costul: wasm 1,02 → 1,19 MB (servit comprimat),
# server 12,7 → 15,3 MB. Dacă dimensiunea wasm devine o problemă, scoate override-ul de la @client.
WASM_PROFILE='--config=profile.wasm-release.lto=false --config=profile.wasm-release.codegen-units=16'
SERVER_PROFILE='--config=profile.server-release.lto=false --config=profile.server-release.codegen-units=16'

echo "▶ dx bundle --release: wasm client + server for $TARGET (zig as C compiler and linker, $JOBS jobs)"
# @server primește explicit platforma, feature-ul și target-ul: doar cu `@server --target`, dx 0.7.10
# construiește pentru aarch64 un binar cu feature-urile clientului (fără `server`), care moare la pornire.
# dx nu curăță public/assets de bundle-urile vechi (fiecare wasm/js cu hash-ul lui rămâne); îl refacem de la zero.
rm -rf "$DX_OUT/public"
dx bundle --release -p urban-app \
  @client --platform web --cargo-args="$WASM_PROFILE" \
  @server --platform server --features server --target "$TARGET" --cargo-args="$SERVER_PROFILE" &
dx_pid=$!
# dx tace cât rustc compilează ultimul crate; un rând la 15 s arată că mai lucrează (verificăm la fiecare secundă)
start=$SECONDS
while kill -0 "$dx_pid" 2>/dev/null; do
  sleep 1
  el=$((SECONDS - start))
  if [ "$el" -gt 0 ] && [ $((el % 15)) -eq 0 ]; then echo "  … still building (${el}s)"; fi
done
wait "$dx_pid"
if [ -f "$URBAN_LINK_MARKER" ] && grep -qv "^exit=0 " "$URBAN_LINK_MARKER"; then
  echo "✗ the server link failed (see above); dx did not report it:" >&2; cat "$URBAN_LINK_MARKER" >&2; exit 1
fi

rm -rf "$OUT" && mkdir -p "$OUT"
cp -r "$DX_OUT/public" "$OUT/public"
cp "$DX_OUT/server" "$OUT/server"
echo "✓ Built $OUT ($(du -sh "$OUT" | cut -f1)): $(file -b "$OUT/server" | cut -d, -f1-2)"
