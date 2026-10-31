#Requires -Version 7.0
# Read-only native dialog observation. No keyboard/mouse injection and no print job submission.
param([string]$Page='artifacts/joysheet/desktop/M6-pdf-page.png')
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$directory=Join-Path $root 'artifacts/joysheet/print-backend'
$null=New-Item -ItemType Directory -Path $directory -Force
Copy-Item -LiteralPath (Join-Path $root $Page) -Destination (Join-Path $directory 'page-1.png')
$helper=Join-Path $PSScriptRoot 'native-print.ps1'
$validation=& pwsh.exe -NoProfile -STA -File $helper -Directory $directory -Validate
if($LASTEXITCODE -ne 0){throw 'WPF print paginator failed'}
$info=[Diagnostics.ProcessStartInfo]::new('pwsh.exe')
$info.UseShellExecute=$false;$info.CreateNoWindow=$true;$info.RedirectStandardError=$true
foreach($arg in @('-NoProfile','-STA','-File',$helper,'-Directory',$directory)){$info.ArgumentList.Add($arg)}
$p=[Diagnostics.Process]::Start($info)
try {
    $deadline=[DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 200;$p.Refresh()
        if($p.HasExited){throw "Native print helper exited: $($p.StandardError.ReadToEnd())"}
    } while(!$p.MainWindowTitle -and [DateTime]::UtcNow -lt $deadline)
    if(!$p.MainWindowTitle){throw 'No native print dialog window observed'}
    @{paginator=($validation|ConvertFrom-Json);dialogOpened=$true;windowTitle=$p.MainWindowTitle;dialogCancellationVerified=$false;queueSubmissionVerified=$false;cleanup='terminated only the test-owned helper process'}|ConvertTo-Json -Depth 6|Set-Content (Join-Path $directory 'report.json')
    Get-Content (Join-Path $directory 'report.json')
} finally {if(!$p.HasExited){Stop-Process -Id $p.Id}}
