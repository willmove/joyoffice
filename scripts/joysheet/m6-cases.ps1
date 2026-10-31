function Invoke-M6Workflow {
    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Wait-View {param($v) ($v.windows | Where-Object window -eq $v.window).path -eq $null} 'M6 workbook'
    $null=Send-Control 'engine.execute' @{command='range.setValues';params=@{range='A1';values=@(@('地区','销售'),@('东区',10),@('西区',20),@('南区',30))}}
    Test-DataForm 'insert.chart' @{range='A1:B4';type='column';title='M6 图表';at='D2';width=400;height=260}
    $null=Wait-View {param($v) $v.drawingObjects.charts.Count -eq 1} 'chart insertion'
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M6-chart-before.png')}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='B2';input='80'}}
    $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M6-chart-after.png')}
    $checks.Add('M6.1 GPUI chart insertion, rendering and invalidation after data edit')
    if ($M6Task -ge 2) {
        Test-DataForm 'insert.picture' @{path=(Join-Path $workspace 'docs\joysheet\screenshots\sales.png');at='K2';width=160;height=100}
        Test-DataForm 'insert.shape' @{kind='ellipse';at='D17';text='M6 形状'}
        Test-DataForm 'insert.textBox' @{at='H17';text='中文文本框'}
        Test-DataForm 'insert.sparkline' @{range='B2:B4';location='C2'}
        $null=Wait-View {param($v) $v.decodedImages -ge 1 -and $v.drawingObjects.shapes.Count -eq 2} 'images decoded and shapes inserted'
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M6-objects.png')}
        $checks.Add('M6.2 native images, shapes, text boxes and sparklines')
    }
    if ($M6Task -ge 3) {
        $v=Send-Control 'ui.inspect';$b=@($v.objectBounds | Where-Object kind -eq 'shape')[0]
        $null=Send-Control 'ui.drag' @{x=($b.x+30);y=($b.y+20);toX=($b.x+70);toY=($b.y+50)}
        $v=Wait-View {param($v) $v.drawingObjects.shapes[0].anchor.dx -ge 39} 'object drag'
        if($v.selectedObject.kind -ne 'shape'){throw 'Pointer did not select shape'}
        $null=Send-Control 'ui.key' @{key='z';ctrl=$true}
        $null=Wait-View {param($v) $v.drawingObjects.shapes[0].anchor.dx -eq 0} 'object undo'
        $null=Send-Control 'ui.key' @{key='y';ctrl=$true}
        $v=Wait-View {param($v) $v.drawingObjects.shapes[0].anchor.dx -ge 39} 'object redo'
        $b=@($v.objectBounds | Where-Object kind -eq 'shape')[0]
        $null=Send-Control 'ui.drag' @{x=($b.x+$b.width);y=($b.y+$b.height);toX=($b.x+$b.width+30);toY=($b.y+$b.height+20)}
        $null=Wait-View {param($v) $v.drawingObjects.shapes[0].anchor.width -ge 209} 'object resize'
        Test-DataForm 'object.move' @{width=220;height=110}
        $null=Wait-View {param($v) $v.drawingObjects.shapes[0].anchor.width -eq 220} 'object property form'
        $null=Send-Control 'ui.key' @{key='delete'}
        $null=Wait-View {param($v) $v.drawingObjects.shapes.Count -eq 1} 'object delete'
        $null=Send-Control 'ui.key' @{key='z';ctrl=$true}
        $null=Wait-View {param($v) $v.drawingObjects.shapes.Count -eq 2} 'object delete undo'
        $saved=Join-Path $artifacts 'M6-workflow.xlsx'
        $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$saved}}
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M6-editing.png')}
        $checks.Add('M6.3 actual pointer selection/drag/resize, property form, delete, undo/redo and XLSX save')
    }
    if ($M6Task -ge 4) {
        Test-DataForm 'pageLayout.pageSetup' @{orientation='landscape';paper='A4';printArea='A1:N24';fitWidth=1;fitHeight=1;gridlines=$true;headings=$true}
        $pdf=Join-Path $artifacts 'M6-report.pdf'
        Test-DataForm 'file.exportPdf' @{path=$pdf}
        if ((Get-Item -LiteralPath $pdf).Length -lt 10000) {throw 'PDF export omitted drawings'}
        Test-DataForm 'file.printPreview'
        $v=Wait-View {param($v) $v.printStatus -eq 'preview ready' -and $v.printPreview.pages -ge 1} 'real PDF raster preview'
        Copy-Item -LiteralPath $v.printPreview.images[0] -Destination (Join-Path $artifacts 'M6-pdf-page.png')
        $null=Send-Control 'ui.screenshot' @{path=(Join-Path $artifacts 'M6-preview.png')}
        $null=Send-Control 'ui.print.close'
        Test-DataForm 'pageLayout.pageSetup' @{orientation='landscape';printArea='A1:N80';fitWidth=1;fitHeight=0}
        Test-DataForm 'file.printPreview'
        $null=Wait-View {param($v) $v.printPreview.pages -ge 2} 'multipage PDF preview'
        $null=Send-Control 'ui.key' @{key='pagedown'}
        $null=Wait-View {param($v) $v.printPreview.page -eq 2} 'preview page navigation'
        $null=Send-Control 'ui.text' @{text='must not reach worksheet'}
        Assert-Cell 'A1' '地区'
        $null=Send-Control 'ui.key' @{key='escape'}
        $null=Wait-View {param($v) $null -eq $v.printPreview -and $null -eq $v.editor} 'preview closes without worksheet edit'
        Test-DataForm 'pageLayout.pageSetup' @{orientation='landscape';printArea='A1:N24';fitWidth=1;fitHeight=1}
        $saved=Join-Path $artifacts 'M6-workflow.xlsx'
        $null=Send-Control 'engine.execute' @{command='file.saveAs';params=@{path=$saved}}
        $null=Send-Control 'engine.execute' @{command='file.open';params=@{path=$saved}}
        $null=Wait-View {param($v) $v.drawingObjects.shapes.Count -eq 2 -and $v.drawingObjects.charts.Count -eq 1 -and $v.decodedImages -ge 1} 'XLSX drawing roundtrip'
        $checks.Add('M6.4 page setup, real PDF export/raster preview and XLSX drawing roundtrip; printer dialog tested separately')
    }
}
