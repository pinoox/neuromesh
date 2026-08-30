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

Write-Host "NeuroMesh v0.8.6 — bundled MiniLM embed · MCP context engine`n" -ForegroundColor Green

Write-Host "Fetching latest release…" -ForegroundColor Gray
$DownloadUrl = "https://github.com/$Repo/releases/latest/download/neuromesh-windows-x86_64.zip"
$ReleaseTag = "latest"

try {
    $ReleaseInfo = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest" -UseBasicParsing
    if ($ReleaseInfo.tag_name) {
        $ReleaseTag = $ReleaseInfo.tag_name
        Write-Host "Release: $ReleaseTag" -ForegroundColor Green
    }
    if ($ReleaseInfo.assets) {
        $WinAsset = $ReleaseInfo.assets | Where-Object { $_.name -like "*windows-x86_64.zip" } | Select-Object -First 1
        if ($WinAsset) { $DownloadUrl = $WinAsset.browser_download_url }
    }
} catch {
    Write-Host "Using latest/download fallback…" -ForegroundColor Yellow
}

$TempZip = Join-Path $env:TEMP "neuromesh-windows-x86_64.zip"
$TempExtract = Join-Path $env:TEMP "neuromesh_extract"

Write-Host "Downloading pre-built binary…" -ForegroundColor Yellow
try {
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $TempZip -UseBasicParsing
} catch {
    Write-Host "Download failed. See https://github.com/$Repo/releases" -ForegroundColor Red
    throw
}

if (Test-Path $TempExtract) { Remove-Item -Path $TempExtract -Recurse -Force }
New-Item -ItemType Directory -Path $TempExtract -Force | Out-Null
Expand-Archive -Path $TempZip -DestinationPath $TempExtract -Force

if (!(Test-Path $InstallDir)) {
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
}

Get-Process neuromesh -ErrorAction SilentlyContinue | Stop-Process -Force -ErrorAction SilentlyContinue
Start-Sleep -Milliseconds 400

Copy-Item -Path (Join-Path $TempExtract "neuromesh.exe") -Destination $BinPath -Force
$NmxPath = Join-Path $InstallDir "nmx.exe"
Copy-Item -Path (Join-Path $TempExtract "neuromesh.exe") -Destination $NmxPath -Force

$BundledModels = Join-Path $TempExtract "models\minilm-multilingual-q"
if (Test-Path $BundledModels) {
    $DestModels = Join-Path $InstallDir "models\minilm-multilingual-q"
    New-Item -ItemType Directory -Path $DestModels -Force | Out-Null
    Copy-Item -Path (Join-Path $BundledModels "*") -Destination $DestModels -Recurse -Force
    Write-Host "[OK] MiniLM weights bundled next to binary" -ForegroundColor Green
}

$CargoBinDir = Join-Path $env:USERPROFILE ".cargo\bin"
if (Test-Path $CargoBinDir) {
    Copy-Item -Path (Join-Path $TempExtract "neuromesh.exe") -Destination (Join-Path $CargoBinDir "neuromesh.exe") -Force -ErrorAction SilentlyContinue
    Copy-Item -Path (Join-Path $TempExtract "neuromesh.exe") -Destination (Join-Path $CargoBinDir "nmx.exe") -Force -ErrorAction SilentlyContinue
}

Remove-Item -Path $TempZip -Force -ErrorAction SilentlyContinue
Remove-Item -Path $TempExtract -Recurse -Force -ErrorAction SilentlyContinue

Write-Host "`n[OK] Installed: $BinPath (alias: nmx.exe)" -ForegroundColor Green

$UserPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($UserPath -notlike ("*" + $InstallDir + "*")) {
    Write-Host "Adding $InstallDir to User PATH…" -ForegroundColor Cyan
    $NewPath = $InstallDir + ";" + $UserPath
    [Environment]::SetEnvironmentVariable("Path", $NewPath, "User")
    $env:Path = $InstallDir + ";" + $env:Path
}

try {
    $ver = & $BinPath -V 2>&1
    Write-Host "Version: $ver" -ForegroundColor White
} catch {}

Write-Host "`nQuick start:" -ForegroundColor Green
Write-Host "  1. neuromesh doctor       verify install" -ForegroundColor White
Write-Host "  2. neuromesh connect      wire Cursor / VS Code / Claude MCP" -ForegroundColor White
Write-Host "  3. neuromesh index        index your repo" -ForegroundColor White
Write-Host "  4. neuromesh monitor      3D galaxy UI -> http://127.0.0.1:8765" -ForegroundColor White
Write-Host "`nRestart your terminal/IDE for PATH changes.`n" -ForegroundColor Gray
