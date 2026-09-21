@echo off
rem deploy/cross/zigcc.bat: compilator C si linker pentru aarch64-unknown-linux-gnu, prin shim-ul cargo-zigbuild (filtreaza argumentele rustc pentru zig cc)
cargo-zigbuild zig cc -- -fno-sanitize=all -target aarch64-linux-gnu %*
