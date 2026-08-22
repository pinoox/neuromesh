# ==============================================================================
# 🌿 NeuroMesh V2 — Zero-Prerequisite Universal Installer (Windows PowerShell)
# ==============================================================================
# Usage:
#   irm https://raw.githubusercontent.com/pinoox/neuromesh/main/install.ps1 | iex
# ==============================================================================

$ErrorActionPreference = "Stop"

$Repo = "pinoox/neuromesh"
$InstallDir = "$env:LOCALAPPDATA\Programs\neuromesh"
$BinPath = "$InstallDir\neuromesh.exe"

Write-Host @"
  _   _                      __  __           _     
 | \ | | ___ _   _ _ __ ___ |  \/  | ___  ___| |__  
 |  \| |/ _ \ | | | '__/ _ \| |\/| |/ _ \/ __| '_ \ 
 | |\  |  __/ |_| | | | (_) | |  | |  __/\__ \ | | |
 |_| \_|\___|\__,_|_|  \___/|_|  |_|\___||___/_| |_|
"@ -ForegroundColor Cyan

Write-Host "🌿 Biomimetic MCP Context Engine & Visual Runtime`n" -ForegroundColor Green

# 1. Fetch latest release from GitHub
Write-Host " Checking latest release on GitHub..." -ForegroundColor Gray
$DownloadUrl = "https://github.com/$Repo/releases/latest/download/neuromesh-windows-x86_64.zip"

try {
    $ReleaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing -ErrorAction SilentlyContinue
    if ($ReleaseInfo.assets) {
        $WinAsset = $ReleaseInfo.assets | Where-Object { $_.name -like "*windows-x86_64.zip" } | Select-Object -First 1
        if ($WinAsset) {
            $DownloadUrl = $WinAsset.browser_download_url
        }
    }
} catch {}

$TempZip = "$env:TEMP\neuromesh-windows-x86_64.zip"
$TempExtract = "$env:TEMP\neuromesh_extract"

Write-Host " Downloading precompiled Windows binary from: $DownloadUrl" -ForegroundColor Yellow

try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing
} catch {
    Write-Host "⚠️ Direct release asset not ready yet, trying fallback repository..." -ForegroundColor Yellow
    # If release is still building, try cargo install alternative
    Write-Host "You can also install via Cargo: cargo install --git https://github.com/$Repo.git neuromesh-cli --bin neuromesh" -ForegroundColor Gray
    throw $_
}

# 2. Extract and Install
if (Test-Path $TempExtract) { Remove-Item -Path $TempExtract -Recurse -Force }
New-Item -ItemType Directory -Path $TempExtract -Force | Out-Null
Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force

if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Copy-Item -Path "$TempExtract\neuromesh.exe" -Destination $BinPath -Force

# Clean up temp
Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "`n✓ NeuroMesh installed successfully to: $BinPath" -ForegroundColor Green

# 3. Add to User PATH if not present
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike "*$InstallDir*") {
    Write-Host " Adding $InstallDir to User PATH..." -ForegroundColor Cyan
    $NewPath = "$UserPath;$InstallDir"
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = "$env:Path;$InstallDir"
    Write-Host "✓ Added to PATH successfully!" -ForegroundColor Green
}

# 4. Verify installation
Write-Host "`n Verifying installation:" -ForegroundColor Cyan
& $BinPath --help | Select-Object -First 8

Write-Host "`n🚀 Quick Start:" -ForegroundColor Green
Write-Host "  1. Launch 3D Monitor:  neuromesh monitor (Open http://127.0.0.1:8765)" -ForegroundColor White
Write-Host "  2. Connect to IDE:     neuromesh connect" -ForegroundColor White
Write-Host "  3. Index Workspace:    neuromesh index" -ForegroundColor White
Write-Host "`n(Note: Restart your terminal/IDE for PATH changes to take full effect.)`n" -ForegroundColor Gray