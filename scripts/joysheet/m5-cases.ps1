# Native production form acceptance, called inside desktop-smoke's isolated application.
function Test-DataForm([string]$Name,[hashtable]$Fields=@{}) {
    $null=Send-Control 'ui.dialog' @{name=$Name}
    foreach($field in $Fields.Keys){$null=Send-Control 'ui.dialog.set' @{field=$field;value=$Fields[$field]}}
    $null=Send-Control 'ui.dialog.confirm'
    $null=Wait-View {param($v) $v.dialog -eq '' -and -not $v.error} $Name
}
function Invoke-M5Workflow {
    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Wait-View {param($v) ($v.windows | Where-Object window -eq $v.window).path -eq $null} 'M5 workbook'
    $null=Send-Control 'engine.execute' @{command='range.setValues';params=@{range='A1';values=@(@('地区','金额'),@('东区',30),@('西区',10),@('东区',20),@('东区',20))}}
    Test-DataForm 'data.sort' @{range='A1:B5';column='B';order='asc';header=$true}
    Assert-Cell 'B2' 10
    Test-DataForm 'edit.find' @{what='西区'}
    $null=Wait-View {param($v) $v.selection -eq 'A2'} 'find navigated to match'
    Test-DataForm 'edit.replace' @{what='西区';with='北区';all=$true}
    Assert-Cell 'A2' '北区'
    Test-DataForm 'data.removeDuplicates' @{range='A1:B5';columns='A,B';header=$true}
    Assert-Cell 'A5' $null
    Test-DataForm 'data.filter' @{range='A1:B4'}
    Test-DataForm 'data.filterBy' @{column='B';operator='>';value='15'}
    $filtered=Send-Control 'ui.inspect'
    if ($filtered.worksheet.hiddenRows -notcontains 2){throw 'Column filter did not hide the low amount row.'}
    Test-DataForm 'data.clearFilter'
    $checks.Add('M5.1 native sort/find/replace/deduplicate/filter forms and workbook results')
    if ($M5Task -ge 2) {
        Test-DataForm 'insert.table' @{range='A1:B4';header=$true;style='TableStyleMedium2'}
        Test-DataForm 'home.conditionalFormat' @{range='B2:B4';type='dataBar';value=15}
        Test-DataForm 'data.validation' @{range='C2:C4';type='whole';formula1='0';formula2='100'}
        $state=Send-Control 'ui.inspect'
        if($state.worksheet.tables -ne 1 -or $state.worksheet.conditionalFormats -ne 1 -or $state.worksheet.validations -ne 1){throw 'Table/CF/validation models missing.'}
        $null=Send-Control 'ui.cell.click' @{cell='C2'}
        $null=Send-Control 'ui.text' @{text='999'}
        $null=Send-Control 'ui.key' @{key='enter'}
        $null=Wait-View {param($v) $v.error -and $null -ne $v.editor} 'validation rejection preserves draft'
        Assert-Cell 'C2' $null
        $null=Send-Control 'ui.edit.cancel'
        Type-Cell 'C2' '50'
        Assert-Cell 'C2' 50
        $checks.Add('M5.2 styled table, conditional formatting, validation rejection and successful edit')
    }
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M5-workflow.png')}
    if ($M5Task -ge 3) {
        Test-DataForm 'data.group' @{by='rows';lines='2:3'}
        Test-DataForm 'data.hideDetail' @{by='rows';lines='2:3'}
        $hidden=Send-Control 'ui.inspect'
        if($hidden.worksheet.hiddenRows -notcontains 2 -or $hidden.worksheet.hiddenRows -notcontains 3){throw 'Outline collapse failed.'}
        Test-DataForm 'data.showDetail' @{by='rows';lines='2:3'}
        Test-DataForm 'data.ungroup' @{by='rows';lines='2:3'}
        $expanded=Send-Control 'ui.inspect'
        if($expanded.worksheet.hiddenRows -contains 2){throw 'Outline expansion failed.'}
        $checks.Add('M5.3 grouping, collapse, expand and ungroup through production forms')
    }
    if ($M5Task -ge 4) {
        $csv=Join-Path $artifacts 'M5-import.csv'
        "名称,数额`n甲,11`n乙,22" | Set-Content -LiteralPath $csv -Encoding utf8
        $null=Send-Control 'ui.dialog' @{name='data.getData'}
        $null=Send-Control 'ui.dialog.set' @{field='path';value=$csv}
        $null=Send-Control 'ui.dialog.set' @{field='sheet';value='M5导入'}
        $null=Send-Control 'ui.import.preview'
        $preview=Wait-View {param($v) $v.dataResultKind -eq '__joysheet.previewImport' -and $v.dataResults.Count -gt 0} 'import preview'
        if($preview.sheetCount -ne 1){throw 'Import preview changed the workbook.'}
        Test-DataForm 'data.getData' @{path=$csv;text='';sheet='M5导入'}
        Assert-Cell 'B3' 22
        Test-DataForm 'data.getData' @{path='';text="[`n{`"Name`":`"Gamma`",`"Value`":33},`n{`"Name`":`"Delta`",`"Value`":44}`n]";sheet='JSON导入'}
        Assert-Cell 'B3' 44
        Test-DataForm 'data.queriesConnections'
        $null=Wait-View {param($v) $v.dataResultKind -eq 'data.queriesConnections' -and $v.dataResults.Count -ge 3} 'imported table connection list'
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M5-connections.png')}
        $saved=Join-Path $artifacts 'M5-workflow.xlsx'
        $null=Send-Control 'ui.command' @{command='file.save';params=@{path=$saved}}
        $null=Wait-View {param($v) -not ($v.windows | Where-Object window -eq $v.window).dirty} 'M5 save'
        $null=Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}
        $null=Send-Control 'app.open' @{path=$saved}
        $null=Wait-View {param($v) ($v.windows | Where-Object window -eq $v.window).path -eq $saved} 'M5 reopen'
        $null=Send-Control 'engine.execute' @{command='sheet.activate';params=@{sheet='M5导入'}}
        Assert-Cell 'B3' 22
        $checks.Add('M5.4 local CSV preview without mutation, import, connections list and XLSX roundtrip')
    }
}
