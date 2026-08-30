$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$Dest = Join-Path $Root "crates\neuromesh-embed\models\minilm-multilingual-q"
$Base = "https://huggingface.co/Qdrant/paraphrase-multilingual-MiniLM-L12-v2-onnx-Q/resolve/main"

New-Item -ItemType Directory -Path $Dest -Force | Out-Null

$files = @(
    "model_optimized.onnx",
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json"
)

foreach ($f in $files) {
    $out = Join-Path $Dest $f
    if (Test-Path $out) {
        Write-Host "  skip $f (exists)"
        continue
    }
    Write-Host "  fetch $f..."
    Invoke-WebRequest -Uri "$Base/$f" -OutFile $out -UseBasicParsing
}

Write-Host "MiniLM bundled at $Dest"
