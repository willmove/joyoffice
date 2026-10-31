#Requires -Version 7.0
param([Parameter(Mandatory)][string]$Reference,[string]$Candidate='target/release/joysheet-cli.exe',[string]$Report='docs/evidence/core-parity.json')
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
$output=Join-Path $root 'artifacts/core-parity'
New-Item -ItemType Directory -Force $output|Out-Null
$steps=@(
 @{command='range.setValues';params=@{range='A1';values=@(@('地区','数量','价格'),@('东区',2,3),@('西区',4,5),@('东区',6,7),@('南区',8,9),@('东区',10,11))}},
 @{command='cell.set';params=@{cell='E1';input='=SUM(B2:B6)'}},
 @{command='cell.set';params=@{cell='E2';input='=SUMIFS(B2:B6,A2:A6,"东区")'}},
 @{command='cell.set';params=@{cell='E3';input='=SUMPRODUCT(B2:B6,C2:C6)'}},
 @{command='cell.set';params=@{cell='E4';input='=IF(SUM(B2:B6)>0,SUM(B2:B6)+SUM(C2:C6),0)'}},
 @{command='cell.set';params=@{cell='E5';input='=SUM(B2:B6)'}},
 @{command='cell.set';params=@{cell='F1';input='=SEQUENCE(3)'}},
 @{command='cell.set';params=@{cell='G1';input='=SUM(F1:F3)'}},
 @{command='cell.set';params=@{cell='G2';input='=SUM(F1:F3)'}},
 @{command='cell.set';params=@{cell='H1';input='=1/0'}},
 @{command='cell.set';params=@{cell='H2';input='=SUM(H1:H3)'}},
 @{command='cell.set';params=@{cell='H3';input='=AVERAGE(B2:B6)'}},
 @{command='cell.set';params=@{cell='B4';input='12'}},
 @{command='edit.undo';params=@{}},@{command='edit.redo';params=@{}},
 @{command='home.bold';params=@{range='A1:E5'}},
 @{command='formulas.calculateNow';params=@{}}
)
$script=Join-Path $output 'commands.jsonl'
$steps|ForEach-Object{$_|ConvertTo-Json -Depth 20 -Compress}|Set-Content -LiteralPath $script -Encoding utf8NoBOM
$tools=@((Resolve-Path -LiteralPath $Reference).Path,(Resolve-Path -LiteralPath $Candidate).Path)
foreach($index in 0..1){
 $path=Join-Path $output "$index.json"
 & $tools[$index] run --script $script --out $path --quiet
 if($LASTEXITCODE -ne 0){throw "Parity commands failed: $index"}
 & $tools[$index] convert $path (Join-Path $output "$index.xlsx")
 if($LASTEXITCODE -ne 0){throw "Parity XLSX save failed: $index"}
}
$a=Get-Content (Join-Path $output '0.json') -Raw|ConvertFrom-Json -Depth 100|ConvertTo-Json -Depth 100 -Compress
$b=Get-Content (Join-Path $output '1.json') -Raw|ConvertFrom-Json -Depth 100|ConvertTo-Json -Depth 100 -Compress
if($a -cne $b){throw 'Persisted workbook content differs from previous release'}
foreach($formulas in @($false,$true)){
 $expected=$null
 foreach($index in 0..1){foreach($reader in 0..1){
  $readerArgs=@('cat',(Join-Path $output "$index.xlsx"),'--range','A1:H8')
  if($formulas){${readerArgs}+='--formulas'}
  $text=(& $tools[$reader] @readerArgs) -join "`n"
  if($LASTEXITCODE -ne 0){throw 'Cross-version XLSX reopen failed'}
  if($null -eq $expected){$expected=$text}elseif($text -cne $expected){throw 'Cross-version XLSX values/formulas differ'}
 }}
}
@{passed=$true;commands=$steps.Count;reference=$tools[0];candidate=$tools[1];referenceSha256=(Get-FileHash -LiteralPath $tools[0] -Algorithm SHA256).Hash.ToLowerInvariant();candidateSha256=(Get-FileHash -LiteralPath $tools[1] -Algorithm SHA256).Hash.ToLowerInvariant();persistedWorkbookEqual=$true;crossVersionXlsxReaders=2;checks=@('SUM','SUMIFS','SUMPRODUCT','nested','spill','errors','undo-redo','format','full-recalc','JSON parity','cross-version XLSX values and formulas')}|ConvertTo-Json -Depth 6|Set-Content ([IO.Path]::GetFullPath($Report,$root)) -Encoding utf8
'Core parity and cross-version XLSX reopening passed.'
