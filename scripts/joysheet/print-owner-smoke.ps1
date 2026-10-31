#Requires -Version 7.0
param([string]$BinaryDirectory='target/debug')
$ErrorActionPreference='Stop'
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
& (Join-Path $PSScriptRoot 'desktop-smoke.ps1') -BinaryDirectory $BinaryDirectory -Stage M1 -KeepOpen *> (Join-Path $root 'artifacts/joysheet/print-owner-start.log')
if($LASTEXITCODE -ne 0){throw 'Print owner test could not start the native desktop'}
$r=Get-Content (Join-Path $root 'artifacts/joysheet/desktop/report.json') -Raw|ConvertFrom-Json
$app=Get-Process -Id $r.processId
$cli=Join-Path (Split-Path $r.binary) 'joysheet-cli.exe'
$previous=$env:JOYSHEET_CONFIG_DIR
$env:JOYSHEET_CONFIG_DIR=Join-Path $root "artifacts/joysheet/desktop/isolated-profile-$($r.port)"
$helper=$null
function Call-Owner([string]$Method,[hashtable]$Params=@{}){
    $result=& $cli send $r.port $Method ($Params|ConvertTo-Json -Depth 8 -Compress) 2>&1
    if($LASTEXITCODE -ne 0){throw "$Method failed: $result"}
    ($result -join "`n")|ConvertFrom-Json
}
try{
    $null=Call-Owner 'ui.command' @{command='file.print';params=@{}}
    $deadline=[DateTime]::UtcNow.AddSeconds(30)
    do{
        $v=Call-Owner 'ui.inspect'
        if($v.printWork.dialogOpen -and $v.printPreview){
            $scriptPath=Join-Path (Split-Path $v.printPreview.pdf) 'print.ps1'
            $owned=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$($app.Id)"|Where-Object {$_.Name -eq 'pwsh.exe' -and $_.CommandLine.Contains($scriptPath)})
            if($owned.Count -eq 1){$helper=Get-Process -Id $owned[0].ProcessId;$helper.Refresh()}
        }
        if($helper -and $helper.MainWindowHandle -ne 0){break}
        Start-Sleep -Milliseconds 100
    }while([DateTime]::UtcNow -lt $deadline)
    if(!$helper -or $helper.MainWindowHandle -eq 0){throw 'Native print dialog was not opened'}
    $null=Call-Owner 'app.quit' @{force=$true}
    if(!$app.WaitForExit(15000)){throw 'Application did not quit with a native print dialog open'}
    if(!$helper.WaitForExit(10000)){throw 'Print helper outlived its application owner'}
    @{passed=$true;source='production app.quit → native print helper owner-exit watcher';binary=$r.binary;binarySha256=$r.binarySha256;nativeDialogOpened=$true;ownerExited=$true;helperExited=$true;queueSubmitted=$false}|ConvertTo-Json|Set-Content (Join-Path $root 'artifacts/joysheet/print-owner-report.json') -Encoding utf8NoBOM
    Get-Content (Join-Path $root 'artifacts/joysheet/print-owner-report.json')
}finally{
    if($helper -and !$helper.HasExited){Stop-Process -Id $helper.Id}
    if(!$app.HasExited){Stop-Process -Id $app.Id}
    $env:JOYSHEET_CONFIG_DIR=$previous
}

