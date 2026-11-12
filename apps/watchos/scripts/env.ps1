$projectRoot = Split-Path $PSScriptRoot -Parent
$pinnedRust = Join-Path $projectRoot '.tools\rustup\toolchains\1.99.1-x86_64-pc-windows-gnu\bin'
$localRust = Join-Path $projectRoot '.tools\rustup\toolchains\stable-x86_64-pc-windows-gnu\bin'
if (Test-Path $pinnedRust) { $localRust = $pinnedRust }
if (Test-Path $localRust) {
    $env:PATH = "$localRust;$(Join-Path $projectRoot '.tools\mingw64\bin');$env:PATH"
    $env:CARGO_HOME = Join-Path $projectRoot '.tools\cargo'
    $env:RUSTUP_HOME = Join-Path $projectRoot '.tools\rustup'
}
