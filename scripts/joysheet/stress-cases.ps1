# Invoked inside desktop-smoke.ps1; uses the production engine and GPUI edit path.
$stress = [System.Collections.Generic.List[object]]::new()
$stressMachine=@{os=[Environment]::OSVersion.VersionString;cpu=@(Get-CimInstance Win32_Processor | ForEach-Object Name);binarySha256=(Get-FileHash -LiteralPath $app -Algorithm SHA256).Hash.ToLowerInvariant();coreTracing=(Test-Path Env:GRIDCRAFT_PROFILE)}
function Measure-Stress([string]$Name, [string]$Cell, [scriptblock]$Text, [scriptblock]$Verify, [int]$ExpectedFormulas) {
    $null=Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference=$Cell}}
    $null=Wait-View {param($v) $v.selection -eq $Cell} "$Name selection"
    $null=Send-Control 'ui.perf' @{reset=$true}
    for($n=1;$n -le 12;$n++) {
        $null=Send-Control 'ui.cell.click' @{cell=$Cell}
        $null=Send-Control 'ui.edit.text' @{text=(& $Text $n)}
        $null=Send-Control 'ui.edit.commit'
        $null=Send-Control 'ui.awaitIdle'
        & $Verify $n
    }
    $sample=Send-Control 'ui.perf' @{raw=$true}
    if($sample.stages.editCount -ne 12 -or $sample.stages.pending -ne 0 -or $sample.stages.discarded -ne 0){throw "Missing stress samples: $Name; count=$($sample.stages.editCount) pending=$($sample.stages.pending) discarded=$($sample.stages.discarded)"}
    if(@($sample.stages.edits |Where-Object {$_.formulasEvaluated -ne $ExpectedFormulas}).Count){throw "Unexpected evaluated formula count: $Name"}
    $started.Refresh()
    $stress.Add(@{name=$Name;size=$rowCount;machine=$stressMachine;workingSetBytes=$started.WorkingSet64;samples=$sample})
    $stress.ToArray() | ConvertTo-Json -Depth 24 | Set-Content -LiteralPath (Join-Path $artifacts 'stress-performance.json') -Encoding utf8
}
foreach($rowCount in $StressSizes) {
    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Send-Control 'engine.execute' @{command='home.rowHeight';params=@{rows="1:$rowCount";height=26}}
    $null=Send-Control 'engine.execute' @{command='home.columnWidth';params=@{cols='A:ALL';width=92}}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell="T$rowCount";input='1'}}
    Measure-Stress 'custom-dimensions' 'A1' {param($n) "$n"} {param($n) Assert-Cell 'A1' $n} 0
    # Resized/hidden axes, undo and far hit testing exercise invalidation.
    $null=Send-Control 'engine.execute' @{command='home.rowHeight';params=@{rows='1:2';height=39}}
    $null=Send-Control 'engine.execute' @{command='edit.undo';params=@{}}
    $null=Send-Control 'engine.execute' @{command='home.hideRows';params=@{rows='1:2'}}
    $null=Send-Control 'engine.execute' @{command='home.hideColumns';params=@{cols='A:B'}}
    $null=Send-Control 'engine.execute' @{command='edit.goTo';params=@{reference="T$rowCount"}}
    $null=Wait-View {param($v) $v.selection -eq "T$rowCount"} 'custom far row'
    $null=Send-Control 'ui.cell.click' @{cell="T$rowCount"}
    Assert-Cell "T$rowCount" 1
    $null=Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}

    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $values = [System.Collections.Generic.List[object]]::new()
    for($i=1;$i -le $rowCount;$i++) {$values.Add(@("=`$A`$1+$i"))}
    $null=Send-Control 'engine.execute' @{command='range.setValues';params=@{range='B1';values=$values.ToArray()}}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='D1';input="=SUM(B1:B$rowCount)"}}
    Measure-Stress 'dense-formulas' 'A1' {param($n) "$n"} {param($n)
        Assert-Cell "B$rowCount" ($rowCount+$n)
        Assert-Cell 'D1' ([double]$rowCount*($rowCount+1)/2+$rowCount*$n)
    } ($rowCount+1)
    $null=Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}

    $null=Send-Control 'engine.execute' @{command='file.new';params=@{}}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='A1';input="=SEQUENCE($rowCount,1,0,1)"}}
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='C1';input="=SUM(A1:A$rowCount)"}}
    Measure-Stress 'dynamic-array' 'A1' {param($n) "=SEQUENCE($rowCount,1,$n,1)"} {param($n)
        Assert-Cell "A$rowCount" ($rowCount+$n-1)
        Assert-Cell 'C1' ([double]$rowCount*($rowCount+1)/2+$rowCount*($n-1))
    } 3
    # Shrink, blocker/unblock and undo must preserve spill dependents.
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='A1';input='=SEQUENCE(3)'}}
    Assert-Cell "A$rowCount" $null
    Assert-Cell 'C1' 6
    $null=Send-Control 'engine.execute' @{command='cell.set';params=@{cell='A2';input='99'}}
    if((Send-Control 'cell.get' @{cell='A1'}).value.error -ne '#SPILL!'){throw 'Array blocker did not produce #SPILL!'}
    $null=Send-Control 'engine.execute' @{command='edit.undo';params=@{}}
    Assert-Cell 'A3' 3
    $null=Send-Control 'engine.execute' @{command='file.close';params=@{force=$true;document=$true}}
}
$checks.Add('custom 10k/100k row heights and 1000 column widths; dense 10k/100k dependent formulas; dynamic 10k/100k spills with real values, shrink/block/undo')
