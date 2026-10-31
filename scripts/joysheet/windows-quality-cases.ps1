function Invoke-WindowsQualityWorkflow {
    $before=Send-Control 'ui.inspect';$previous=$before.window
    $opened=Send-Control 'app.new'
    $null=Wait-View {param($v) $v.window -eq $opened.window -and $v.grid.height -gt 100} 'quality workbook'
    $null=Send-Control 'ui.ribbon' @{tab='Home'}
    $null=Send-Control 'ui.key' @{key='escape'}
    if($WindowsQualityTask -ge 1){
        foreach($inline in @($true,$false)){
            $cell=if($inline){'D5'}else{'E6'}
            $null=Send-Control 'ui.cell.click' @{cell=$cell}
            $null=Send-Control 'ui.edit.begin' @{inline=$inline}
            $null=Send-Control 'ui.ime.mark' @{text='中文🙂'}
            $null=Send-Control 'ui.ime.mark' @{text='中文🙂组合'}
            $v=Wait-View {param($w) $w.composing -and $w.inputGeometry.markedUtf16.Count -eq 2} 'composition in active text input'
            if($v.inputGeometry.selectionUtf16[1] -ne '中文🙂组合'.Length){throw 'IME caret did not preserve UTF-16 surrogate pairs'}
            if($v.inputGeometry.formula -eq $inline){throw 'Composition reached the wrong editor'}
            foreach($key in @('enter','tab','f6')){
                $null=Send-Control 'ui.key' @{key=$key}
                if(!(Send-Control 'ui.inspect').composing){throw "Composition lost on $key"}
                Assert-Cell $cell $null
            }
            $null=Send-Control 'ui.edit.commit';Assert-Cell $cell $null
            $saveParams=@{path=(Join-Path $artifacts 'IME-should-not-save.xlsx')};$json=$saveParams|ConvertTo-Json -Compress
            $rejected=& $cli send $port app.save $json 2>&1
            if($LASTEXITCODE -eq 0){throw 'Save admitted marked composition'}
            $null=Send-Control 'ui.cell.click' @{cell='G8'}
            $v=Send-Control 'ui.inspect'
            if($v.editor.cell -ne $cell -or $v.editor.text -ne '中文🙂组合'){throw 'Pointer navigation displaced the composition draft'}
            $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts "W5-ime-$inline.png")}
            $v=Send-Control 'ui.inspect';$b=$v.inputGeometry.input;$c=$v.inputGeometry.candidate
            if(!$c -or $c.height -le 0 -or $c.y -lt $b.y-2 -or $c.y -gt $b.y+$b.height+2){throw 'IME candidate anchor is outside its production input layout'}
            $null=Send-Control 'ui.ime.unmark'
            $null=Send-Control 'ui.edit.commit';$null=Send-Control 'ui.awaitIdle'
            Assert-Cell $cell '中文🙂组合'
        }
        $checks.Add('W5 both real input handlers: UTF-16 Chinese/emoji composition, Enter/Tab/F6 and pointer/commit/save guards, candidate layout and exactly-once commit (synthetic IME)')
    }
    if($WindowsQualityTask -ge 2){
        $original=(Send-Control 'ui.inspect').display.scaleFactor
        $null=Send-Control 'engine.execute' @{command='view.freezePanes';params=@{cell='B2'}}
        $null=Send-Control 'engine.execute' @{command='view.zoom';params=@{percent=125}}
        foreach($scale in @(1.0,1.25,1.5,1.75,2.0,3.0)){
            $null=Send-Control 'ui.dpi' @{scale=$scale}
            $null=Send-Control 'ui.cell.click' @{cell='D5'}
            $null=Send-Control 'ui.edit.begin' @{text='缩放🙂候选位置'}
            $null=Send-Control 'ui.ime.mark' @{text='中文'}
            $shot=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts "W6-scale-$scale.png")}
            $v=Send-Control 'ui.inspect';$b=$v.inputGeometry.input;$c=$v.inputGeometry.candidate
            if([Math]::Abs($v.display.scaleFactor-$scale)-gt 0.01 -or !$c -or $c.y -lt $b.y-2 -or $c.y -gt $b.y+$b.height+2){throw 'Scale change broke the candidate layout'}
            if([Math]::Abs($shot.width-$v.display.viewportWidth*$original)-gt 2){throw 'Native GPU readback dimensions changed unexpectedly under a layout-only override'}
            $null=Send-Control 'ui.ime.unmark';$null=Send-Control 'ui.edit.cancel'
            $null=Send-Control 'ui.cell.click' @{cell='F7'}
            if((Send-Control 'ui.inspect').selection -ne 'F7'){throw 'DPI changed logical hit testing'}
        }
        $null=Send-Control 'ui.dpi' @{reset=$true}
        if([Math]::Abs((Send-Control 'ui.inspect').display.scaleFactor-$original)-gt 0.01){throw 'Native scale was not restored'}
        $null=Send-Control 'engine.execute' @{command='view.unfreezePanes';params=@{}}
        $null=Send-Control 'engine.execute' @{command='view.zoom';params=@{percent=100}}
        $checks.Add('W6 production logical layout at synthetic 100/125/150/175/200/300 percent DPI with native-resolution GPU captures, 125 percent worksheet zoom/freeze, IME anchors and hit testing; real OS scale restored')
    }
    if($WindowsQualityTask -ge 3){
        $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='B2';input='辅助功能中文🙂'}}
        $null=Send-Control 'engine.execute' @{command='home.mergeCells';params=@{range='B2:C3'}}
        $null=Send-Control 'ui.cell.click' @{cell='F8'}
        $v=Send-Control 'ui.inspect';$handle=($v.nativeWindows|Where-Object window -eq $v.window).native.handle
        $raw=& pwsh.exe -NoProfile -File (Join-Path $PSScriptRoot 'accessibility-smoke.ps1') -Handle $handle -OwnerProcessId $started.Id -Cell B2 -Focus 2>&1
        if($LASTEXITCODE -ne 0){throw "Native UIA probe failed: $raw"}
        $native=($raw -join "`n")|ConvertFrom-Json
        $native|ConvertTo-Json -Depth 8|Set-Content -LiteralPath (Join-Path $artifacts 'W7-uia.json') -Encoding utf8NoBOM
        $null=Wait-View {param($w) $w.selection -eq 'B2:C3'} 'actual UIA focus action selects merged cell'
        $a=Send-Control 'ui.accessibility';$cell=@($a.cells|Where-Object cell -eq 'B2')
        if(!$a.platformActive -or $cell.Count -ne 1 -or $cell[0].rowSpan -ne 2 -or $cell[0].columnSpan -ne 2 -or $cell[0].value -ne '辅助功能中文🙂'){throw 'Accessible grid lost merged semantics/value'}
        $v=Send-Control 'ui.inspect';$win=($v.nativeWindows|Where-Object window -eq $v.window).native;$scale=$win.dpi/96.0
        if([Math]::Abs($native.bounds.width-$cell[0].bounds.width*$scale)-gt 3 -or [Math]::Abs($native.bounds.x-($win.client.x+($v.grid.x+$cell[0].bounds.x)*$scale))-gt 3){throw 'Actual UIA bounds do not match native DPI/client coordinates'}
        if($native.bounds.width -le 0 -or $native.bounds.height -le 0 -or $a.cells.Count -gt 4096){throw 'Accessible bounds/count invalid'}
        $null=Send-Control 'ui.cell.click' @{cell='F8'}
        $null=Send-Control 'ui.dialog' @{name='home.fontSize'}
        $modalProbe=& pwsh.exe -NoProfile -File (Join-Path $PSScriptRoot 'accessibility-smoke.ps1') -Handle $handle -OwnerProcessId $started.Id -Cell B2 -Focus 2>&1
        if($LASTEXITCODE -ne 0){throw "Modal UIA probe failed: $modalProbe"}
        $v=Send-Control 'ui.inspect'
        if($v.selection -ne 'F8' -or $v.dialog -ne 'commands' -or $v.focus.grid){throw 'Background accessibility action bypassed an active dialog'}
        $null=Send-Control 'ui.dialog.cancel'
        $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='H2';input='=SUM(1,2)'}}
        $null=Send-Control 'engine.execute' @{command='home.formatCells';params=@{range='H2';style=@{protection=@{hidden=$true;locked=$true}}}}
        $null=Send-Control 'engine.execute' @{command='review.protectSheet';params=@{password='quality-test'}}
        $a=Send-Control 'ui.accessibility';$hidden=@($a.cells|Where-Object cell -eq 'H2')[0]
        if(!$hidden.readonly -or $hidden.description.Contains('SUM')){throw 'Accessible metadata disclosed a protected hidden formula'}
        $null=Send-Control 'engine.execute' @{command='review.unprotectSheet';params=@{password='quality-test'}}
        $null=Send-Control 'engine.execute' @{command='view.freezePanes';params=@{cell='B2'}}
        $v=Send-Control 'ui.inspect'
        $null=Send-Control 'ui.scroll' @{x=($v.grid.x+500);y=($v.grid.y+400);dx=-230;dy=-240}
        $a=Send-Control 'ui.accessibility'
        foreach($c in $a.cells){if($c.bounds.x -lt 52 -or $c.bounds.y -lt 28 -or $c.bounds.width -le 0 -or $c.bounds.height -le 0){throw 'Accessible cell escaped frozen clipping'}}
        $a|ConvertTo-Json -Depth 20|Set-Content -LiteralPath (Join-Path $artifacts 'W7-tree.json') -Encoding utf8NoBOM
        $null=Send-Control 'engine.execute' @{command='view.unfreezePanes';params=@{}}
        $null=Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference='A1'}}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W7-accessibility.png')}
        $checks.Add('W7 actual Windows UIA discovers the AccessKit grid and focuses a merged cell through its provider; spans/values, virtual node budget and frozen clipping verified (screen-reader speech remains manual)')
    }
    if($WindowsQualityTask -ge 4){
        $created=Send-Control 'app.new'
        $null=Wait-View {param($w) $w.window -eq $created.window} 'print quality workbook'
        $null=Send-Control 'engine.execute' @{command='home.insertSheet';params=@{name='Other print sheet'}}
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet='Sheet1'}}
        $null=Send-Control 'engine.execute' @{command='range.setValues';params=@{range='A1';values=@(@('打印边界','金额'),@('中文🙂',12),@('公式','=B2*2'))}}
        $null=Send-Control 'engine.execute' @{command='pageLayout.pageSetup';params=@{orientation='landscape';paper='A4';printArea='A1:N80';fitWidth=1;fitHeight=0;gridlines=$true;headings=$true}}
        $null=Send-Control 'ui.command' @{command='file.printPreview';params=@{}}
        $v=Wait-View {param($w) $w.printPreview.pages -ge 2 -and $w.printStatus -eq 'preview ready'} 'quality PDF preview'
        $directory=Split-Path $v.printPreview.pdf
        $xps=Join-Path $artifacts "W8-output-$port.xps"
        $raw=& pwsh.exe -NoProfile -STA -File (Join-Path $PSScriptRoot 'native-print.ps1') -Directory $directory -Validate -Width 1056 -Height 816 -OriginX 40 -OriginY 32 -MarginRight 36 -MarginBottom 24 -FromPage 2 -ToPage 2 -ExportXps $xps 2>&1
        if($LASTEXITCODE -ne 0){throw "WPF pagination/XPS validation failed: $raw"}
        $layout=($raw -join "`n")|ConvertFrom-Json
        if($layout.pages -ne 1 -or $layout.queueSubmitted -or $layout.originX -ne 40 -or $layout.originY -ne 32){throw 'Print range/margin semantics changed'}
        $z=[IO.Compression.ZipFile]::OpenRead($xps)
        try{if(@($z.Entries|Where-Object FullName -like '*.fpage').Count -ne 1){throw 'XPS output did not contain exactly the requested page'}}finally{$z.Dispose()}
        $layout|ConvertTo-Json -Depth 6|Set-Content -LiteralPath (Join-Path $artifacts 'W8-paginator.json') -Encoding utf8NoBOM
        $bad=& pwsh.exe -NoProfile -STA -File (Join-Path $PSScriptRoot 'native-print.ps1') -Directory $directory -Validate -FromPage 3 -ToPage 2 2>&1
        if($LASTEXITCODE -eq 0){throw 'Invalid print range admitted'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'W8-print-preview.png')}
        foreach($closePreview in @($false,$true)){
            $null=Send-Control 'ui.print.dialog'
            $null=Wait-View {param($w) $w.printWork.dialogOpen} 'owned native print dialog'
            if($closePreview){$null=Send-Control 'ui.print.close'}
            $cancel=& pwsh.exe -NoProfile -File (Join-Path $PSScriptRoot 'cancel-owned-print-dialog.ps1') -OwnerProcessId $started.Id -ScriptPath (Join-Path $directory 'print.ps1') 2>&1
            if($LASTEXITCODE -ne 0){throw "Native print cancellation failed: $cancel"}
            $cancel -join "`n"|Set-Content -LiteralPath (Join-Path $artifacts "W8-cancel-$closePreview.json") -Encoding utf8NoBOM
            $null=Wait-View {param($w) !$w.printWork.dialogOpen} 'native print result returned'
            if(!$closePreview -and (Send-Control 'ui.inspect').printStatus -ne 'cancelled'){throw 'Native Cancel result not propagated'}
        }
        if((Send-Control 'ui.inspect').printPreview){throw 'Closed preview was resurrected by native cancellation'}
        $null=Send-Control 'ui.command' @{command='file.printPreview';params=@{}}
        $null=Wait-View {param($w) $w.printStatus -eq 'preview ready'} 'print retry after cancellation'
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet='Other print sheet'}}
        $null=Wait-View {param($w) !$w.printWork.current} 'stale print sheet'
        $errorReply=& $cli send $port ui.print.dialog '{}' 2>&1
        if($LASTEXITCODE -eq 0 -or (Send-Control 'ui.inspect').printWork.dialogOpen){throw 'Stale preview reached system printing'}
        $null=Send-Control 'ui.print.close'
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet='Sheet1'}}
        $null=Send-Control 'ui.command' @{command='file.printPreview';params=@{}}
        $null=Send-Control 'ui.print.close'
        $null=Wait-View {param($w) $w.printWork.exporting -eq 0 -and $w.printWork.rendering -eq 0 -and $null -eq $w.printPreview} 'closed preview stays closed after background rendering'
        $null=Send-Control 'ui.command' @{command='file.printPreview';params=@{}}
        $null=Wait-View {param($w) $w.printStatus -eq 'preview ready'} 'preview retry'
        $null=Send-Control 'ui.key' @{key='tab'}
        $null=Send-Control 'ui.key' @{key='escape'}
        $null=Wait-View {param($w) $null -eq $w.printPreview} 'keyboard closes print preview'
        $null=Send-Control 'app.save' @{path=(Join-Path $artifacts 'Windows-quality.xlsx')}
        $checks.Add('W8 real WPF paginator writes a one-page XPS with selected page/margins and no queue submission; invalid ranges, stale-sheet print rejection, background close race and keyboard preview retry verified')
        $checks.Add('W8 actual native PrintDialog Cancel invoked through its owned-process UIA provider twice: outcome/retry and closing preview while the dialog retains its document; no spool job submitted')
    }
}



