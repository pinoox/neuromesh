# NeuroMesh v0.8.1 benchmark runner (Benchmark A–F)
# Supports test2/test3 corpus compare via -Compare

param(
    [ValidateSet("A", "B", "C", "D", "E", "F", "all")]
    [string]$Suite = "A",
    [ValidateSet("test2", "test3", "")]
    [string]$Compare = ""
)

$ErrorActionPreference = "Stop"
$NeuromeshRoot = Split-Path -Parent $PSScriptRoot
$BenchRoot = if ($Compare -eq "test3") {
    "C:\projects\benchmark\nm_vs_cbm\test3"
} else {
    "C:\projects\benchmark\nm_vs_cbm\test2"
}

Write-Host "NeuroMesh v0.8.1 benchmarks — suite $Suite"
Write-Host "Repo: $NeuromeshRoot"
if ($Compare) { Write-Host "Compare baseline: $Compare ($BenchRoot)" }

Push-Location $NeuromeshRoot
try {
    cargo build --release -p neuromesh-cli 2>&1 | Write-Host
    $bin = Join-Path $NeuromeshRoot "target\release\neuromesh.exe"

    if ($Suite -eq "A" -or $Suite -eq "all") {
        Write-Host "`n=== Benchmark A: Regression ($Compare) ==="
        if (Test-Path $BenchRoot) {
            Push-Location $BenchRoot
            if (Test-Path ".\run-benchmark.ps1") {
                & .\run-benchmark.ps1
            } else {
                Write-Warning "run-benchmark.ps1 not found in $BenchRoot"
            }
            Pop-Location
        } else {
            Write-Warning "Benchmark corpus not found: $BenchRoot"
        }
    }

    Write-Host "`n=== Release gates (Benchmark A on current repo) ==="
    & $bin eval --release-gates 2>&1 | Write-Host

    if ($Compare -eq "test3" -and (Test-Path "$BenchRoot\results")) {
        Write-Host "`n=== test3 baseline comparison ==="
        $baseline = Get-ChildItem "$BenchRoot\results\benchmark_report*.md" | Sort-Object LastWriteTime -Descending | Select-Object -First 1
        if ($baseline) {
            Write-Host "Prior report: $($baseline.FullName)"
            Get-Content $baseline.FullName -TotalCount 40 | Write-Host
        }
    }

    if ($Suite -eq "F" -or $Suite -eq "all") {
        Write-Host "`n=== Benchmark F: Cursor-like (requires agent harness) ==="
        Write-Host 'Run external harness: test3\ or extend mcp_driver_v2.mjs'
    }
} finally {
    Pop-Location
}
