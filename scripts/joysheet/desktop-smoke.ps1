#Requires -Version 7.0
param([string]$BinaryDirectory = 'target\debug', [ValidateSet('M0','M1','M2','M3')][string]$Stage = 'M3', [ValidateRange(0,4)][int]$M4Task=0, [ValidateRange(0,4)][int]$M5Task=0,[ValidateRange(0,4)][int]$M6Task=0, [ValidateRange(0,4)][int]$M7Task=0, [ValidateRange(0,4)][int]$M8Task=0, [ValidateRange(0,4)][int]$WindowsTask=0, [ValidateRange(1,4)][int]$WindowsFrom=1, [ValidateRange(0,4)][int]$WindowsQualityTask=0, [switch]$SaveDialog, [switch]$Performance, [switch]$FormulaCases, [switch]$StressCases, [int[]]$StressSizes=@(10000,100000), [ValidateSet('on','off')][string]$PerformanceRecovery='on', [ValidateRange(1,5)][int]$PerformanceRepeats=1, [ValidateRange(0,20)][int]$PerformanceWarmup=5, [switch]$KeepOpen)
$ErrorActionPreference = 'Stop'
$workspace = Split-Path (Split-Path $PSScriptRoot -Parent) -Parent
$binaryRoot = [System.IO.Path]::GetFullPath($(if([IO.Path]::IsPathRooted($BinaryDirectory)){$BinaryDirectory}else{Join-Path $workspace $BinaryDirectory}))
$app = Join-Path $binaryRoot 'joysheet.exe'
$cli = Join-Path $binaryRoot 'joysheet-cli.exe'
if (-not (Test-Path -LiteralPath $app)) { throw "Build first: pwsh -File scripts/joysheet/dev.ps1 build --features joysheet/automation -p joysheet -p joysheet-cli" }
$artifacts = Join-Path $workspace 'artifacts\joysheet\desktop'
New-Item -ItemType Directory -Path $artifacts -Force | Out-Null
$priorConfig = $env:JOYSHEET_CONFIG_DIR
$probe = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Loopback, 0)
$probe.Start()
$port = $probe.LocalEndpoint.Port
$probe.Stop()
$env:JOYSHEET_CONFIG_DIR = Join-Path $artifacts "isolated-profile-$port"
# This is the interactive application under test; GPUI does not continually paint a hidden window.
$started = Start-Process -FilePath $app -ArgumentList @('--sample','sales','--control',"$port",'--diagnostics') -WorkingDirectory $binaryRoot -WindowStyle Normal -PassThru -RedirectStandardOutput (Join-Path $artifacts 'stdout.log') -RedirectStandardError (Join-Path $artifacts 'stderr.log')
$checks = [System.Collections.Generic.List[string]]::new()
function Send-Control([string]$Method, [hashtable]$Parameters = @{}) {
    $json = ConvertTo-Json -InputObject $Parameters -Depth 20 -Compress
    if($json.Length -gt 16000){
        $file=Join-Path $artifacts "params-$port.json";$json|Set-Content -LiteralPath $file -Encoding utf8NoBOM
        try {$reply=& $cli send $port $Method "@$file" 2>&1}finally{Remove-Item -LiteralPath $file -ErrorAction SilentlyContinue}
    }else{$reply = & $cli send $port $Method $json 2>&1}
    if ($LASTEXITCODE -ne 0) { throw "$Method failed: $($reply -join "`n")" }
    return (($reply -join "`n") | ConvertFrom-Json)
}
function Wait-View([scriptblock]$Predicate, [string]$Label) {
    $deadline = [DateTime]::UtcNow.AddSeconds(20)
    while ([DateTime]::UtcNow -lt $deadline) {
        try { $view = Send-Control 'ui.inspect'; if (& $Predicate $view) { return $view } } catch { if ($started.HasExited) { throw "App exited: $(Get-Content (Join-Path $artifacts 'stderr.log') -Raw)" } }
        Start-Sleep -Milliseconds 80
    }
    throw "Timed out: $Label; last view: $($view | ConvertTo-Json -Depth 10 -Compress)"
}
function Assert-Cell([string]$Cell, $Expected) {
    $actual = Send-Control 'cell.get' @{cell=$Cell}
    if ($actual.value -ne $Expected) { throw "$Cell expected '$Expected', got '$($actual.value)'" }
}
function Type-Cell([string]$Cell,[string]$Text) {
    $null = Send-Control 'ui.cell.click' @{cell=$Cell}
    $null = Send-Control 'ui.text' @{text=$Text}
    $null = Send-Control 'ui.key' @{key='enter'}
    $null = Wait-View { param($v) $null -eq $v.editor } 'edit completion'
}
function Write-Report {
    $performance = Send-Control 'ui.perf'
    $report = @{passed=$true;stage=$Stage;m4Task=$M4Task;m5Task=$M5Task;m6Task=$M6Task;m7Task=$M7Task;m8Task=$M8Task;windowsTask=$WindowsTask;windowsQualityTask=$WindowsQualityTask;windowsFrom=$WindowsFrom;saveDialog=($SaveDialog -or $M4Task -ge 3);checks=$checks.ToArray();performance=$performance;nativeImeVerified=$false;eventSource='production GPUI synthetic input and GPU readback in native Windows process';platform='Windows x64';binary=$app;binarySha256=(Get-FileHash -LiteralPath $app -Algorithm SHA256).Hash.ToLowerInvariant();processId=$started.Id;port=$port}
    $report | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $artifacts 'report.json') -Encoding utf8
    $report | ConvertTo-Json -Depth 20
}
try {
    $initial = Wait-View { param($v) $v.grid.height -gt 100 -and $v.perf.frame -gt 1 } 'first native frame'
    if (-not $initial.capabilities.screenshot) { throw 'Build with --features automation for GPU screenshots.' }
    $null = Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'sales.png')}
    $checks.Add('native GPUI window and sales screenshot')
    if ($Stage -eq 'M0') { Write-Report; return }
    $null = Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null = Wait-View { param($v) $v.windows.Count -ge 2 -and $v.grid.height -gt 100 } 'new workbook window'
    if ($Stage -eq 'M3') {
    Type-Cell 'A1' '中文报表'
    Assert-Cell 'A1' '中文报表'
    Type-Cell 'B1' '10'
    Type-Cell 'C1' '=B1*2'
    Assert-Cell 'C1' 20
    $checks.Add('GPUI pointer and text events; Unicode; formula editing and recalculation')
    Type-Cell 'B1' '20'
    Assert-Cell 'C1' 40
    $null = Send-Control 'ui.key' @{key='z';cmd=$true}
    Assert-Cell 'C1' 20
    $null = Send-Control 'ui.key' @{key='y';cmd=$true}
    Assert-Cell 'C1' 40
    $checks.Add('keyboard undo and redo')
    # Feed composition through the same GPUI input handler as the native IME.
    # Candidate positioning and OS composition still require a human/native run.
    $null = Send-Control 'ui.cell.click' @{cell='D1'}
    $null = Send-Control 'ui.ime.mark' @{text='中文组合态'}
    $null = Wait-View {param($v) $v.composing -and $v.editor.text -eq '中文组合态'} 'marked text'
    $null = Send-Control 'ui.key' @{key='enter'}
    $composing = Send-Control 'ui.inspect'
    $composing | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $artifacts 'composition-before.json')
    if ($null -eq $composing.editor) { throw 'Enter committed a marked composition.' }
    Assert-Cell 'D1' $null
    $null = Send-Control 'ui.ime.unmark'
    $null = Send-Control 'ui.key' @{key='enter'}
    $compositionAfter = Wait-View {param($v) $null -eq $v.editor} 'composition commit'
    $compositionAfter | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $artifacts 'composition-after.json')
    Assert-Cell 'D1' '中文组合态'
    $checks.Add('GPUI marked composition blocks premature spreadsheet submission (synthetic)')
    $null = Send-Control 'ui.cell.click' @{cell='E1'}
    $null = Send-Control 'ui.text' @{text='共享草稿'}
    $null = Send-Control 'ui.edit.begin' @{inline=$false}
    $shared = Send-Control 'ui.inspect'
    if ($shared.formulaText -ne '共享草稿' -or $shared.cellEditorText -ne $shared.formulaText) { throw 'Formula/cell drafts diverged.' }
    $null=Send-Control 'ui.key' @{key='f6'}
    if(!(Send-Control 'ui.inspect').focus.name){throw 'F6 formula to name failed'}
    $null=Send-Control 'ui.key' @{key='f6'}
    $null=Send-Control 'ui.key' @{key='f6'}
    if(!(Send-Control 'ui.inspect').focus.sheets){throw 'F6 stable ribbon to sheets failed'}
    $null=Send-Control 'ui.key' @{key='f6'}
    if(!(Send-Control 'ui.inspect').focus.grid){throw 'F6 sheets to grid failed'}
    $null=Send-Control 'ui.key' @{key='f6'}
    $focusCycle=Send-Control 'ui.inspect'
    if(!$focusCycle.focus.formula -or $focusCycle.editor.text -ne '共享草稿'){throw 'F6 draft retention failed'}
    $checks.Add('F6 formula/name/stable ribbon/sheet tabs/grid focus cycle preserves the shared draft')

    $null = Send-Control 'ui.key' @{key='escape'}
    Assert-Cell 'E1' $null
    $checks.Add('shared formula/cell draft and Escape cancellation')
    } else {
        $null = Send-Control 'engine.execute' @{command='range.setValues';params=@{range='A1';values=@(,@('中文报表',20,'=B1*2'))}}
        Assert-Cell 'C1' 40
        $checks.Add('engine workbook data and formula preview')
    }
    if ($Stage -in @('M2','M3')) {
        $null = Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='T1000000'}}
        $far = Wait-View {param($v) $v.selection -eq 'T1000000' -and $v.scroll.y -gt 1000000} 'millionth row viewport'
        if ($far.perf.paintedCells -gt 4096) { throw 'Virtual grid painted too many cells.' }
        $null = Send-Control 'ui.cell.click' @{cell='T1000000'}
        $null = Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='A1'}}
        $null = Wait-View {param($v) $v.selection -eq 'A1' -and $v.scroll.y -eq 0} 'return to origin'
        $null = Send-Control 'ui.key' @{key='right'}
        $null = Wait-View {param($v) $v.selection -eq 'B1'} 'arrow navigation'
        $null = Send-Control 'ui.key' @{key='down';shift=$true}
        $null = Wait-View {param($v) $v.selection -eq 'B1:B2'} 'keyboard range extension'
        $null = Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='A1'}}
        $null = Wait-View {param($v) $v.selection -eq 'A1'} 'reset selection'
        $checks.Add('native millionth-row go-to, bounded painting and hit testing')
    }
    $null = Send-Control 'engine.execute' @{command='edit.paste';params=@{at='A3';text="项目`t金额`n测试`t42"}}
    Assert-Cell 'B4' 42
    if ($Stage -eq 'M3') {
        $null = Send-Control 'ui.cell.click' @{cell='G1'}
        $null = Send-Control 'ui.text' @{text='=SUM('}
        $null = Send-Control 'ui.cell.drag' @{from='B1';to='B4'}
        $picked = Send-Control 'ui.inspect'
        if ($picked.editor.text -ne '=SUM(B1:B4') { throw "Range picking failed: $($picked.editor.text)" }
        $null = Send-Control 'ui.text' @{text=')'}
        $null = Send-Control 'ui.key' @{key='enter'}
        $null = Wait-View {param($v) $null -eq $v.editor} 'reference formula submission'
        Assert-Cell 'G1' 62
        $null = Send-Control 'ui.cell.click' @{cell='G2'}
        $null = Send-Control 'ui.text' @{text='=SU'}
        $null = Send-Control 'ui.key' @{key='tab'}
        $completion = Send-Control 'ui.inspect'
        if ($completion.editor.text -ne '=SUM(') { throw "Formula completion failed: $($completion.editor.text)" }
        $null = Send-Control 'ui.text' @{text='B1)'}
        $null = Send-Control 'ui.key' @{key='enter'}
        $null = Wait-View {param($v) $null -eq $v.editor} 'completed function submission'
        Assert-Cell 'G2' 20
        $checks.Add('formula range picking and registered function completion')
        $null = Send-Control 'engine.execute' @{command='selection.set';params=@{range='B1:C1'}}
        $null = Send-Control 'ui.key' @{key='c';cmd=$true}
        $null = Send-Control 'ui.cell.click' @{cell='B6'}
        $null = Send-Control 'ui.key' @{key='v';cmd=$true}
        $null = Wait-View {param($v) -not $v.error -and $v.editor -eq $null} 'system clipboard paste'
        Assert-Cell 'C6' 40
        Type-Cell 'B6' '7'
        Assert-Cell 'C6' 14
        $checks.Add('native clipboard keyboard copy/paste preserves relative formulas')
    }
    $null = Send-Control 'engine.execute' @{command='home.bold';params=@{range='A1:C1'}}
    $null = Send-Control 'engine.execute' @{command='view.freezeTopRow';params=@{}}
    $null = Send-Control 'engine.execute' @{command='view.zoom';params=@{percent=125}}
    $null = Wait-View { param($v) $null -eq $v.editor } 'format/freeze/zoom render'
    $checks.Add('TSV paste; formatting; freeze; zoom')
    if ($Stage -eq 'M3') {
        $null = Send-Control 'ui.dialog' @{name='home.fontSize'}
        $null = Send-Control 'ui.dialog.set' @{field='size';value=14}
        $null = Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'format-dialog.png')}
        $null = Send-Control 'ui.dialog.confirm'
        $null = Wait-View {param($v) $v.dialog -eq '' -and -not $v.error} 'GPUI parameter form'
        $checks.Add('GPUI formatting form and command submission')
    }
    $workbook = Join-Path $artifacts 'desktop-roundtrip.xlsx'
    $null = Send-Control 'app.save' @{path=$workbook}
    if (-not (Test-Path -LiteralPath $workbook)) { throw 'Saved workbook missing.' }
    if ($Stage -eq 'M3') {
        $null = Send-Control 'ui.set' @{autosave=$true;persist=$false}
        Type-Cell 'F1' '自动保存'
        $null = Wait-View {param($v) -not ($v.windows | Where-Object window -eq $v.window).dirty} 'automatic XLSX save'
        $null = Send-Control 'ui.set' @{autosave=$false;persist=$false}
        $checks.Add('debounced automatic save of an existing XLSX')
        Type-Cell 'F2' '关闭取消保留'
        $null = Send-Control 'ui.key' @{key='w';cmd=$true}
        $null = Wait-View {param($v) $v.dialog -eq 'saveChanges'} 'save changes prompt'
        $null = Send-Control 'ui.dialog.cancel'
        Assert-Cell 'F2' '关闭取消保留'
        $null = Send-Control 'app.save' @{path=$workbook}
        $checks.Add('cancelled close retains workbook edits')
    }
    $null = Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}
    $null = Send-Control 'app.open' @{path=$workbook}
    $null = Wait-View { param($v) $v.windows | Where-Object path -eq $workbook } 'reopened native workbook window'
    Assert-Cell 'A1' '中文报表'
    Assert-Cell 'C1' 40
    $null = Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'roundtrip.png')}
    $checks.Add('XLSX save/close/reopen and native GPU screenshot')
    if ($Stage -eq 'M3') {
        $requests = @(
            @{jsonrpc='2.0';id=1;method='initialize';params=@{protocolVersion='2025-03-26';capabilities=@{};clientInfo=@{name='Joysheet smoke';version='0.1.0'}}},
            @{jsonrpc='2.0';id=2;method='tools/call';params=@{name='get_cell';arguments=@{cell='C1'}}},
            @{jsonrpc='2.0';id=3;method='tools/call';params=@{name='screenshot';arguments=@{path=(Join-Path $artifacts 'mcp.png')}}}
        )
        $lines = @($requests | ForEach-Object { ConvertTo-Json -InputObject $_ -Depth 20 -Compress })
        $replies = $lines | & $cli mcp --connect "$port"
        if ($LASTEXITCODE -ne 0) { throw 'MCP bridge exited unsuccessfully.' }
        $parsed = @($replies | ForEach-Object { $_ | ConvertFrom-Json })
        if ($parsed.Count -ne 3 -or ($parsed | Where-Object { $_.error -or $_.result.isError })) { throw "MCP bridge failed: $replies" }
        if (-not (Test-Path -LiteralPath (Join-Path $artifacts 'mcp.png'))) { throw 'MCP screenshot missing.' }
        $checks.Add('live stdio MCP handshake, cell query and native screenshot')
    }
    if ($Stage -eq 'M3') {
        $origin = Send-Control 'ui.inspect'
        $second = Send-Control 'engine.execute' @{command='view.newWindow';params=@{}}
        $null = Wait-View {param($v) $v.window -eq $second.window} 'shared workbook native window'
        $null = Send-Control 'ui.cell.click' @{cell='H1'}
        $null = Send-Control 'ui.text' @{text='第二视图草稿'}
        $null = Send-Control 'engine.execute' @{command='window.activate';params=@{window=$origin.window}}
        $null = Wait-View {param($v) $v.window -eq $origin.window} 'original native window'
        Assert-Cell 'H1' $null
        $null = Send-Control 'ui.edit.commit' @{window=$second.window}
        $null = Wait-View {param($v) $null -eq $v.editor} 'targeted draft commit'
        Assert-Cell 'H1' '第二视图草稿'
        $null = Send-Control 'engine.execute' @{command='window.close';params=@{window=$second.window;force=$true}}
        $checks.Add('independent native views, retained background draft and explicit-window commit')
    }
    if ($StressCases) { . (Join-Path $PSScriptRoot 'stress-cases.ps1') }
    if ($Performance) {
        $benchmarkMachine=@{coreTracing=(Test-Path Env:GRIDCRAFT_PROFILE);os=[Environment]::OSVersion.VersionString;cpu=@(Get-CimInstance Win32_Processor | ForEach-Object Name);logicalProcessors=[Environment]::ProcessorCount;binarySha256=(Get-FileHash -LiteralPath $app -Algorithm SHA256).Hash.ToLowerInvariant()}
        $null=Send-Control 'ui.set' @{recovery=($PerformanceRecovery -eq 'on');persist=$false}
        $measurements = [System.Collections.Generic.List[object]]::new()
        foreach($iteration in 1..$PerformanceRepeats){
        foreach ($rowCount in @(10000,100000)) {
            $csv = Join-Path $artifacts "benchmark-$rowCount.csv"
            $writer = [System.IO.StreamWriter]::new($csv,$false,[System.Text.UTF8Encoding]::new($false))
            try { for ($row=1; $row -le $rowCount; $row++) { $writer.WriteLine("$row,2,3,4,5,6,7,8,9,10,11,12,13,14,15,16,17,18,19,20") } } finally { $writer.Dispose() }
            $timer = [System.Diagnostics.Stopwatch]::StartNew()
            $openResult = Send-Control 'app.open' @{path=$csv}
            $loaded = Wait-View {param($v) ($v.windows | Where-Object window -eq $v.window).path -eq $csv} 'benchmark CSV'
            $openMs = $timer.Elapsed.TotalMilliseconds
            $engineMs = $loaded.commandMs
            $null = Send-Control 'ui.perf' @{reset=$true}
            for ($i=0; $i -lt 60; $i++) {
                $null = Send-Control 'ui.scroll' @{x=($loaded.grid.x+200);y=($loaded.grid.y+200);dy=-150;dx=0}
            }
            $stats = Send-Control 'ui.perf' @{raw=$true}
            $null = Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference="T$rowCount"}}
            $farView = Wait-View {param($v) $v.selection -eq "T$rowCount"} 'last data row'
            $null = Send-Control 'ui.cell.click' @{cell="T$rowCount"}
            Assert-Cell "T$rowCount" 20
            $null=Send-Control 'ui.perf' @{reset=$true}
            for($warm=0;$warm -lt $PerformanceWarmup;$warm++){$null=Send-Control 'ui.cell.click' @{cell="T$rowCount"};$null=Send-Control 'ui.edit.text' @{text="=$warm+1"};$null=Send-Control 'ui.edit.commit';$null=Send-Control 'ui.awaitIdle'}
            $null=Send-Control 'ui.perf' @{reset=$true}
            for($edit=1;$edit -le 30;$edit++){
                $null=Send-Control 'ui.cell.click' @{cell="T$rowCount"}
                $null=Send-Control 'ui.edit.text' @{text="=$edit*2"}
                $null=Send-Control 'ui.edit.commit'
                $null=Send-Control 'ui.awaitIdle'
            }
            $edits=Send-Control 'ui.perf' @{raw=$true}
            if($edits.editSamples -ne 30 -or $edits.stages.editCount -ne 30 -or $edits.stages.pending -ne 0 -or $edits.stages.discarded -ne 0){throw 'Missing or incorrectly fenced edit-response samples'}
            if($stats.paintedCells -gt 2000){throw 'Grid paint grew beyond the visible viewport'}
            foreach($sample in $edits.stages.edits){
                foreach($phase in @('totalMs','preflightMs','queueMs','engineMs','snapshotMs','publishMs','uiPollMs','uiSyncMs','frameWaitMs','geometryMs','layoutMs','paintMs','textMs','scrollbarMs')){
                    if($null -eq $sample.$phase -or [double]$sample.$phase -lt 0 -or [double]::IsNaN([double]$sample.$phase) -or [double]::IsInfinity([double]$sample.$phase)){throw "Invalid phase $phase"}
                }
            }
            $started.Refresh()
            $lowDependencyWorkingSet=$started.WorkingSet64
            # Separate a range aggregate from the low-dependency interactive gate.
            $null=Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='U1'}}
            $null=Wait-View {param($v) $v.selection -eq 'U1'} 'aggregate cell'
            $null=Send-Control 'ui.perf' @{reset=$true}
            for($aggregateEdit=1;$aggregateEdit -le 10;$aggregateEdit++){
                $null=Send-Control 'ui.cell.click' @{cell='U1'}
                $null=Send-Control 'ui.edit.text' @{text="=SUM(A1:A$rowCount)+$aggregateEdit"}
                $null=Send-Control 'ui.edit.commit'
                $null=Send-Control 'ui.awaitIdle'
            }
            $aggregate=Send-Control 'ui.perf' @{raw=$true}
            if($aggregate.stages.editCount -ne 10 -or $aggregate.stages.pending -ne 0 -or $aggregate.stages.discarded -ne 0){throw 'Missing aggregate phase samples'}
            Assert-Cell 'U1' ([double]$rowCount*($rowCount+1)/2+10)
            $complex=@()
            if($FormulaCases){
                $sum=[double]$rowCount*($rowCount+1)/2
                foreach($case in @(
                    @{name='conditional';formula="SUMIFS(A1:A$rowCount,B1:B$rowCount,2)";expected=$sum},
                    @{name='sumproduct';formula="SUMPRODUCT(A1:A$rowCount,B1:B$rowCount)";expected=2*$sum},
                    @{name='nested';formula="IF(SUM(A1:A$rowCount)>0,SUM(A1:A$rowCount)+SUM(B1:B$rowCount),0)";expected=$sum+2*$rowCount}
                )){
                    $null=Send-Control 'ui.perf' @{reset=$true}
                    for($n=1;$n -le 10;$n++){
                        $null=Send-Control 'ui.cell.click' @{cell='U1'}
                        $null=Send-Control 'ui.edit.text' @{text="=$($case.formula)+$n"}
                        $null=Send-Control 'ui.edit.commit';$null=Send-Control 'ui.awaitIdle'
                    }
                    $sample=Send-Control 'ui.perf' @{raw=$true}
                    if($sample.stages.editCount -ne 10 -or $sample.stages.pending -ne 0 -or $sample.stages.discarded -ne 0){throw "Missing complex formula samples: $($case.name)"}
                    Assert-Cell 'U1' ($case.expected+10)
                    if(@($sample.stages.edits | Where-Object {$_.formulasEvaluated -ne 1}).Count){throw "Changed complex formula evaluation count"}
                    $complex+=@{name=$case.name;samples=$sample}
                }
                foreach($n in 2..9){$null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell="U$n";input="=SUM(A1:A$rowCount)+$n"}}}
                $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='U10';input='=SUM(U2:U9)'}}
                $null=Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='A1'}}
                $null=Wait-View {param($v) $v.selection -eq 'A1'} 'fanout source'
                $null=Send-Control 'ui.perf' @{reset=$true}
                for($n=1;$n -le 10;$n++){
                    $null=Send-Control 'ui.cell.click' @{cell='A1'}
                    $null=Send-Control 'ui.edit.text' @{text=([string]($n+1))}
                    $null=Send-Control 'ui.edit.commit';$null=Send-Control 'ui.awaitIdle'
                }
                $sample=Send-Control 'ui.perf' @{raw=$true}
                if($sample.stages.editCount -ne 10 -or $sample.stages.pending -ne 0 -or $sample.stages.discarded -ne 0){throw 'Missing fanout samples'}
                Assert-Cell 'U10' (8*($sum+10)+44)
                if(@($sample.stages.edits | Where-Object {$_.formulasEvaluated -ne 10}).Count){throw "Changed fanout evaluation count"}
                $complex+=@{name='fanout';samples=$sample}
            }
            $started.Refresh()
            $measurements.Add(@{complex=$complex;workingSetAfterLowDependencyBytes=$lowDependencyWorkingSet;aggregateResponse=$aggregate;machine=$benchmarkMachine;display=$loaded.display;grid=$loaded.grid;iteration=$iteration;recoveryEnabled=($PerformanceRecovery -eq 'on');engineProfile=(Send-Control 'engine.profile');warmup=$PerformanceWarmup;openTiming=$openResult.timing;rows=$rowCount;columns=20;openToViewMs=$openMs;openEngineCommandMs=$engineMs;workingSetBytes=$started.WorkingSet64;paint=$stats;editResponse=$edits;gpuFrameTimeMeasured=$false;inputLatencyMeasured=$false})
            $null = Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}
        }
        }
        $measurements.ToArray() | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath (Join-Path $artifacts 'performance.json') -Encoding utf8
        $checks.Add('native 10000x20 and 100000x20 CSV viewing and bounded scroll paint samples; identity-fenced edit phases and correct range aggregates')
    }
    if ($M4Task -ge 1) {
        Type-Cell 'A8' '中文自动换行测试 ABC 123'
        foreach ($spec in @(@('home.fontSize','size',16),@('home.numberFormat','code','0.00'),@('home.fillColor','color','#E3F2E7'),@('home.borders','preset','all'))) {
            $null=Send-Control 'ui.dialog' @{name=$spec[0]}
            $null=Send-Control 'ui.dialog.set' @{field=$spec[1];value=$spec[2]}
            $null=Send-Control 'ui.dialog.confirm'
            $null=Wait-View {param($v) $v.dialog -eq '' -and -not $v.error} 'format application'
        }
        $null=Send-Control 'engine.execute' @{command='selection.set';params=@{range='A8'}}
        foreach ($cmd in @('home.wrapText','home.alignTop','home.underline')) {
            $null=Send-Control 'ui.dialog' @{name=$cmd}
            $null=Send-Control 'ui.dialog.confirm'
            $null=Wait-View {param($v) $v.dialog -eq '' -and -not $v.error} 'format toggle'
        }
        $style=Send-Control 'cell.get' @{cell='A8'}
        if (-not $style.style.align.wrap -or $style.style.align.v -ne 'Top') {throw 'Wrap/alignment style was not applied.'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M4-format.png')}
        $checks.Add('M4.1 format forms, wrap, vertical alignment and decoration')
    }
    if ($M4Task -ge 2) {
        $null=Send-Control 'ui.dialog' @{name='home.insertSheet'}
        $null=Send-Control 'ui.dialog.set' @{field='name';value='M4工作表'}
        $null=Send-Control 'ui.dialog.confirm'
        $null=Wait-View {param($v) ($v.windows | Where-Object window -eq $v.window).sheet -eq 1} 'sheet creation'
        Type-Cell 'A1' '工作表操作保留数据'
        $null=Send-Control 'ui.dialog' @{name='sheet.rename'}
        $null=Send-Control 'ui.dialog.set' @{field='name';value='改名报表'}
        $null=Send-Control 'ui.dialog.confirm'
        $null=Wait-View {param($v) $v.dialog -eq '' -and -not $v.error} 'sheet rename'
        foreach ($cmd in @('home.insertRows','home.deleteRows','sheet.hide','sheet.unhide')) {
            $null=Send-Control 'ui.dialog' @{name=$cmd}
            $null=Send-Control 'ui.dialog.confirm'
            $null=Wait-View {param($v) $v.dialog -eq '' -and -not $v.error} $cmd
        }
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet='改名报表'}}
        Assert-Cell 'A1' '工作表操作保留数据'
        $checks.Add('M4.2 sheet add/rename/hide/unhide and row operations')
    }
    if ($M4Task -ge 3) {
        $blocked=Join-Path $artifacts 'blocked-save.xlsx'
        New-Item -ItemType Directory -Path $blocked -Force | Out-Null
        $null=Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='A1'}}
        $null=Wait-View {param($v) $v.selection -eq 'A1' -and $v.scroll.y -eq 0} 'save target viewport'
        $null=Send-Control 'ui.cell.click' @{cell='A1'}
        $null=Send-Control 'ui.edit.text' @{text='保存失败仍保留内容'}
        $null=Send-Control 'ui.command' @{command='file.save';params=@{path=$blocked}}
        $null=Wait-View {param($v) $v.error -and $null -eq $v.editor} 'failed save status'
        Assert-Cell 'A1' '保存失败仍保留内容'
        $alphaPath=Join-Path $artifacts 'alpha-report.xlsx'
        $null=Send-Control 'ui.command' @{command='file.save';params=@{path=$alphaPath}}
        $null=Wait-View {param($v) -not ($v.windows | Where-Object window -eq $v.window).dirty -and $v.recentFiles -contains $alphaPath} 'successful save and recent file'
        $checks.Add('M4.3 draft submission, failed save preservation, retry and recent files')
    }
    if ($M4Task -ge 4) {
        $current=Send-Control 'ui.inspect'
        $null=Send-Control 'ui.scroll' @{x=($current.grid.x+200);y=($current.grid.y+200);dy=-500;dx=0}
        $scrolled=Send-Control 'ui.inspect'
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet=0}}
        $null=Wait-View {param($v) ($v.windows | Where-Object window -eq $v.window).sheet -eq 0} 'switch worksheet'
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet=1}}
        $null=Wait-View {param($v) $v.scroll.y -eq $scrolled.scroll.y} 'restore worksheet scroll'
        $checks.Add('M4.4 per-worksheet scroll restoration; multi-window shared data regression included')
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M4-alpha.png')}
    }
    if ($M5Task -gt 0) { . (Join-Path $PSScriptRoot 'm5-cases.ps1'); Invoke-M5Workflow }
    if ($M6Task -gt 0) {. (Join-Path $PSScriptRoot 'm5-cases.ps1');. (Join-Path $PSScriptRoot 'm6-cases.ps1');Invoke-M6Workflow}
    if ($M8Task -gt 0) {. (Join-Path $PSScriptRoot 'm8-cases.ps1');Invoke-M8Workflow}
    if ($M7Task -gt 0) {. (Join-Path $PSScriptRoot 'm5-cases.ps1');. (Join-Path $PSScriptRoot 'm7-cases.ps1');Invoke-M7Workflow}
    if ($WindowsTask -gt 0) {. (Join-Path $PSScriptRoot 'm5-cases.ps1');. (Join-Path $PSScriptRoot 'windows-cases.ps1');Invoke-WindowsWorkflow}
    if ($WindowsQualityTask -gt 0) {. (Join-Path $PSScriptRoot 'windows-quality-cases.ps1');Invoke-WindowsQualityWorkflow}
    if ($SaveDialog -or $M4Task -ge 3) {. (Join-Path $PSScriptRoot 'save-dialog-cases.ps1');Invoke-SaveDialogCases}
    Write-Report
} finally {
    if (-not $KeepOpen -and -not $started.HasExited) { Stop-Process -Id $started.Id -ErrorAction SilentlyContinue }
    $env:JOYSHEET_CONFIG_DIR=$priorConfig
}




