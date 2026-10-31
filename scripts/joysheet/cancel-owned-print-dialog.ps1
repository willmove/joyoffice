#Requires -Version 7.0
# Integration test: invoke only Cancel on the helper child of this owned Joysheet process.
param([Parameter(Mandatory)][int]$OwnerProcessId,[Parameter(Mandatory)][string]$ScriptPath)
$ErrorActionPreference='Stop'
$deadline=[DateTime]::UtcNow.AddSeconds(15);$helper=$null
while([DateTime]::UtcNow -lt $deadline){
    $children=@(Get-CimInstance Win32_Process -Filter "ParentProcessId=$OwnerProcessId"|Where-Object {$_.Name -eq 'pwsh.exe' -and $_.CommandLine.Contains($ScriptPath)})
    if($children.Count -eq 1){$helper=Get-Process -Id $children[0].ProcessId -ErrorAction SilentlyContinue}
    if($helper){$helper.Refresh();if($helper.MainWindowHandle -ne 0){break}}
    Start-Sleep -Milliseconds 100
}
if(!$helper -or $helper.MainWindowHandle -eq 0){throw 'Owned native print helper dialog not found'}
$root=Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$binary=Join-Path $root 'artifacts/joysheet/CancelPrintDialog.exe'
$framework=Join-Path $env:WINDIR 'Microsoft.NET/Framework64/v4.0.30319'
$wpf=Join-Path $framework 'WPF'
$arguments=@('/nologo',('/out:'+$binary),('/reference:'+(Join-Path $wpf 'UIAutomationClient.dll')),('/reference:'+(Join-Path $wpf 'UIAutomationTypes.dll')),('/reference:'+(Join-Path $wpf 'WindowsBase.dll')),(Join-Path $PSScriptRoot 'CancelPrintDialog.cs'))
$compile=& (Join-Path $framework 'csc.exe') @arguments 2>&1
if($LASTEXITCODE -ne 0){throw "UIA integration probe compilation failed: $compile"}
# Full .NET Framework provides Win32 client-side UIA proxies; PowerShell 7 remains the host.
$reply=& $binary $helper.Id 2>&1
if($LASTEXITCODE -ne 0){throw "Native Cancel invocation failed: $reply"}
if(!$helper.WaitForExit(10000)){throw 'Print helper did not finish after Cancel'}
@{passed=$true;source='actual native PrintDialog Cancel through owned-process UIA InvokePattern';helperProcessId=$helper.Id;exitCode=$helper.ExitCode;queueSubmitted=$false}|ConvertTo-Json -Compress
