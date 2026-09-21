@echo off
rem deploy/cross/zigranlib.bat: ranlib (RANLIB_*) pentru bibliotecile C statice
cargo-zigbuild zig ranlib -- %*
