#Requires -Version 7.0
$ErrorActionPreference = 'Stop'
$report = [ordered]@{ os=[System.Runtime.InteropServices.RuntimeInformation]::OSDescription; architecture=[System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString(); powershell=$PSVersionTable.PSVersion.ToString() }
foreach ($tool in @('rustc','cargo','cmake','git')) {
    $command = Get-Command $tool -ErrorAction SilentlyContinue
    $report[$tool] = if ($command) { (& $command.Source --version | Select-Object -First 1) } else { 'missing from PATH' }
}
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
$report.visualStudio = if (Test-Path -LiteralPath $vswhere) { & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath } else { 'missing' }
if ($report.visualStudio -and $report.visualStudio -ne 'missing') {
    $bundledCmake=Join-Path $report.visualStudio 'Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe'
    if (Test-Path -LiteralPath $bundledCmake) { $report.bundledCmake=(& $bundledCmake --version | Select-Object -First 1) }
}
$report.installedTargets = @(& rustup target list --installed)
$report | ConvertTo-Json -Depth 5
if ($report.rustc -notmatch '^rustc 1\.98\.1 ') { throw 'Rust 1.98.1 is required by rust-toolchain.toml.' }
if (-not $report.visualStudio) { throw 'Visual Studio C++ build tools are required.' }
# scripts/dev.ps1 supplies the Visual Studio SDK and bundled CMake to Cargo.
