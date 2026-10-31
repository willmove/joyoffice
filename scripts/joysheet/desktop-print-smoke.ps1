#Requires -Version 7.0
# Uses Joysheet's production controller and observes only the helper owned by this test application.
param([string]$Report='artifacts/joysheet/desktop/report.json')
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$reportData=Get-Content -LiteralPath (Join-Path $root $Report) -Raw|ConvertFrom-Json
$appProcess=Get-CimInstance Win32_Process -Filter "ProcessId=$($reportData.processId)"
if(!$appProcess -or $appProcess.ExecutablePath -ne $reportData.binary -or $appProcess.CommandLine -notmatch "--control\s+$($reportData.port)\b"){throw 'The test-owned application is no longer running'}
$cli=Join-Path (Split-Path $reportData.binary) 'joysheet-cli.exe'
function Send-PrintControl([string]$Method,[hashtable]$Values=@{}) {
    $priorConfig=$env:JOYSHEET_CONFIG_DIR
    try {
        $env:JOYSHEET_CONFIG_DIR=Join-Path $root "artifacts/joysheet/desktop/isolated-profile-$($reportData.port)"
        $response=& $cli send $reportData.port $Method ($Values|ConvertTo-Json -Depth 10 -Compress) 2>&1
        if($LASTEXITCODE -ne 0){throw ($response -join "`n")}
        return ($response -join "`n")|ConvertFrom-Json
    } finally {$env:JOYSHEET_CONFIG_DIR=$priorConfig}
}
$null=Send-PrintControl 'ui.command' @{command='file.print';params=@{}}
$helper=$null
try {
    $deadline=[DateTime]::UtcNow.AddSeconds(30)
    do {
        Start-Sleep -Milliseconds 200
        $view=Send-PrintControl 'ui.inspect'
        if($view.printPreview -and $view.printStatus -eq 'dialog open') {
            $scriptPath=Join-Path (Split-Path $view.printPreview.pdf) 'print.ps1'
            $candidates=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($reportData.processId)" | Where-Object {$_.Name -eq 'pwsh.exe' -and $_.CommandLine.Contains($scriptPath)})
            if($candidates.Count -eq 1){$helper=Get-Process -Id $candidates[0].ProcessId}
        }
        if($helper){$helper.Refresh()}
    } while((!$helper -or !$helper.MainWindowTitle) -and [DateTime]::UtcNow -lt $deadline)
    if(!$helper -or !$helper.MainWindowTitle){throw 'The desktop did not open the native print dialog'}
    $evidence=@{passed=$true;binary=$reportData.binary;sha256=(Get-FileHash -LiteralPath $reportData.binary).Hash;source='GPUI file.print → guarded engine PDF export → native rasterization → WPF PrintDialog';windowTitle=$helper.MainWindowTitle;dialogOpened=$true;cancelVerified=$false;queueSubmissionVerified=$false;cleanup='only the test-owned print helper was terminated'}
    $evidence|ConvertTo-Json -Depth 10|Set-Content (Join-Path $root 'docs/joysheet/evidence/M6-desktop-print.json')
    $evidence|ConvertTo-Json -Depth 10
} finally {
    if($helper -and !$helper.HasExited){Stop-Process -Id $helper.Id}
}
