#Requires -Version 7.0
param([switch]$Validation)
$ErrorActionPreference='Stop'
$workspace=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
Push-Location $workspace
try {
    $options=@('scripts/joysheet/package.py','--checkpoint-only')
    if($Validation){$options+='--validation'}
    & python @options
    exit $LASTEXITCODE
} finally { Pop-Location }
