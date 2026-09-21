@echo off
rem deploy/cross/ziglink.bat: linkerul pentru aarch64-unknown-linux-gnu (CARGO_TARGET_*_LINKER). dx trimite argumentele
rem printr-un fisier @link_args.json pe care shim-ul cargo-zigbuild nu il recunoaste; ziglink.ps1 il expandeaza.
powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File "%~dp0ziglink.ps1" %*
exit /b %ERRORLEVEL%
