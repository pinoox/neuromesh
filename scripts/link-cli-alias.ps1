# Create nmx.exe as a hard link to neuromesh.exe (fallback: copy).
# Run after: cargo build --release -p neuromesh-cli --features embeddings
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$ReleaseDir = Join-Path $Root "target\release"
$Source = Join-Path $ReleaseDir "neuromesh.exe"
$Alias = Join-Path $ReleaseDir "nmx.exe"

if (-not (Test-Path $Source)) {
    Write-Error "Missing $Source — build first: cargo build --release -p neuromesh-cli --features embeddings"
    exit 1
}

if (Test-Path $Alias) { Remove-Item $Alias -Force }

try {
    New-Item -ItemType HardLink -Path $Alias -Target $Source -ErrorAction Stop | Out-Null
    Write-Host "Linked: $Alias -> $Source (hard link)"
} catch {
    Copy-Item -Path $Source -Destination $Alias -Force
    Write-Host "Copied: $Alias (hard link unavailable)"
}
