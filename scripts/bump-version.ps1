[CmdletBinding()]
param (
    [Parameter(Mandatory = $true, Position = 0)]
    [string]$NewVersion,

    [Parameter(Mandatory = $false, Position = 1)]
    [string]$OldVersion
)

$ErrorActionPreference = "Stop"
$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Split-Path -Parent $ScriptDir
Set-Location $RepoRoot

# Normalize versions (strip leading 'v' if provided)
$NewVersion = $NewVersion.TrimStart('v')

if (-not $OldVersion) {
    $cargoContent = Get-Content (Join-Path $RepoRoot "Cargo.toml") -Raw
    if ($cargoContent -match '\[workspace\.package\][\s\S]*?version\s*=\s*"([^"]+)"') {
        $OldVersion = $Matches[1]
    } else {
        Write-Error "Could not automatically determine old version from Cargo.toml. Please supply -OldVersion."
        exit 1
    }
} else {
    $OldVersion = $OldVersion.TrimStart('v')
}

if ($OldVersion -eq $NewVersion) {
    Write-Warning "Current version is already $NewVersion. Nothing to bump."
    exit 0
}

Write-Host "Bumping NeuroMesh version: $OldVersion -> $NewVersion" -ForegroundColor Cyan

function Replace-InFile {
    param (
        [string]$RelativePath,
        [scriptblock]$Transform
    )
    $fullPath = Join-Path $RepoRoot $RelativePath
    if (-not (Test-Path $fullPath)) {
        Write-Warning "File not found: $RelativePath"
        return
    }
    $raw = [System.IO.File]::ReadAllText($fullPath)
    $updated = & $Transform $raw
    if ($raw -ne $updated) {
        $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
        [System.IO.File]::WriteAllText($fullPath, $updated, $utf8NoBom)
        Write-Host "  Updated: $RelativePath" -ForegroundColor Green
    } else {
        Write-Host "  No changes needed: $RelativePath" -ForegroundColor DarkGray
    }
}

# 1. Cargo.toml
Replace-InFile "Cargo.toml" {
    param($s)
    $s -replace '(\[workspace\.package\][\s\S]*?version\s*=\s*)"[^"]+"', "`$1`"$NewVersion`""
}

# 2. package.json (VS Code extension)
Replace-InFile "editors/vscode-neuromesh/package.json" {
    param($s)
    $s -replace '("version":\s*)"[^"]+"', "`$1`"$NewVersion`""
}

# 3. install.sh & install.ps1
Replace-InFile "install.sh" {
    param($s)
    $s -replace "v$OldVersion", "v$NewVersion"
}
Replace-InFile "install.ps1" {
    param($s)
    $s -replace "v$OldVersion", "v$NewVersion"
}

# 4. MCP protocol & descriptors
Replace-InFile "crates/neuromesh-mcp/src/protocol.rs" {
    param($s)
    $s -replace "NeuroMesh MCP v$OldVersion", "NeuroMesh MCP v$NewVersion"
}
Replace-InFile "crates/neuromesh-mcp/src/descriptors.rs" {
    param($s)
    $s -replace "Default \(v$OldVersion\):", "Default (v$NewVersion):"
}

# 5. README & Docs
$docFiles = @(
    "README.md",
    "docs/index.html",
    "docs/assets/i18n.js",
    "docs/README.md",
    "docs/agent-guide.md",
    "docs/agent-rule.mdc",
    "docs/api.md",
    "docs/architecture.md",
    "docs/cli.md",
    "docs/configuration.md",
    "docs/engines.md",
    "docs/graph-proxy.md",
    "docs/mcp.md",
    "editors/vscode-neuromesh/README.md"
)

foreach ($doc in $docFiles) {
    Replace-InFile $doc {
        param($s)
        $out = $s -replace "v$OldVersion", "v$NewVersion"
        $out = $out -replace "نسخه $OldVersion", "نسخه $NewVersion"
        $out
    }
}

# 6. Check CHANGELOG.md
$changelogPath = Join-Path $RepoRoot "docs/CHANGELOG.md"
if (Test-Path $changelogPath) {
    $changelog = [System.IO.File]::ReadAllText($changelogPath)
    if ($changelog -notmatch "##\s*$NewVersion") {
        Write-Warning "docs/CHANGELOG.md does not yet have an entry for ## $NewVersion."
        Write-Warning "Remember to add the release notes under ## $NewVersion in docs/CHANGELOG.md!"
    }
}

Write-Host "`nVersion bump to $NewVersion completed successfully." -ForegroundColor Cyan
Write-Host "Next recommended steps:" -ForegroundColor Yellow
Write-Host "  1. Update docs/CHANGELOG.md with release notes"
Write-Host "  2. cargo check --workspace (updates Cargo.lock)"
Write-Host "  3. cargo test --workspace"
Write-Host "  4. git commit -am `"chore(release): bump version to $NewVersion`""
Write-Host "  5. git tag -a v$NewVersion -m `"Release v$NewVersion`""
Write-Host "  6. git push origin main --tags"
