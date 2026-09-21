# deploy/cross/ziglink.ps1 — linker wrapper for aarch64-unknown-linux-gnu, called through ziglink.bat.
# dx links twice: once with explicit arguments and once through a response file named link_args.json.
# cargo-zigbuild's shim only expands response files named *linker-arguments (rustc's convention), so
# it would pass @link_args.json straight to zig, which then trips on rustc flags the shim normally
# filters (e.g. -Wl,--fix-cortex-a53-843419). We expand such files into explicit arguments here; the
# whole link line is short (fat LTO: one object plus a few rlibs), so no command-line limit is hit.
$expanded = New-Object System.Collections.Generic.List[string]
foreach ($a in $args) {
    $s = [string]$a
    if ($s.StartsWith('@') -and -not $s.EndsWith('linker-arguments')) {
        $raw = [IO.File]::ReadAllText($s.Substring(1))
        if ($env:URBAN_CROSS_DEBUG) { Copy-Item $s.Substring(1) (Join-Path $env:URBAN_CROSS_DEBUG 'link_args.captured') -Force }
        if ($raw.TrimStart().StartsWith('[')) {
            foreach ($x in (ConvertFrom-Json $raw)) { $expanded.Add([string]$x) }
        } else {
            foreach ($line in ($raw -split "`r?`n")) { if ($line -ne '') { $expanded.Add($line) } }
        }
    } else {
        $expanded.Add($s)
    }
}
& cargo-zigbuild zig cc -- -fno-sanitize=all -target aarch64-linux-gnu @expanded
exit $LASTEXITCODE
