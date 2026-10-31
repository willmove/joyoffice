#Requires -Version 7.0
$CargoArgs = @($args)
if ($CargoArgs.Count -gt 0 -and $CargoArgs[0] -eq 'clippy' -and $CargoArgs -notcontains '--') {
    $lintIndex = [Array]::IndexOf($CargoArgs, '-D')
    if ($lintIndex -gt 0) { $CargoArgs = @($CargoArgs[0..($lintIndex-1)]) + @('--') + @($CargoArgs[$lintIndex..($CargoArgs.Count-1)]) }
}
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
if (Test-Path -LiteralPath $vswhere) {
    $installation = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($installation) {
        $module = Join-Path $installation 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll'
        if (Test-Path -LiteralPath $module) {
            Import-Module $module
            Enter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
        }
        $cmakeBin = Join-Path $installation 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin'
        if (Test-Path -LiteralPath $cmakeBin) { $env:PATH = "$cmakeBin;$env:PATH" }
    }
}
if (-not (Get-Command cmake -ErrorAction SilentlyContinue)) { throw 'Install CMake or the Visual Studio CMake tools.' }
if (-not $CargoArgs) { $CargoArgs = @('run','--locked','-p','joysheet','--','--sample','sales') }
Push-Location (Split-Path $PSScriptRoot -Parent)
try { & cargo @CargoArgs; exit $LASTEXITCODE } finally { Pop-Location }
