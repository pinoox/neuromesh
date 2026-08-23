$ErrorActionPreference = "Stop"
try {
    $OutputEncoding = [System.Text.Encoding]::UTF8
    [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
} catch {}

$Repo = "pinoox/neuromesh"
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs\neuromesh"
$BinPath = Join-Path $InstallDir "neuromesh.exe"

Write-Host @"
  _   _                      __  __           _     
 | \ | | ___ _   _ _ __ ___ |  \/  | ___  ___| |__  
 |  \| |/ _ \ | | | '__/ _ \| |\/| |/ _ \/ __| '_ \ 
 | |\  |  __/ |_| | | | (_) | |  | |  __/\__ \ | | |
 |_| \_|\___|\__,_|_|  \___/|_|  |_|\___||___/_| |_|
"@ -ForegroundColor Cyan

Write-Host "Biomimetic MCP Context Engine & Visual Runtime`n" -ForegroundColor Green

# 1. Fetch latest release from GitHub
Write-Host "Checking latest release on GitHub..." -ForegroundColor Gray
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

$TempZip = Join-Path $env:TEMP "neuromesh-windows-x86_64.zip"
$TempExtract = Join-Path $env:TEMP "neuromesh_extract"

Write-Host "Downloading precompiled Windows binary from: $DownloadUrl" -ForegroundColor Yellow

try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing
} catch {
    Write-Host "Release asset not ready yet, checking fallback..." -ForegroundColor Yellow
    throw $_
}

# 2. Extract and Install
if (Test-Path $TempExtract) { Remove-Item -Path $TempExtract -Recurse -Force }
New-Item -ItemType Directory -Path $TempExtract -Force | Out-Null
Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force

if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Get-Process neuromesh -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

Copy-Item -Path (Join-Path $TempExtract "neuromesh.exe") -Destination $BinPath -Force

# Also update .cargo/bin if user previously installed via cargo
$CargoBin = Join-Path $env:USERPROFILE ".cargo\bin\neuromesh.exe"
if (Test-Path (Join-Path $env:USERPROFILE ".cargo\bin")) {
    Copy-Item -Path (Join-Path $TempExtract "neuromesh.exe") -Destination $CargoBin -Force -ErrorAction SilentlyContinue
}

# Clean up temp
Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "`n[OK] NeuroMesh installed successfully to: $BinPath" -ForegroundColor Green

# 3. Add to User PATH if not present (Prepend so it takes priority)
$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike ("*" + $InstallDir + "*")) {
    Write-Host "Adding $InstallDir to User PATH..." -ForegroundColor Cyan
    $NewPath = $InstallDir + ";" + $UserPath
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = $InstallDir + ";" + $env:Path
    Write-Host "[OK] Added to PATH successfully!" -ForegroundColor Green
}

# 4. Verify installation
Write-Host "`nVerifying installation:" -ForegroundColor Cyan
& $BinPath --help | Select-Object -First 8

Write-Host "`nQuick Start:" -ForegroundColor Green
Write-Host "  1. Launch 3D Monitor:  neuromesh monitor (default http://127.0.0.1:8765; neuromesh port to change)" -ForegroundColor White
Write-Host "  2. Connect to IDE:     neuromesh connect" -ForegroundColor White
Write-Host "  3. Index Workspace:    neuromesh index" -ForegroundColor White
Write-Host "`n(Note: Restart your terminal/IDE for PATH changes to take full effect.)`n" -ForegroundColor Gray