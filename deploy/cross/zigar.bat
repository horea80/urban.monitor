@echo off
rem deploy/cross/zigar.bat: arhivator (AR_*) pentru bibliotecile C statice (SQLite bundled, ring)
cargo-zigbuild zig ar -- %*
