param([switch]$LeaveRunning)
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
New-Item -ItemType Directory -Force logs | Out-Null
# This launches the interactive desktop application requested for acceptance.
$process = Start-Process -FilePath (Join-Path (Get-Location) 'target/debug/joydeck.exe') -WindowStyle Normal -ArgumentList '--smoke' -RedirectStandardOutput logs/smoke.stdout.log -RedirectStandardError logs/smoke.stderr.log -PassThru
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    do {
        Start-Sleep -Milliseconds 250
        $process.Refresh()
        if ($process.HasExited) { throw "JoyDeck exited unexpectedly: $($process.ExitCode)" }
        $log = Get-Content logs/smoke.stderr.log -Raw -ErrorAction SilentlyContinue
    } until (($log -match 'Diagnostic worker settled at 100%') -or [DateTime]::UtcNow -ge $deadline)
    foreach ($marker in @('utf16_commit=true','composition_cancel=true','modal_open=true','second_window_closed=true','Diagnostic worker settled at 100%')) {
        if (-not $log.Contains($marker)) { throw "Missing native smoke result: $marker" }
    }
    if ($process.MainWindowHandle -eq 0 -or -not $process.Responding) { throw 'Native window is not responding' }
    Write-Output "Native GPUI smoke passed. PID=$($process.Id)"
    $process.Id | Set-Content logs/app.pid
} finally {
    if (-not $LeaveRunning -and -not $process.HasExited) { Stop-Process -Id $process.Id }
}
