param([switch]$LeaveRunning, [string]$Executable = 'target/debug/joydeck.exe')
$ErrorActionPreference = 'Stop'
Set-Location (Split-Path $PSScriptRoot -Parent)
New-Item -ItemType Directory -Force logs | Out-Null
$samples = @('tests/fixtures/native-20.joydeck','tests/fixtures/public-format-20.pptx')
$before = @($samples | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
# Interactive application; native windows are deliberately visible for acceptance.
$app = Start-Process -FilePath (Join-Path (Get-Location) $Executable) -WindowStyle Normal -ArgumentList '--viewer-smoke' -RedirectStandardOutput logs/m1.stdout.log -RedirectStandardError logs/m1.stderr.log -PassThru
$app.Id | Set-Content logs/m1.pid
$passed = $false
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(40)
    do {
        Start-Sleep -Milliseconds 250
        $app.Refresh()
        if ($app.HasExited) { throw "Viewer exited unexpectedly: $($app.ExitCode)" }
        $log = Get-Content logs/m1.stderr.log -Raw -ErrorAction SilentlyContinue
        if ($log -match 'M1_SMOKE failed=true|startup/runtime failure') { throw "Viewer smoke failed. See logs/m1.stderr.log" }
    } until (($log -match 'M1_SMOKE passed=true') -or [DateTime]::UtcNow -ge $deadline)
    foreach ($marker in @('native_open=true','native_next=true','last_page=true','rapid_navigation_no_stale=true','zoom_max=true','zoom_min=true','fit_resize=true','cancel_preserves_document=true','pptx_open_order=true','pptx_next_report=true','failure_preserves_document=true','M1_SMOKE passed=true')) {
        if (-not $log.Contains($marker)) { throw "Missing result: $marker" }
    }
    if ($app.MainWindowHandle -eq 0 -or -not $app.Responding) { throw 'Viewer native window is not responding' }
    $after = @($samples | ForEach-Object { (Get-FileHash -LiteralPath $_).Hash })
    if (Compare-Object $before $after) { throw 'Read-only sample hash changed' }
    Add-Content logs/m1.stderr.log 'M1_SMOKE source_hash_unchanged=true'
    $passed = $true
    Write-Output "M1 desktop smoke passed. PID=$($app.Id) RSS_MiB=$([math]::Round($app.WorkingSet64 / 1MB, 1))"
} finally {
    if ((-not $LeaveRunning -or -not $passed) -and -not $app.HasExited) { Stop-Process -Id $app.Id }
}
