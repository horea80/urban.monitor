@echo off
rem deploy/cross/zigcxx.bat: compilator C++ pentru aarch64-unknown-linux-gnu (CXX_*), prin shim-ul cargo-zigbuild
cargo-zigbuild zig c++ -- -fno-sanitize=all -target aarch64-linux-gnu %*
