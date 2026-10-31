param([ValidateSet('build','check','run','package')][string]$Action = 'run')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        $installation = & $vswhere -latest -products '*' -property installationPath
        $cmakeBin = Join-Path $installation 'Common7/IDE/CommonExtensions/Microsoft/CMake/CMake/bin'
        if (Test-Path -LiteralPath (Join-Path $cmakeBin 'cmake.exe')) { $env:PATH = "$cmakeBin;$env:PATH" }
    }
}
switch ($Action) {
    build { cargo build --locked -p joydeck }
    check { cargo xtask check }
    run { cargo run --locked -p joydeck }
    package {
        python scripts/package-native.py --profile release
        if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    }
}
exit $LASTEXITCODE
