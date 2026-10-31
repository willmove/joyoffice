function Invoke-WindowsWorkflow {
    if($WindowsFrom -le 1){
    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Wait-View {param($v) $v.sheetCount -eq 1 -and $v.worksheet.name -eq 'Sheet1'} 'Windows boundary workbook'
    $null=Send-Control 'ui.ribbon' @{tab='Home'}
    $null=Send-Control 'engine.execute' @{command='home.columnWidth';params=@{cols='A:G';width=150}}
    $null=Send-Control 'engine.execute' @{command='home.rowHeight';params=@{rows='1:8';height=68}}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='A1';input='旋转中文 45°'}}
    $null=Send-Control 'ui.cell.click' @{cell='A1'}
    Test-DataForm 'home.orientation' @{angle=45}
    $style=Send-Control 'cell.get' @{cell='A1'}
    if($style.style.align.rotation -ne 45){throw 'Orientation form did not reach engine'}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='B1';input='反向旋转 −45°'}}
    $null=Send-Control 'engine.execute' @{command='home.orientation';params=@{range='B1';angle=-45}}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='C1';input='竖排中文'}}
    $null=Send-Control 'engine.execute' @{command='home.orientation';params=@{range='C1';angle='vertical'}}
    $styles=@('thin','medium','thick','dashed','dotted','double','dashDot','dashDotDot')
    for($i=0;$i -lt $styles.Count;$i++){
        $cell="D$($i+1)"
        $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell=$cell;input=$styles[$i]}}
        $null=Send-Control 'engine.execute' @{command='home.borders';params=@{range=$cell;preset='all';style=$styles[$i];color='#156CC4'}}
    }
    $patterns=@('Gray50','Gray75','Gray25','Gray125','LightDown','LightUp','LightGrid','LightTrellis')
    for($i=0;$i -lt $patterns.Count;$i++){
        $cell="E$($i+1)"
        $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell=$cell;input=$patterns[$i]}}
        $null=Send-Control 'engine.execute' @{command='home.formatCells';params=@{range=$cell;style=@{fill=@{pattern=$patterns[$i];fg=@{Rgb=3717766};bg=@{Rgb=15793392}}}}}
    }
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='A3';input='双下划线'}}
    $null=Send-Control 'engine.execute' @{command='home.formatCells';params=@{range='A3';style=@{font=@{underline='Double'};align=@{indent=2}}}}
    $null=Send-Control 'engine.execute' @{command='home.borders';params=@{range='B3';preset='diagonalDown';style='dashDot';color='#B42B54'}}
    $null=Send-Control 'engine.execute' @{command='home.borders';params=@{range='B3';preset='diagonalUp';style='double';color='#B42B54'}}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='B5';input='格式表单保留已有属性'}}
    $null=Send-Control 'ui.cell.click' @{cell='B5'}
    Test-DataForm 'home.formatCells' @{angle=30;fillPattern='LightGrid';fillForeground='#9AB0C0';bold=$true;underline='Double';fontName='Microsoft YaHei UI'}
    $styled=Send-Control 'cell.get' @{cell='B5'}
    if($styled.style.align.rotation -ne 30 -or $styled.style.fill.pattern -ne 'LightGrid' -or !$styled.style.font.bold){throw 'Complex format form failed'}
    $null=Wait-View {param($v) $v.outlineText.fontsReady -and $v.outlineText.cachedBlocks -ge 2} 'rotated font outlines painted'
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W1-complex-format.png')}
    $path=Join-Path $artifacts 'Windows-workflow.xlsx'
    $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$path}}
    $null=Send-Control 'engine.execute' @{command='file.open';params=@{path=$path}}
    $null=Send-Control 'ui.awaitIdle'
    $roundtrip=Send-Control 'cell.get' @{cell='B5'}
    if($roundtrip.style.align.rotation -ne 30 -or $roundtrip.style.fill.pattern -ne 'LightGrid'){throw 'Complex formatting XLSX roundtrip failed'}
    $checks.Add('W1 complex-format production forms, angled/stacked Unicode outlines, border patterns/diagonals, hatch fills, double underline and XLSX roundtrip')
    }
    if($WindowsTask -ge 2 -and $WindowsFrom -le 2){
        $v=Send-Control 'ui.inspect';$first=$v.window;$doc=($v.windows|Where-Object window -eq $first).doc;$before=$v.windows.Count
        $null=Send-Control 'ui.command' @{command='view.newWindow';params=@{}}
        $v=Wait-View {param($w) $w.windows.Count -gt $before -and $w.window -ne $first} 'new native view'
        $second=$v.window
        $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
        $v=Wait-View {param($w) ($w.windows|Where-Object window -eq $w.window).doc -ne $doc -and $w.window -ne $first -and $w.window -ne $second} 'third workbook'
        $other=$v.window
        $otherDoc=($v.windows|Where-Object window -eq $other).doc
        foreach($group in @($v.windows|Where-Object {$_.doc -ne $doc -and $_.doc -ne $otherDoc}|Group-Object doc)){
            $null=Send-Control 'engine.execute' @{window=$group.Group[0].window;command='file.close';params=@{force=$true;document=$true}}
        }
        $null=Wait-View {param($w) $w.windows.Count -eq 3} 'three isolated arrangement targets'
        $null=Send-Control 'ui.cell.click' @{window=$first;cell='C2'}
        $null=Send-Control 'ui.edit.text' @{window=$first;text='排列时保留第一草稿'}
        $null=Send-Control 'ui.cell.click' @{window=$second;cell='C3'}
        $null=Send-Control 'ui.edit.text' @{window=$second;text='排列时保留第二草稿'}
        foreach($mode in @('tiled','horizontal','vertical','cascade')){
            $null=Send-Control 'ui.command' @{window=$first;command='view.arrangeAll';params=@{arrange=$mode}}
            $deadline=[DateTime]::UtcNow.AddSeconds(12)
            do {
                $v=Send-Control 'ui.inspect' @{window=$first}
                if($v.arrangement.mode -eq $mode){break}
                if($v.arrangement.error){throw $v.arrangement.error}
                Start-Sleep -Milliseconds 100
            }while([DateTime]::UtcNow -lt $deadline)
            if($v.arrangement.mode -ne $mode){throw "Native arrangement did not complete: $mode"}
            foreach($item in $v.nativeWindows){
                $b=$item.native.bounds;$a=$item.native.work_area
                if($b.x -lt $a.x-2 -or $b.y -lt $a.y-2 -or $b.x+$b.width -gt $a.x+$a.width+2 -or $b.y+$b.height -gt $a.y+$a.height+2){throw "Window left the work area: $mode"}
            }
            if((Send-Control 'ui.inspect' @{window=$first}).editor.text -ne '排列时保留第一草稿' -or (Send-Control 'ui.inspect' @{window=$second}).editor.text -ne '排列时保留第二草稿'){throw 'Window arrangement committed or lost another view draft'}
        }
        $unaffected=@((Send-Control 'ui.inspect' @{window=$other}).nativeWindows|Where-Object window -eq $other)[0].native.bounds|ConvertTo-Json -Compress
        $null=Send-Control 'ui.command' @{window=$first;command='view.arrangeAll';params=@{arrange='vertical';activeWorkbook=$true}}
        $deadline=[DateTime]::UtcNow.AddSeconds(12)
        do {$v=Send-Control 'ui.inspect' @{window=$first};if($v.arrangement.mode -eq 'vertical' -and $v.arrangement.windows.Count -eq 2){break};Start-Sleep -Milliseconds 100}while([DateTime]::UtcNow -lt $deadline)
        if($v.arrangement.windows.Count -ne 2){throw 'Active workbook arrangement included unrelated documents'}
        $after=@($v.nativeWindows|Where-Object window -eq $other)[0].native.bounds|ConvertTo-Json -Compress
        if($unaffected -ne $after){throw 'Active-workbook arrangement moved an unrelated document'}
        $null=Send-Control 'ui.edit.commit' @{window=$first}
        $null=Send-Control 'ui.awaitIdle' @{window=$first}
        if((Send-Control 'cell.get' @{window=$second;cell='C2'}).value -ne '排列时保留第一草稿'){throw 'Shared workbook did not receive committed draft'}
        if((Send-Control 'ui.inspect' @{window=$second}).editor.text -ne '排列时保留第二草稿'){throw 'Other view draft was lost after a shared update'}
        $null=Send-Control 'ui.edit.cancel' @{window=$second}
        $null=Send-Control 'ui.command' @{window=$first;command='view.arrangeAll';params=@{arrange='cascade'}}
        $null=Send-Control 'ui.awaitIdle' @{window=$first}
        $null=Send-Control 'ui.screenshot' @{window=$first;path=(Join-Path $artifacts 'W2-arranged.png')}
        $checks.Add('W2 existing native windows tiled/horizontal/vertical/cascade in monitor work area; active-workbook filtering, independent drafts and shared commit preserved')
    }
    if($WindowsTask -ge 3 -and $WindowsFrom -le 3){
        $before=Send-Control 'ui.inspect';$prior=($before.windows|Where-Object window -eq $before.window).doc
        $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
        $null=Wait-View {param($w) ($w.windows|Where-Object window -eq $w.window).doc -ne $prior} 'object workflow workbook'
        $null=Send-Control 'ui.ribbon' @{tab='Home'}
        $null=Send-Control 'engine.execute' @{command='range.setValues';params=@{range='A1';values=@(@('项目','金额'),@('甲',10),@('乙',30),@('丙',20))}}
        $a=(Send-Control 'engine.execute' @{command='insert.shape';params=@{kind='rectangle';at='B6';width=140;height=80;text='第一形状'}}).shape
        $b=(Send-Control 'engine.execute' @{command='insert.textBox';params=@{at='F12';width=140;height=80;text="中文自动换行测试，多行文本应留在对象范围内。`n第二段 English text wraps inside the box."}}).shape
        $chart=(Send-Control 'engine.execute' @{command='insert.chart';params=@{range='A1:B4';type='column';title='旋转轴标签';at='J6';width=230;height=170}}).chart
        $null=Send-Control 'ui.awaitIdle'
        $v=Send-Control 'ui.inspect';$one=@($v.objectBounds|Where-Object id -eq $a)[0];$two=@($v.objectBounds|Where-Object id -eq $b)[0]
        $null=Send-Control 'ui.click' @{x=($one.x+30);y=($one.y+30)}
        $null=Send-Control 'ui.click' @{x=($two.x+30);y=($two.y+30);cmd=$true}
        if((Send-Control 'ui.inspect').selectedObjects.Count -ne 2){throw 'Ctrl object multi-selection failed'}
        $null=Send-Control 'ui.drag' @{x=($one.x+30);y=($one.y+30);toX=($one.x+75);toY=($one.y+55)}
        $null=Send-Control 'ui.awaitIdle'
        $v=Send-Control 'ui.inspect';$movedA=@($v.objectBounds|Where-Object id -eq $a)[0];$movedB=@($v.objectBounds|Where-Object id -eq $b)[0]
        if([Math]::Abs($movedA.x-$one.x-45)-gt 1 -or [Math]::Abs($movedB.x-$two.x-45)-gt 1){throw 'Multi-object drag did not move every selected member'}
        $null=Send-Control 'ui.key' @{key='z';cmd=$true};$null=Send-Control 'ui.awaitIdle'
        $v=Send-Control 'ui.inspect'
        if([Math]::Abs(@($v.objectBounds|Where-Object id -eq $a)[0].x-$one.x)-gt 1 -or [Math]::Abs(@($v.objectBounds|Where-Object id -eq $b)[0].x-$two.x)-gt 1){throw 'One undo did not restore all dragged objects'}
        $beforeNudge=Send-Control 'ui.inspect'
        $null=Send-Control 'ui.key' @{key='right';shift=$true};$null=Send-Control 'ui.awaitIdle'
        $v=Send-Control 'ui.inspect';if([Math]::Abs(@($v.objectBounds|Where-Object id -eq $a)[0].x-$one.x-10)-gt 1){@{before=$beforeNudge;after=$v;expected=$one.x+10}|ConvertTo-Json -Depth 30|Set-Content (Join-Path $artifacts 'W3-nudge.json');throw "Object keyboard nudge failed: expected $($one.x+10), got $(@($v.objectBounds|Where-Object id -eq $a)[0].x), focus $($v.focus|ConvertTo-Json -Compress), message $($v.message)"}
        $null=Send-Control 'ui.key' @{key='delete'};$null=Send-Control 'ui.awaitIdle'
        if((Send-Control 'ui.inspect').objectBounds.Count -ne 1){throw 'Group delete did not remove selected objects'}
        $null=Send-Control 'ui.key' @{key='z';cmd=$true};$null=Send-Control 'ui.awaitIdle'
        if((Send-Control 'ui.inspect').objectBounds.Count -ne 3){throw 'One undo did not restore group deletion'}
        $v=Send-Control 'ui.inspect';$one=@($v.objectBounds|Where-Object id -eq $a)[0];$last=@($v.objectBounds|Where-Object id -eq $chart)[0]
        $null=Send-Control 'ui.click' @{x=($one.x+30);y=($one.y+30)}
        $null=Send-Control 'ui.click' @{x=($last.x+30);y=($last.y+30);cmd=$true}
        $null=Send-Control 'ui.ribbon' @{tab='Objects'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W3-before-align.png')}
        $v=Send-Control 'ui.inspect';$button=$v.advancedBounds.'objects-left'
        $null=Send-Control 'ui.click' @{x=($button.x+$button.width/2);y=($button.y+$button.height/2)}
        $null=Send-Control 'ui.awaitIdle'
        $v=Send-Control 'ui.inspect';$leftA=@($v.objectBounds|Where-Object id -eq $a)[0].x;$leftB=@($v.objectBounds|Where-Object id -eq $chart)[0].x
        if([Math]::Abs($leftA-$leftB)-gt 1){throw 'Mixed chart/shape alignment failed'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W3-objects.png')}
        $null=Send-Control 'ui.key' @{key='escape'}
        $checks.Add('W3 Ctrl mixed-object selection, group pointer drag, one-step undo, keyboard nudge, group delete/undo and actual align button')
        $null=Send-Control 'ui.cell.click' @{cell='A8'}
        $null=Send-Control 'ui.edit.text' @{text='=SUM(A2:B4,IF(B2>0,B3,B4))'}
        $v=Send-Control 'ui.inspect'
        if($v.formulaReferences.Count -lt 4){throw 'Formula reference legend did not parse real cell references'}
        $null=Send-Control 'ui.key' @{key='left'}
        $v=Send-Control 'ui.inspect';if($v.formulaHint.signature -notlike 'SUM*'){throw 'Formula hint did not follow caret into the outer call'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W3-formula.png')}
        $null=Send-Control 'ui.key' @{key='f6'}
        $v=Send-Control 'ui.inspect';if(!$v.focus.formula -or $v.editor.text -ne '=SUM(A2:B4,IF(B2>0,B3,B4))'){throw 'F6 lost shared formula draft'}
        $null=Send-Control 'ui.key' @{key='f6'};if(!(Send-Control 'ui.inspect').focus.name){throw 'F6 did not reach name box'}
        $null=Send-Control 'ui.key' @{key='f6'};$null=Send-Control 'ui.key' @{key='f6'}
        if(!(Send-Control 'ui.inspect').focus.sheets){throw 'F6 did not reach worksheet tabs'}
        $null=Send-Control 'ui.key' @{key='f6'}
        if(!(Send-Control 'ui.inspect').focus.grid){throw 'F6 did not return to the grid'}
        $null=Send-Control 'ui.edit.cancel'
        $null=Send-Control 'ui.cell.click' @{cell='A8'}
        $null=Send-Control 'ui.key' @{key='1';cmd=$true}
        $v=Wait-View {param($w) $w.dialog -eq 'commands'} 'Ctrl+1 format form'
        $null=Send-Control 'ui.key' @{key='escape'}
        try {$null=Wait-View {param($w) $w.dialog -eq ''} 'Escape closes actual format input panel'} catch {Send-Control 'ui.inspect'|ConvertTo-Json -Depth 30|Set-Content (Join-Path $artifacts 'W3-escape.json');throw}
        $null=Send-Control 'engine.execute' @{command='home.mergeCells';params=@{range='A9:C10'}}
        $null=Send-Control 'engine.execute' @{command='view.freezePanes';params=@{cell='B2'}}
        $null=Send-Control 'engine.execute' @{command='view.zoom';params=@{percent=125}}
        $null=Send-Control 'ui.cell.click' @{cell='A9'}
        $null=Send-Control 'ui.edit.text' @{text='跨冻结边界的合并编辑'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W3-merged-editor.png')}
        $null=Send-Control 'ui.edit.commit';$null=Send-Control 'ui.awaitIdle';Assert-Cell 'A9' '跨冻结边界的合并编辑'
        $null=Send-Control 'engine.execute' @{command='view.unfreezePanes';params=@{}}
        $null=Send-Control 'engine.execute' @{command='view.zoom';params=@{percent=100}}
        $checks.Add('W3 caret-aware function parameter highlight/reference outlines, F6 shared-draft focus, Ctrl+1 and merged editing across freeze at 125% zoom')
        foreach($set in @('3Arrows','3Flags','3TrafficLights1','3Signs','3Symbols2','4Rating','5Quarters')){
            $null=Send-Control 'engine.execute' @{command='home.conditionalFormat';params=@{range='B2:B4';rule=@{type='iconSet';set=$set}}}
            $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts "W3-icons-$set.png")}
            $null=Send-Control 'engine.execute' @{command='home.clearRules';params=@{range='B2:B4'}}
        }
        $checks.Add('W3 native vector icon-set screenshots for arrows, flags, lights, signs, symbols, ratings and quarters')
        $path=Join-Path $artifacts 'Windows-objects.xlsx'
        $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$path}}
    }
    if($WindowsTask -ge 4){
        $before=Send-Control 'ui.inspect';$prior=($before.windows|Where-Object window -eq $before.window).doc
        $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
        $null=Wait-View {param($w) ($w.windows|Where-Object window -eq $w.window).doc -ne $prior} 'final boundary workbook'
        $null=Send-Control 'ui.ribbon' @{tab='Home'}
        $null=Send-Control 'engine.execute' @{command='range.setValues';params=@{range='A1';values=@(@('组','金额','序号'),@('B',1,1),@('A',2,1),@('A',5,2),@('A',5,1))}}
        Test-DataForm 'data.sort' @{range='A1:C5';column='A';order='asc';keysText='B:desc,C:asc';header=$true}
        Assert-Cell 'B2' 5;Assert-Cell 'C2' 1;Assert-Cell 'C3' 2;Assert-Cell 'A5' 'B'
        $null=Send-Control 'ui.command' @{command='insert.checkbox';params=@{range='B7'}}
        $null=Send-Control 'ui.awaitIdle';Assert-Cell 'B7' $false
        $null=Send-Control 'ui.cell.click' @{cell='B7'};$null=Send-Control 'ui.awaitIdle';Assert-Cell 'B7' $true
        $null=Send-Control 'ui.key' @{key='space'};$null=Send-Control 'ui.awaitIdle';Assert-Cell 'B7' $false
        $null=Send-Control 'ui.key' @{key='space'};$null=Send-Control 'ui.awaitIdle';Assert-Cell 'B7' $true
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W4-checkbox.png')}
        $null=Send-Control 'ui.key' @{key='delete'};$null=Send-Control 'ui.awaitIdle';Assert-Cell 'B7' $false
        $null=Send-Control 'ui.key' @{key='delete'};$null=Send-Control 'ui.awaitIdle'
        $cell=Send-Control 'cell.get' @{cell='B7'};if($cell.style.num_fmt -eq 'checkbox' -or $null -ne $cell.value){throw 'Checkbox delete did not follow two-pass engine semantics'}
        $null=Send-Control 'ui.command' @{command='insert.checkbox';params=@{range='B7'}};$null=Send-Control 'ui.awaitIdle'
        $null=Send-Control 'ui.cell.click' @{cell='E7'}
        $null=Send-Control 'ui.key' @{key='right'}
        $v=Wait-View {param($w) $w.selection -eq 'F7'} 'worksheet arrow through nested input'
        $null=Send-Control 'ui.key' @{key='space';cmd=$true}
        $null=Wait-View {param($w) $w.selection -eq 'F:F'} 'Ctrl+Space column selection'
        $null=Send-Control 'ui.cell.click' @{cell='A8'}
        $null=Send-Control 'ui.key' @{key='space';shift=$true}
        $null=Wait-View {param($w) $w.selection -eq '8:8'} 'Shift+Space row selection'
        $null=Send-Control 'ui.cell.click' @{cell='A8'}
        $null=Send-Control 'ui.key' @{key='home';cmd=$true}
        $null=Wait-View {param($w) $w.selection -eq 'A1'} 'Ctrl+Home navigation'
        $null=Send-Control 'engine.execute' @{command='home.insertSheet';params=@{name='键盘切表'}}
        $null=Send-Control 'ui.key' @{key='f6';shift=$true}
        if(!(Send-Control 'ui.inspect').focus.sheets){throw 'Shift+F6 did not focus sheet tabs'}
        $null=Send-Control 'ui.key' @{key='left'}
        $null=Wait-View {param($w) $w.worksheet.name -eq 'Sheet1'} 'keyboard worksheet navigation'
        $null=Send-Control 'ui.key' @{key='enter'}
        if(!(Send-Control 'ui.inspect').focus.grid){throw 'Worksheet tab activation did not return to grid'}
        $null=Send-Control 'engine.execute' @{command='insert.textBox';params=@{at='E12';width=150;height=90;text='中文字体与斜体轮廓文字，自动换行并裁剪在文本框内。'}}
        $chart=(Send-Control 'engine.execute' @{command='insert.chart';params=@{range='A1:B5';type='column';at='H12';width=260;height=190;title='旋转轴标题'}}).chart
        $null=Send-Control 'engine.execute' @{command='chart.addElement';params=@{chart=$chart;element='axisTitles';on=$true}}
        $jsonPath=Join-Path $artifacts 'W4-source.json'
        $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$jsonPath}}
        $fixture=Get-Content -LiteralPath $jsonPath -Raw|ConvertFrom-Json -Depth 100
        $fixture.sheets[0].shapes[0].format.font='Microsoft YaHei UI'
        $fixture.sheets[0].shapes[0].format.italic=$true
        $fixture.sheets[0].shapes[0].format.font_size=16
        $fixture.sheets[0].shapes[0].format.text_color=@{Rgb=2361807}
        $fixture.sheets[0].shapes[0].format.text_outline=@(@{Rgb=16711680},0.6)
        $imported=Join-Path $artifacts 'W4-import.json'
        $fixture|ConvertTo-Json -Depth 100|Set-Content -LiteralPath $imported -Encoding utf8NoBOM
        $null=Send-Control 'engine.execute' @{command='file.open';params=@{path=$imported}}
        $null=Wait-View {param($w) ($w.windows|Where-Object window -eq $w.window).path -eq $imported} 'import existing shape text style fixture'
        if((Send-Control 'ui.inspect').panel -eq 21){throw 'New document unnecessarily reopened the startup recovery sidebar'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W4-shape-font.png')}
        $null=Send-Control 'engine.execute' @{command='view.freezePanes';params=@{cell='B2'}}
        $v=Send-Control 'ui.inspect'
        $null=Send-Control 'ui.scroll' @{x=($v.grid.x+500);y=($v.grid.y+400);dx=-220;dy=-240}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W4-frozen-objects.png')}
        $v=Send-Control 'ui.inspect'
        $null=Send-Control 'ui.cell.click' @{cell='A1'}
        if((Send-Control 'ui.inspect').selectedObjects.Count -ne 0){throw 'Scrolled object intercepted frozen cell input'}
        $null=Send-Control 'engine.execute' @{command='view.unfreezePanes';params=@{}}
        $null=Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='A1'}}
        $null=Send-Control 'engine.execute' @{command='home.insertSheet';params=@{name='图标兼容'}}
        $null=Send-Control 'engine.execute' @{command='home.columnWidth';params=@{cols='A:J';width=100}}
        $null=Send-Control 'engine.execute' @{command='home.rowHeight';params=@{rows='1:9';height=36}}
        $icons=@('check','close','plus','minus','chart','line chart','pie chart','scatter','table','sum','filter','lock','comment','note','picture','book','folder','search','calculator','link','pen','share','settings','home','save','theme','shapes','text','trace','validation')
        for($i=0;$i -lt $icons.Count;$i++){
            $column=[char](65+($i%10));$row=1+[int][math]::Floor($i/10)*3
            $null=Send-Control 'engine.execute' @{command='insert.icons';params=@{name=$icons[$i];at="$column$row";size=26;color='#007A5A'}}
            $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell="$column$($row+1)";input=$icons[$i]}}
        }
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W4-icons.png')}
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet='Sheet1'}}
        $saved=Join-Path $artifacts 'Windows-boundaries.xlsx'
        $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$saved}}
        for($cycle=0;$cycle -lt 6;$cycle++){
            $null=Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}
            $reopened=Send-Control 'app.open' @{path=$saved}
            $null=Wait-View {param($w) $w.window -eq $reopened.window -and ($w.windows|Where-Object window -eq $w.window).path -eq $saved} 'close/open native activation race'
            Assert-Cell 'B2' 5
            if((Send-Control 'ui.inspect').window -ne $reopened.window){throw 'A stale native activation displaced the newly opened document'}
        }
        $checks.Add('W4 multi-key sort form, checkbox pointer/Space/two-pass Delete, nested-input navigation, F6 sheet tabs, styled shape/rotated chart axis, frozen object clipping and all 30 upstream icon names')
        $checks.Add('W4 six close/open cycles preserve the intended document despite native close activation events')
    }
}
