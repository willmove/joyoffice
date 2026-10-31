function Click-M7Element([string]$Name){
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-current.png');allowBusy=$Name.StartsWith('cancel-task:')}
    $v=Send-Control 'ui.inspect';$b=$v.advancedBounds.$Name
    if(!$b -or $b.width -le 0){throw "Missing M7 element: $Name"}
    $null=Send-Control 'ui.click' @{x=($b.x+$b.width/2);y=($b.y+$b.height/2)}
}
function Open-M7Run([string]$Name){
    $null=Send-Control 'ui.dialog' @{name='automate.runScript'}
    $null=Send-Control 'ui.dialog.set' @{field='name';value=$Name}
    $null=Send-Control 'ui.dialog.confirm'
}
function Invoke-M7Workflow {
    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Wait-View {param($v) ($v.windows|Where-Object window -eq $v.window).path -eq $null} 'M7 workbook'
    $null=Send-Control 'engine.execute' @{command='range.setValues';params=@{range='A1';values=@(@('地区','金额','数量','利润'),@('东区',10,2,'=B2*C2'),@('西区',20,3,'=B3*C3'),@('东区',30,4,'=B4*C4'))}}
    Test-DataForm 'insert.pivotTable' @{source='Sheet1!A1:C4';destination='Sheet1!F3';name='M7透视';rowFields='地区';valueFields='金额'}
    Test-DataForm 'pivot.fields'
    $v=Wait-View {param($v) $v.dataRaw.fields.Count -eq 3 -and $v.advancedBounds.'field:数量'.width -gt 0} 'pivot field pane'
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-pivot-fields.png')}
    $a=$v.advancedBounds.'field:数量';$b=$v.advancedBounds.'area:values'
    $null=Send-Control 'ui.drag' @{x=($a.x+12);y=($a.y+$a.height/2);toX=($b.x+50);toY=($b.y+$b.height/2)}
    $null=Wait-View {param($v) $v.dataRaw.values.Count -eq 2} 'pivot field drag adds quantity'
    Test-DataForm 'data.analysisToolPak' @{tool='descriptive';range='B2:B4';outputTo='range';output='J2'}
    $null=Wait-View {param($v) $v.dataResultKind -eq 'data.analysisToolPak'} 'analysis result'
    Assert-Cell 'K4' 20
    Test-DataForm 'data.goalSeek' @{set='D2';to=50;changing='B2'}
    $v=Send-Control 'cell.get' @{cell='D2'};if([Math]::Abs($v.value-50)-gt .001){throw 'Goal seek failed'}
    Test-DataForm 'data.solver' @{objective='D2';variables='B2';constraintLines='B2 <= 10';method='simplex';goal='max'}
    $v=Send-Control 'cell.get' @{cell='B2'};if([Math]::Abs($v.value-10)-gt .001){throw 'Solver failed'}
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-analysis.png')}
    $checks.Add('M7.1 production pivot builder and field drag, descriptive report, goal seek and simplex solver')
    if($M7Task -ge 2) {
        Test-DataForm 'review.newComment' @{cell='A2';text='M7 审阅批注';author='Joysheet'}
        Test-DataForm 'review.replyComment' @{cell='A2';text='已核对数据'}
        Test-DataForm 'review.resolveComment' @{cell='A2';resolved=$true}
        Test-DataForm 'review.newNote' @{cell='C2';text='数量单位：件'}
        Test-DataForm 'review.showAllNotes'
        $v=Send-Control 'ui.inspect'
        if(@($v.review.comments).Count -ne 2 -or @($v.review.comments|Where-Object cell -eq 'A2')[0].replies -ne 1){throw 'Comment thread was not updated'}
        Test-DataForm 'formulas.defineName' @{name='Sales';refersTo='=Sheet1!$B$2';scope='Workbook'}
        $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='D6';input='=Sales*2'}}
        Test-DataForm 'formulas.defineName' @{name='Revenue';oldName='Sales';oldScope='Workbook';refersTo='=Sheet1!$B$2';scope='Workbook'}
        Assert-Cell 'D6' 20
        if((Send-Control 'cell.get' @{cell='D6'}).formula -notmatch 'Revenue'){throw 'Name rename did not update formulas'}
        Test-DataForm 'formulas.nameManager'
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-names.png')}
        Test-DataForm 'review.protectSheet' @{password='m7-sheet-test'}
        $null=Send-Control 'ui.cell.click' @{cell='A2'}
        $null=Send-Control 'ui.edit.text' @{text='不应写入保护单元格'}
        $null=Send-Control 'ui.edit.commit'
        $null=Wait-View {param($v) $v.error -and $null -ne $v.editor -and $v.review.sheetProtected} 'sheet protection preserves rejected draft'
        Assert-Cell 'A2' '东区'
        $null=Send-Control 'ui.edit.cancel'
        $null=Send-Control 'ui.dialog' @{name='review.unprotectSheet'}
        $null=Send-Control 'ui.dialog.set' @{field='password';value='wrong-password'}
        $null=Send-Control 'ui.dialog.confirm'
        $null=Wait-View {param($v) $v.error -and $v.review.sheetProtected} 'wrong unprotect password'
        Test-DataForm 'review.unprotectSheet' @{password='m7-sheet-test'}
        Test-DataForm 'review.protectWorkbook' @{on=$true;password='m7-book-test'}
        $saved=Join-Path $artifacts 'M7-workflow.xlsx'
        $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$saved}}
        $null=Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}
        $null=Send-Control 'engine.execute' @{command='file.open';params=@{path=$saved}}
        $null=Wait-View {param($v) $v.review.structureProtected -and @($v.review.comments).Count -eq 2 -and $v.review.names -contains 'Revenue'} 'review and protection XLSX roundtrip'
        Test-DataForm 'review.protectWorkbook' @{on=$false;password='m7-book-test'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-review.png')}
        $checks.Add('M7.2 comments/replies/resolve, notes, name rename, password protection/rejection and XLSX roundtrip')
    }
    if($M7Task -ge 3){
        $body=@(@{command='cell.set';params=@{cell='G2';input='审核后运行的脚本'}})|ConvertTo-Json -Depth 10 -Compress -AsArray
        Test-DataForm 'automate.newScript' @{name='审核脚本';commandsText=$body}
        Test-DataForm 'automate.recordActions' @{stop=$false;name='录制操作'}
        Type-Cell 'B20' '录制内容'
        Test-DataForm 'automate.recordActions' @{stop=$true;name='录制操作'}
        $saved=Join-Path $artifacts 'M7-workflow.xlsx'
        $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$saved}}
        $null=Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}
        $null=Send-Control 'engine.execute' @{command='file.open';params=@{path=$saved}}
        $null=Wait-View {param($v) -not $v.review.scriptsTrusted} 'file scripts start untrusted'
        Open-M7Run '审核脚本'
        $null=Wait-View {param($v) $v.dialog -eq 'scriptTrust'} 'script trust review'
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-trust.png')}
        Click-M7Element 'trust-decline'
        $null=Wait-View {param($v) $v.dialog -eq '' -and -not $v.review.scriptsTrusted} 'declining trust'
        Assert-Cell 'G2' $null
        Open-M7Run '审核脚本'
        $null=Wait-View {param($v) $v.dialog -eq 'scriptTrust'} 'second trust review'
        Click-M7Element 'trust-accept'
        $null=Wait-View {param($v) $v.review.scriptsTrusted -and @($v.tasks|Where-Object state -eq 'completed').Count -gt 0} 'trusted script task completion'
        Assert-Cell 'G2' '审核后运行的脚本'
        $badPath=Join-Path $artifacts 'script-must-not-write.xlsx'
        $bad=@(@{command='cell.set';params=@{cell='G9';input='不应执行'}},@{command='file.save';params=@{path=$badPath}})|ConvertTo-Json -Depth 10 -Compress -AsArray
        Test-DataForm 'automate.newScript' @{name='沙箱测试';commandsText=$bad}
        Open-M7Run '沙箱测试'
        $null=Wait-View {param($v) $v.error -and @($v.tasks|Where-Object state -eq 'failed').Count -gt 0} 'sandbox rejection before any step'
        Assert-Cell 'G9' $null
        if(Test-Path -LiteralPath $badPath){throw 'Script escaped file sandbox'}
        $long=for($i=0;$i -lt 15000;$i++){@{command='formulas.evaluate';params=@{cell='A1';formula='=SUM(SEQUENCE(100000))'}}}
        $null=Send-Control 'engine.execute' @{command='automate.newScript';params=@{name='取消测试';commands=$long}}
        $null=Send-Control 'ui.command' @{command='automate.runScript';params=@{name='取消测试'}}
        $v=Wait-View {param($v) @($v.tasks|Where-Object {$_.state -in @('queued','running')}).Count -gt 0} 'running cancellable script'
        $id=@($v.tasks|Where-Object {$_.state -in @('queued','running')})[0].id
        $null=Wait-View {param($v) $v.advancedBounds."cancel-task:$id".width -gt 0} 'cancel button painted'
        Click-M7Element "cancel-task:$id"
        $null=Wait-View {param($v) @($v.tasks|Where-Object {$_.id -eq $id -and $_.state -eq 'cancelled'}).Count -eq 1} 'cooperative script cancellation'
        $null=Send-Control 'engine.execute' @{command='automate.deleteScript';params=@{name='取消测试'}}
        Test-DataForm 'automate.deleteScript' @{name='沙箱测试'}
        Test-DataForm 'automate.allScripts'
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-scripts.png')}
        $null=Send-Control 'engine.execute' @{command='file.save';params=@{}}
        $checks.Add('M7.3 recording/editor, untrusted XLSX review/decline/accept, sandbox preflight and actual task cancel button')
    }
    if($M7Task -ge 4){
        $origin=Send-Control 'ui.inspect';$originId=$origin.window
        $path=@($origin.windows|Where-Object window -eq $originId)[0].path
        $second=Send-Control 'engine.execute' @{command='file.new';params=@{}}
        $secondId=$second.window
        $null=Wait-View {param($v) $v.window -eq $secondId} 'M7 second document'
        Type-Cell 'A1' '第二工作簿不能被覆盖'
        $null=Send-Control 'engine.execute' @{command='window.activate';params=@{window=$originId}}
        $null=Wait-View {param($v) $v.window -eq $originId} 'M7 original document'
        $null=Send-Control 'ui.cell.click' @{cell='G8'}
        $null=Send-Control 'ui.edit.text' @{text='由 MCP 保存的后台窗口草稿'}
        $null=Send-Control 'engine.execute' @{command='window.activate';params=@{window=$secondId}}
        $null=Wait-View {param($v) $v.window -eq $secondId} 'M7 background draft'
        $before=Send-Control 'cell.get' @{cell='G8';window=$originId}
        if($before.value){throw 'Background draft committed before save'}
        $null=Send-Control 'ui.resize' @{window=$originId;width=800;height=600}
        $shot=Join-Path $artifacts 'M7-mcp-targeted.png'
        $requests=@(
            @{jsonrpc='2.0';id=1;method='initialize';params=@{protocolVersion='2025-03-26';capabilities=@{};clientInfo=@{name='Joysheet M7';version='0.1.0'}}},
            @{jsonrpc='2.0';id=2;method='tools/call';params=@{name='save_workbook';arguments=@{path=$path;window=$originId}}},
            @{jsonrpc='2.0';id=3;method='tools/call';params=@{name='get_cell';arguments=@{cell='G8';window=$originId}}},
            @{jsonrpc='2.0';id=4;method='tools/call';params=@{name='get_cell';arguments=@{cell='A1';window=$secondId}}},
            @{jsonrpc='2.0';id=5;method='tools/call';params=@{name='screenshot';arguments=@{path=$shot;window=$originId}}}
        )
        $lines=@($requests|ForEach-Object{$_|ConvertTo-Json -Depth 20 -Compress})
        $replies=@($lines|& $cli mcp --connect "$port")
        if($LASTEXITCODE -ne 0){throw 'M7 MCP bridge exited'}
        $parsed=@($replies|ForEach-Object{$_|ConvertFrom-Json})
        if($parsed.Count -ne 5 -or @($parsed|Where-Object{$_.error -or $_.result.isError}).Count -gt 0){throw "M7 MCP errors: $replies"}
        $cell=($parsed[2].result.content[0].text|ConvertFrom-Json)
        $other=($parsed[3].result.content[0].text|ConvertFrom-Json)
        $image=($parsed[4].result.content[0].text|ConvertFrom-Json)
        if($cell.value -ne '由 MCP 保存的后台窗口草稿' -or $other.value -ne '第二工作簿不能被覆盖' -or $image.window -ne $originId){throw 'MCP target identity was lost'}
        $null=Send-Control 'engine.execute' @{command='window.activate';params=@{window=$secondId}}
        $already=Send-Control 'app.open' @{window=$originId;path=$path}
        if(!$already.alreadyOpen -or $already.window -ne $originId -or $already.doc -ne $image.doc){throw 'Opening an existing background document returned another window'}
        $idle=Send-Control 'ui.awaitIdle' @{window=$originId;minRevision=$image.revision;minSequence=$image.sequence}
        if($idle.doc -ne $image.doc){throw 'Revision fence returned wrong document'}
        $bad=Join-Path $artifacts "M7-timeout-$port.png";$rejected=$false
        try{$null=Send-Control 'ui.screenshot' @{window=$originId;path=$bad;minRevision=9999999}}catch{$rejected=$true}
        if(!$rejected -or (Test-Path -LiteralPath $bad)){throw 'Screenshot revision timeout was ignored'}
        $rejected=$false;try{$null=Send-Control 'ui.inspect' @{window=999999}}catch{$rejected=$true};if(!$rejected){throw 'Unknown window accepted'}
        $null=Send-Control 'ui.resize' @{window=$originId;width=1440;height=900}
        $null=Send-Control 'engine.execute' @{command='window.close';params=@{window=$secondId;force=$true}}
        $null=Send-Control 'engine.execute' @{command='window.activate';params=@{window=$originId}}
        $null=Send-Control 'ui.awaitIdle' @{window=$originId}
        $null=Send-Control 'ui.ribbon' @{tab='Review'}
        $null=Send-Control 'ui.menu.invoke' @{command='review.workbookStatistics'}
        $null=Send-Control 'ui.awaitIdle'
        # Fixed 1440x900 Chinese shell: exercise the actual desktop-control toolbar entry.
        $null=Send-Control 'ui.click' @{x=524;y=70}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M7-control.png')}
        $null=Send-Control 'engine.execute' @{command='file.save';params=@{}}
        $checks.Add('M7.4 live MCP targeted background-draft save/read/screenshot, window control, revision fences, timeout and unknown-window rejection')
    }
}
