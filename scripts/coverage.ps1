param(
    [string]$OutputDir = "report/coverage",
    [switch]$NoHtml
)

$ErrorActionPreference = "Stop"

function Require-Command {
    param([string]$Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found in PATH."
    }
}

Require-Command cargo

$llvmCov = & cargo llvm-cov --version 2>$null
if ($LASTEXITCODE -ne 0) {
    throw "cargo-llvm-cov is not installed. Install it with: cargo install cargo-llvm-cov"
}

New-Item -ItemType Directory -Force -Path $OutputDir | Out-Null

$summaryPath = Join-Path $OutputDir "summary.txt"
$jsonPath = Join-Path $PWD "coverage.json"
$lcovPath = Join-Path $PWD "lcov.info"

Write-Host "Running cargo llvm-cov summary..."
& cargo llvm-cov --workspace --all-features --summary-only | Tee-Object -FilePath $summaryPath
if ($LASTEXITCODE -ne 0) {
    throw "cargo llvm-cov summary failed."
}

Write-Host "Exporting coverage.json..."
& cargo llvm-cov --workspace --all-features --json --output-path $jsonPath
if ($LASTEXITCODE -ne 0) {
    throw "cargo llvm-cov json export failed."
}

Write-Host "Exporting lcov.info..."
& cargo llvm-cov --workspace --all-features --lcov --output-path $lcovPath
if ($LASTEXITCODE -ne 0) {
    throw "cargo llvm-cov lcov export failed."
}

if (-not $NoHtml) {
    Write-Host "Generating HTML report..."
    & cargo llvm-cov --workspace --all-features --html --output-dir $OutputDir
    if ($LASTEXITCODE -ne 0) {
        throw "cargo llvm-cov html export failed."
    }
}

Write-Host "Coverage artifacts:"
Write-Host "  Summary: $summaryPath"
Write-Host "  JSON:    $jsonPath"
Write-Host "  LCOV:    $lcovPath"
if (-not $NoHtml) {
    Write-Host "  HTML:    $OutputDir/index.html"
}
