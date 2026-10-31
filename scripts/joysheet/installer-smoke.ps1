#Requires -Version 7.0
param([Parameter(Mandatory)][string]$Installer)
$ErrorActionPreference='Stop'
$workspace=(Resolve-Path -LiteralPath (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent)).Path
$install=Join-Path $workspace "artifacts/joysheet/install-smoke-$([DateTimeOffset]::UtcNow.ToUnixTimeMilliseconds())"
if(![IO.Path]::GetFullPath($install).StartsWith((Join-Path $workspace 'artifacts')+[IO.Path]::DirectorySeparatorChar)){throw 'Invalid test directory'}
$p=Start-Process -FilePath (Resolve-Path -LiteralPath $Installer).Path -ArgumentList @('/S','/TEST',"/D=$install") -WindowStyle Hidden -PassThru -Wait
if($p.ExitCode -ne 0){throw 'Installer failed'}
if(!(Test-Path -LiteralPath (Join-Path $install 'joysheet.exe')) -or !(Test-Path -LiteralPath (Join-Path $install '.joysheet-install-smoke'))){throw 'Installer did not extract its payload'}
# The test adds an unrelated file; uninstall must preserve it and user settings.
$sentinel=Join-Path $install 'user-owned.txt';'preserve me'|Set-Content -LiteralPath $sentinel
& pwsh.exe -NoProfile -File (Join-Path $PSScriptRoot 'desktop-smoke.ps1') -BinaryDirectory $install -Stage M1 -M8Task 1 *> (Join-Path $workspace 'artifacts/joysheet/installer-runtime.log')
if($LASTEXITCODE -ne 0){throw 'Installed app runtime regression failed'}
$p=Start-Process -FilePath (Join-Path $install 'Uninstall.exe') -ArgumentList '/S' -WindowStyle Hidden -PassThru -Wait
$deadline=[DateTime]::UtcNow.AddSeconds(20)
while((Test-Path -LiteralPath (Join-Path $install 'joysheet.exe')) -and [DateTime]::UtcNow -lt $deadline){Start-Sleep -Milliseconds 100}
if((Test-Path -LiteralPath (Join-Path $install 'joysheet.exe')) -or !(Test-Path -LiteralPath $sentinel)){throw 'Uninstaller removed wrong files or failed'}
@{passed=$true;installer=(Resolve-Path -LiteralPath $Installer).Path;installation=$install;nativeInstalledApp=$true;shortcutsAndRegistry='not modified by isolated TEST mode';unrelatedFilePreserved=$true}|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $workspace 'artifacts/joysheet/installer-smoke.json')
Write-Output 'Installer extraction, installed native app, and file-safe uninstall passed.'
