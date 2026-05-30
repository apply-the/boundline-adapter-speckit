Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path -Parent $PSScriptRoot
Set-Location $repoRoot

& cargo llvm-cov clean --workspace
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

& cargo llvm-cov --workspace --all-features --lcov --output-path lcov.info @args
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}