#Requires -Version 7.0
$ErrorActionPreference='Stop'
$workspace=Split-Path $PSScriptRoot -Parent
Push-Location $workspace
try {
    $paths=@(& rg --files --hidden -g '!.git' -g '!target' -g '!dist' -g '!artifacts' -g '!docs/source-checkpoint.json' | Sort-Object)
    if ($LASTEXITCODE -ne 0) { throw 'Source enumeration failed.' }
    $records=@($paths | ForEach-Object { @{path=$_.Replace('\','/');sha256=(Get-FileHash -LiteralPath $_ -Algorithm SHA256).Hash.ToLowerInvariant()} })
    @{version='0.1.0';stage='M8-preview6';date='2026-10-30';files=$records;excluded=@('target','dist','artifacts','.git','docs/source-checkpoint.json')} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath docs/source-checkpoint.json -Encoding utf8
    Write-Output "Recorded $($records.Count) source/artifact hashes."
} finally { Pop-Location }


