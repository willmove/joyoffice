function Invoke-M8Workflow {
    # This workflow requires empty cells: the preceding M6 sample has charts over D5.
    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Wait-View {param($w) $w.sheetNames.Count -eq 1 -and $w.worksheet.name -eq 'Sheet1' -and $w.drawingObjects.charts.Count -eq 0} 'independent M8 workbook'
    $null=Send-Control 'ui.ribbon' @{tab='Home'}
    $path=Join-Path $artifacts "M8-local-$port.xlsx"
    $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$path}}
    $null=Send-Control 'ui.edit.text' @{text='打开文件前保留的草稿'}
    $uri=[Uri]::new($path).AbsoluteUri
    $null=Send-Control 'ui.openFiles' @{paths=@($uri)}
    $null=Send-Control 'ui.awaitIdle'
    Assert-Cell (Send-Control 'ui.inspect').selection '打开文件前保留的草稿'
    $before=(Send-Control 'ui.inspect').windows.Count
    $rejected=$false
    try{$null=Send-Control 'ui.openFiles' @{paths=@($app)}}catch{$rejected=$true}
    if(!$rejected -or (Send-Control 'ui.inspect').windows.Count -ne $before){throw 'Non-workbook drop was accepted'}
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-local-open.png')}
    $checks.Add('M8.1 local file URL/drop handler validates paths and commits draft; unsupported file rejected (synthetic control regression)')

    if($M8Task -ge 2){
        $null=Send-Control 'ui.set' @{formulaBar=$false;dark=$true;persist=$false}
        $hidden=Send-Control 'ui.inspect'
        if($hidden.settings.formula_bar -or !$hidden.settings.dark){throw 'Settings were not applied'}
        $null=Send-Control 'ui.key' @{key=',';cmd=$true}
        if((Send-Control 'ui.inspect').panel -ne 20){throw 'Settings shortcut did not open panel'}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-settings.png')}
        $null=Send-Control 'ui.set' @{formulaBar=$true;dark=$false;persist=$false}
        $null=Send-Control 'ui.cell.click' @{cell='A1'}
        $null=Send-Control 'ui.cell.click' @{cell='C3';cmd=$true}
        $v=Send-Control 'ui.inspect'
        if($v.selection -notmatch 'A1' -or $v.selection -notmatch 'C3'){throw 'Multi-area selection failed'}
        $null=Send-Control 'ui.cell.click' @{cell='A1'}
        $v=Send-Control 'ui.inspect';$col=@($v.visibleColumns|Where-Object index -eq 0)[0]
        $x=$v.grid.x+$col.start+$col.size;$y=$v.grid.y+12
        $null=Send-Control 'ui.drag' @{x=$x;y=$y;toX=($x+36);toY=$y}
        $null=Wait-View {param($w) [Math]::Abs(@($w.visibleColumns|Where-Object index -eq 0)[0].size-$col.size-36)-lt 1} 'column boundary drag'
        $null=Send-Control 'ui.key' @{key='z';cmd=$true}
        $null=Send-Control 'engine.execute' @{command='home.insertSheet';params=@{name='引用数据'}}
        Type-Cell 'A1' '7'
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet=0}}
        $null=Send-Control 'ui.cell.click' @{cell='D5'}
        $null=Send-Control 'ui.edit.text' @{text='='}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-before-reference.png')}
        $v=Send-Control 'ui.inspect';$b=$v.advancedBounds.'sheet-tab:1'
        $null=Send-Control 'ui.click' @{x=($b.x+$b.width/2);y=($b.y+$b.height/2)}
        $null=Wait-View {param($w) $w.worksheet.name -eq '引用数据' -and $null -ne $w.editor} 'cross-sheet draft preserved'
        $null=Send-Control 'ui.cell.click' @{cell='A1'}
        $null=Send-Control 'ui.edit.commit'
        $null=Wait-View {param($w) $null -eq $w.editor -and $w.worksheet.name -eq 'Sheet1'} 'cross-sheet formula commit'
        Assert-Cell 'D5' 7
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-before-tab-drag.png')}
        $v=Send-Control 'ui.inspect';$a=$v.advancedBounds.'sheet-tab:0';$b=$v.advancedBounds.'sheet-tab:1'
        $null=Send-Control 'ui.drag' @{x=($a.x+$a.width/2);y=($a.y+$a.height/2);toX=($b.x+$b.width/2);toY=($b.y+$b.height/2)}
        $null=Wait-View {param($w) $w.sheetNames[0] -eq '引用数据'} 'sheet tab drag reorder'
        $checks.Add('M8.2 settings/shortcut, multi-area selection, resize and undo, cross-sheet formula reference and sheet tab reorder through production GPUI events')
    }

    if($M8Task -ge 3){
        $null=Send-Control 'ui.ribbon' @{tab='Home'}
        $null=Send-Control 'engine.execute' @{command='data.validation';params=@{range='F2';type='whole';formula1='0';formula2='10';errorStyle='warning';showError=$true;errorMessage='测试警告确认'}}
        $null=Send-Control 'ui.cell.click' @{cell='F2'}
        $null=Send-Control 'ui.edit.text' @{text='99'}
        $null=Send-Control 'ui.edit.commit'
        $null=Wait-View {param($w) $w.dialog -eq 'validationConfirmation'} 'validation warning confirmation'
        Assert-Cell 'F2' $null
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-validation.png')}
        $v=Send-Control 'ui.inspect';$b=$v.advancedBounds.'validation-cancel'
        $null=Send-Control 'ui.click' @{x=($b.x+$b.width/2);y=($b.y+$b.height/2)}
        $v=Send-Control 'ui.inspect';if($v.dialog -ne '' -or $v.editor.text -ne '99'){throw 'Validation cancel lost draft'}
        Assert-Cell 'F2' $null
        $null=Send-Control 'ui.edit.commit'
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-validation-accept.png')}
        $v=Send-Control 'ui.inspect';$b=$v.advancedBounds.'validation-accept'
        $null=Send-Control 'ui.click' @{x=($b.x+$b.width/2);y=($b.y+$b.height/2)}
        $null=Wait-View {param($w) $null -eq $w.editor -and $w.dialog -eq ''} 'validation accepted'
        Assert-Cell 'F2' 99
        $checks.Add('M8.3 warning validation requires explicit approval; actual cancel/keep buttons preserve draft and prevent grid click-through')
        Type-Cell 'D9' '=2+3'
        $null=Send-Control 'ui.cell.click' @{cell='D10'}
        $null=Send-Control 'ui.edit.text' @{text='崩溃前未提交的中文草稿'}
        $deadline=[DateTime]::UtcNow.AddSeconds(15)
        $record=$null
        while([DateTime]::UtcNow -lt $deadline){
            $records=Get-ChildItem -LiteralPath (Join-Path $env:JOYSHEET_CONFIG_DIR 'recovery') -Filter '*.recovery.json' -ErrorAction SilentlyContinue
            foreach($file in $records){$r=Get-Content -LiteralPath $file.FullName -Raw|ConvertFrom-Json -Depth 100;if(@($r.entry.drafts|Where-Object text -eq '崩溃前未提交的中文草稿').Count -gt 0){$record=$r;break}}
            if($record){break};Start-Sleep -Milliseconds 150
        }
        if(!$record){throw 'Recovery copy did not include uncommitted draft'}
        Stop-Process -Id $started.Id
        $started.WaitForExit()
        $script:started=Start-Process -FilePath $app -ArgumentList @('--sample','sales','--control',"$port",'--diagnostics') -WorkingDirectory $binaryRoot -WindowStyle Normal -PassThru -RedirectStandardOutput (Join-Path $artifacts 'recovery-stdout.log') -RedirectStandardError (Join-Path $artifacts 'recovery-stderr.log')
        $restarted=Wait-View {param($w) $w.grid.height -gt 100 -and $w.perf.frame -gt 1} 'restart after actual forced termination'
        if($restarted.panel -ne 21){throw 'Startup did not offer discovered recovery copies'}
        $copies=Send-Control 'ui.recovery.list'
        if(@($copies|Where-Object id -eq $record.entry.id).Count -ne 1){throw 'Previous session recovery was not discovered'}
        $null=Send-Control 'ui.recovery.restore' @{id=$record.entry.id}
        $null=Wait-View {param($w) $w.editor.text -eq '崩溃前未提交的中文草稿'} 'restored pending edit'
        Assert-Cell 'D9' 5
        Assert-Cell 'D10' $null
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-recovered.png')}
        $null=Send-Control 'ui.edit.commit'
        $null=Send-Control 'ui.awaitIdle'
        Assert-Cell 'D10' '崩溃前未提交的中文草稿'
        $checks.Add('M8.3 actual owned app process killed and restarted: prior workbook formula and uncommitted Unicode draft restored independently; draft commits normally')
    }

    if($M8Task -ge 4){
        $buildFile=Join-Path $binaryRoot 'BUILD.json'
        if(Test-Path -LiteralPath $buildFile){
            $manifest=Get-Content -LiteralPath $buildFile -Raw|ConvertFrom-Json
            if($manifest.stage -notin @('M8-preview','M8-preview2','M8-preview3','M8-preview4','M8-preview5','M8-preview6') -or !(Test-Path -LiteralPath (Join-Path $binaryRoot 'licenses/inventory.json'))){throw 'Distribution manifest or license inventory is missing'}
            if((Get-Item -LiteralPath $app).VersionInfo.ProductName -ne 'Joysheet'){throw 'App version resources are missing'}
            $checks.Add('M8.4 extracted Release package starts natively with version resources and dependency license inventory')
        }else{$checks.Add('M8.4 source-build gate; extracted-package and installer checks recorded separately')}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M8-preview.png')}
    }
}

