#Requires -Version 7.0
param(
    [Parameter(Mandatory)][ValidateSet('joyword','joydeck','joysheet')][string]$Product,
    [Parameter(Mandatory)][string]$Installer,
    [Parameter(Mandatory)][string]$Archive,
    [switch]$SystemIntegration,
    [switch]$KeepInstalled
)
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../..')).Path
$testRoot = Join-Path $workspace "target/installer-smoke/$Product-$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
$install = Join-Path $testRoot '中文 installation'
$expected = Join-Path $testRoot 'expected'
if (![IO.Path]::GetFullPath($testRoot).StartsWith((Join-Path $workspace 'target') + [IO.Path]::DirectorySeparatorChar)) { throw 'Invalid test directory' }
$title = @{joyword='JoyWord';joydeck='JoyDeck';joysheet='Joysheet'}[$Product]
$registry = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\$title"
$shortcut = Join-Path ([Environment]::GetFolderPath('StartMenu')) "Programs/$title/$title.lnk"
if ($SystemIntegration -and ((Test-Path -LiteralPath $registry) -or (Test-Path -LiteralPath $shortcut))) { throw 'Existing installation must not be replaced by a smoke test' }
New-Item -ItemType Directory -Path $testRoot -Force | Out-Null
$archivePath = (Resolve-Path -LiteralPath $Archive).Path
Expand-Archive -LiteralPath $archivePath -DestinationPath $expected
$payload = if ($Product -eq 'joyword') { $expected } else { Join-Path $expected ([IO.Path]::GetFileNameWithoutExtension($archivePath)) }
$arguments = @('/S')
if (!$SystemIntegration) { $arguments += '/TEST' }
$arguments += "/D=$install"
$process = Start-Process -FilePath (Resolve-Path -LiteralPath $Installer).Path -ArgumentList $arguments -WindowStyle Hidden -PassThru -Wait
if ($process.ExitCode -ne 0) { throw "Installer failed: $($process.ExitCode)" }
foreach ($file in Get-ChildItem -LiteralPath $payload -Recurse -File) {
    $relative = [IO.Path]::GetRelativePath($payload, $file.FullName)
    $installed = Join-Path $install $relative
    if (!(Test-Path -LiteralPath $installed) -or (Get-FileHash -LiteralPath $installed).Hash -ne (Get-FileHash -LiteralPath $file.FullName).Hash) { throw "Installed payload differs: $relative" }
}
if (!(Test-Path -LiteralPath (Join-Path $install 'INSTALLER-INFO.json'))) { throw 'Missing installer provenance' }
if ($SystemIntegration) {
    if ((Get-ItemProperty -LiteralPath $registry).InstallLocation -ne $install -or !(Test-Path -LiteralPath $shortcut)) { throw 'Missing registry entry or Start menu shortcut' }
} elseif (!(Test-Path -LiteralPath (Join-Path $install ".$Product-install-smoke"))) { throw 'Missing isolated test marker' }
& (Join-Path $install "$Product-cli.exe") --help *> (Join-Path $testRoot 'installed-cli.log')
if ($LASTEXITCODE -ne 0) { throw 'Installed CLI failed' }
$sentinel = Join-Path $install 'user-owned.txt'
'preserve this unrelated file' | Set-Content -LiteralPath $sentinel
$report = @{product=$Product;installer=(Resolve-Path -LiteralPath $Installer).Path;archive=$archivePath;installation=$install;payloadMatchesArchive=$true;installedCli=$true;systemIntegration=[bool]$SystemIntegration;keptInstalled=[bool]$KeepInstalled}
if (!$KeepInstalled) {
    $process = Start-Process -FilePath (Join-Path $install 'Uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
    $deadline = [DateTime]::UtcNow.AddSeconds(30)
    while ((Test-Path -LiteralPath (Join-Path $install "$Product.exe")) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
    if ((Test-Path -LiteralPath (Join-Path $install "$Product.exe")) -or !(Test-Path -LiteralPath $sentinel)) { throw 'Uninstall failed or removed an unrelated file' }
    if ($SystemIntegration -and ((Test-Path -LiteralPath $registry) -or (Test-Path -LiteralPath $shortcut))) { throw 'Uninstall left system integration entries' }
    $report['unrelatedFilePreserved'] = $true
    $report['uninstalled'] = $true
}
$report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $testRoot 'report.json') -Encoding utf8
$report | ConvertTo-Json
