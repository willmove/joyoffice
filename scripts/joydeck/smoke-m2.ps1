param([switch]$LeaveRunning, [string]$Executable = 'target/debug/joydeck.exe')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path (Split-Path $PSScriptRoot -Parent) -Parent)
$app = Start-Process -FilePath (Join-Path (Get-Location) $Executable) -WindowStyle Normal -ArgumentList '--editor-smoke' -RedirectStandardError logs/m2.stderr.log -RedirectStandardOutput logs/m2.stdout.log -PassThru
$app.Id | Set-Content logs/m2.pid
$passed = $false
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(25)
    do {
        Start-Sleep -Milliseconds 250
        $app.Refresh()
        if ($app.HasExited) { throw 'Editor exited during smoke' }
        $log = Get-Content logs/m2.stderr.log -Raw -ErrorAction SilentlyContinue
        if ($log -match 'startup/runtime failure|M2_SMOKE error=|save_reopen_recovery=false') { throw $log }
    } until ($log -match 'M2_SMOKE complete=true' -or [DateTime]::UtcNow -gt $deadline)
    foreach ($marker in @('properties_close_cancel=true','ui_save_clean=true','save_reopen_recovery=true','drag_scale_rotate_cancel_single_history=true','marquee_multiselect_reorder=true','overwrite_old_snapshot_stays_dirty=true','save_then_close=true','discard_then_close=true','global_quit_dirty_guard=true','click_jitter_and_page_drag_cancel=true','complete=true')) { if (-not $log.Contains($marker)) {throw "Missing $marker"} }
    if (-not $app.Responding -or $app.MainWindowHandle -eq 0) {throw 'Native editor not responding'}
    $passed = $true
    Write-Output "M2 native editor smoke passed. PID=$($app.Id)"
} finally { if ((-not $LeaveRunning -or -not $passed) -and -not $app.HasExited) {Stop-Process -Id $app.Id} }
