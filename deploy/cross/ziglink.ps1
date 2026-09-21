# deploy/cross/ziglink.ps1 — linker wrapper for aarch64-unknown-linux-gnu, called through ziglink.bat.
# dx links twice: once with explicit arguments and once through a response file it names
# link_args.json — one argument per line, GNU-escaped, i.e. exactly the format of rustc's own
# response files. cargo-zigbuild's shim only expands response files whose name ends in
# `linker-arguments` (rustc's convention) and would hand @link_args.json to zig untouched, so zig
# trips on rustc flags the shim normally filters (e.g. -Wl,--fix-cortex-a53-843419). We copy the
# file under a recognised name and pass @thatfile; the shim filters it and zig reads it.
# Never expand it into explicit arguments: a thin-LTO link lists thousands of object files, far
# beyond the Windows command-line limit.
#
# URBAN_LINK_MARKER (optional): path of a file where we append "exit=<code>" after linking, so the
# build script can tell a failed link from a silent success (dx does not surface linker failures).
$args2 = New-Object System.Collections.Generic.List[string]
foreach ($a in $args) {
    $s = [string]$a
    if ($s.StartsWith('@') -and -not $s.EndsWith('linker-arguments')) {
        $src = $s.Substring(1)
        $dst = "$src-linker-arguments"
        Copy-Item $src $dst -Force
        if ($env:URBAN_CROSS_DEBUG) { Copy-Item $src (Join-Path $env:URBAN_CROSS_DEBUG 'link_args.captured') -Force }
        $args2.Add("@$dst")
    } else {
        $args2.Add($s)
    }
}
& cargo-zigbuild zig cc -- -fno-sanitize=all -target aarch64-linux-gnu @args2
$code = $LASTEXITCODE
if ($env:URBAN_LINK_MARKER) { Add-Content -Path $env:URBAN_LINK_MARKER -Value "exit=$code args=$($args.Count)" }
exit $code
