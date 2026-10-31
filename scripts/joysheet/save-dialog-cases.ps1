# Actual production button clicks; never call save_then_close through a test control shortcut.
function Click-CloseButton([string]$Name) {
    $v=Wait-View {param($v) $v.dialog -eq 'saveChanges' -and $v.closeButtons.save.width -gt 0} 'close dialog layout'
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'save-dialog-current.png')}
    $v=Send-Control 'ui.inspect'
    $b=$v.closeButtons.$Name
    $null=Send-Control 'ui.click' @{x=($b.x+$b.width/2);y=($b.y+$b.height/2)}
}
function Invoke-SaveDialogCases {
    $path=Join-Path $artifacts 'save-dialog-roundtrip.xlsx'
    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Wait-View {param($v) ($v.windows|Where-Object window -eq $v.window).path -eq $null} 'save dialog workbook'
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='A1';input='原始已保存内容'}}
    $null=Send-Control 'engine.execute' @{command='insert.shape';params=@{kind='rectangle';at='J10';width=500;height=360;text='弹窗下方的对象'}}
    $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$path}}
    $originalHash=(Get-FileHash -LiteralPath $path).Hash
    $v=Send-Control 'ui.inspect';$objectId=$v.drawingObjects.shapes[0].id
    $null=Send-Control 'engine.execute' @{command='object.move';params=@{kind='shape';id=$objectId;width=333;dx=17}}
    $null=Send-Control 'ui.edit.text' @{text='关闭弹窗真正保存的中文'}
    $closing=(Send-Control 'ui.inspect').window
    $null=Send-Control 'ui.command' @{command='file.close'}
    $v=Wait-View {param($v) $v.dialog -eq 'saveChanges' -and $null -eq $v.editor} 'dirty draft committed before close dialog'
    $selection=$v.selection;$scroll=$v.scroll|ConvertTo-Json -Compress;$count=$v.windows.Count
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'save-dialog-blocking.png')}
    $null=Send-Control 'ui.cell.drag' @{from='B8';to='D9'}
    $null=Send-Control 'ui.scroll' @{x=($v.grid.x+150);y=($v.grid.y+150);dy=-240;dx=0}
    $null=Send-Control 'ui.text' @{text='不应穿透到工作表'}
    $null=Send-Control 'ui.key' @{key='z';ctrl=$true}
    $null=Send-Control 'ui.key' @{key='n';ctrl=$true}
    $v=Send-Control 'ui.inspect'
    if($v.selection -ne $selection -or ($v.scroll|ConvertTo-Json -Compress) -ne $scroll -or $v.windows.Count -ne $count -or $v.editor -or $v.selectedObject){throw 'Close dialog leaked input to worksheet'}
    Assert-Cell 'A1' '关闭弹窗真正保存的中文'
    Click-CloseButton 'cancel'
    $null=Wait-View {param($v) $v.dialog -eq '' -and $v.window -eq $closing} 'actual cancel button'
    if((Get-FileHash -LiteralPath $path).Hash -ne $originalHash){throw 'Cancel saved unexpectedly'}
    $null=Send-Control 'ui.command' @{command='file.close'}
    Click-CloseButton 'save'
    $null=Wait-View {param($v) @($v.windows|Where-Object window -eq $closing).Count -eq 0} 'actual save button saved then closed'
    if((Get-FileHash -LiteralPath $path).Hash -eq $originalHash){throw 'Save button did not change the file'}
    $null=Send-Control 'engine.execute' @{command='file.open';params=@{path=$path}}
    $v=Wait-View {param($v) ($v.windows|Where-Object window -eq $v.window).path -eq $path} 'reopen saved dialog workbook'
    Assert-Cell 'A1' '关闭弹窗真正保存的中文'
    if($v.drawingObjects.shapes[0].anchor.width -ne 333 -or $v.drawingObjects.shapes[0].anchor.dx -ne 17){throw 'Object changes were not saved'}
    $checks.Add('Actual close dialog: backdrop/keyboard blocked, Cancel preserves file, Save persists cell/object then closes; XLSX reopened')
    $closing=$v.window
    Type-Cell 'A1' '保存失败后重试保留的中文'
    $failureSelection=(Send-Control 'ui.inspect').selection
    $null=Send-Control 'ui.command' @{command='file.close'}
    $lock=[IO.File]::Open($path,[IO.FileMode]::Open,[IO.FileAccess]::ReadWrite,[IO.FileShare]::None)
    try {
        Click-CloseButton 'save'
        $v=Wait-View {param($v) $v.error -and $v.dialog -eq 'saveChanges' -and -not $v.closeSaving} 'failed save retains dialog and enables retry'
        if($v.window -ne $closing -or $v.selection -ne $failureSelection){throw 'Failed save changed the target'}
        Assert-Cell 'A1' '保存失败后重试保留的中文'
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'save-dialog-error.png')}
    } finally {$lock.Dispose()}
    Click-CloseButton 'save'
    $null=Wait-View {param($v) @($v.windows|Where-Object window -eq $closing).Count -eq 0} 'save retry closes only after success'
    $null=Send-Control 'engine.execute' @{command='file.open';params=@{path=$path}}
    $null=Wait-View {param($v) ($v.windows|Where-Object window -eq $v.window).path -eq $path} 'reopen retried save'
    Assert-Cell 'A1' '保存失败后重试保留的中文'
    $checks.Add('Actual close dialog: exclusive file lock causes visible save failure without closing; retry persists and reopens successfully')
    $hash=(Get-FileHash -LiteralPath $path).Hash;$closing=(Send-Control 'ui.inspect').window
    Type-Cell 'A1' '不保存应放弃的内容'
    $null=Send-Control 'ui.command' @{command='file.close'}
    Click-CloseButton 'discard'
    $null=Wait-View {param($v) @($v.windows|Where-Object window -eq $closing).Count -eq 0} 'actual discard button'
    if((Get-FileHash -LiteralPath $path).Hash -ne $hash){throw 'Discard wrote the file'}
    $checks.Add('Actual close dialog Discard closes without writing the file')
}
