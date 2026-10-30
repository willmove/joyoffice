#requires -Version 7.0
<#
Word 2024 background exchange probe, using only original repository fixtures.
This is automated COM evidence, not visual/GUI acceptance. Refuse to attach
when a Word process is already running. Never overwrite inputs or outputs.
Documents.Open / SaveAs2 / AutomationSecurity follow the Microsoft Word VBA API.
#>
param(
    [Parameter(Mandatory)][string]$Out,
    [string]$Cli = 'target/debug/joyword-cli.exe'
)
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$outPath = [IO.Path]::GetFullPath($Out, $repo)
$cliPath = [IO.Path]::GetFullPath($Cli, $repo)
if (Get-Process WINWORD -ErrorAction SilentlyContinue) {
    throw 'Word is already running; keep the user session untouched and retry in an agreed slot.'
}
if (-not (Test-Path -LiteralPath $cliPath -PathType Leaf)) { throw 'Build joyword-cli first.' }
if (Test-Path -LiteralPath $outPath) { throw 'Choose a new output directory.' }
New-Item -ItemType Directory -Path $outPath | Out-Null
$report = [ordered]@{
    schema = 1
    scope = 'automated-word-com-exchange; no-visual-acceptance'
    head = (git -C $repo rev-parse HEAD)
    cli_sha256 = (Get-FileHash -LiteralPath $cliPath -Algorithm SHA256).Hash.ToLowerInvariant()
    word_version = $null
    word_build = $null
    cases = @()
    status = 'incomplete'
}
$classId = (Get-ItemProperty 'Registry::HKEY_CLASSES_ROOT\Word.Application\CLSID').'(default)'
$server = (Get-ItemProperty "Registry::HKEY_CLASSES_ROOT\CLSID\$classId\LocalServer32").'(default)'
$report.com_server = $server
if ($server -notmatch '(?i)winword\.exe') {
    $report.status = 'blocked-environment'
    $report.reason = 'Word.Application is registered to a different product; no application started, no registry changes made.'
    $report | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $outPath 'summary.json') -Encoding utf8
    Write-Output $report.reason
    exit 2
}
$word = $null
$documents = $null
$oldSecurity = $null
$ownsApplication = $false
function Inspect-Cli([string]$Path, [string]$Log) {
    $result = & $cliPath inspect $Path 2> $Log
    if ($LASTEXITCODE -ne 0) { throw "CLI inspect failed ($LASTEXITCODE): $Path" }
    return ($result | ConvertFrom-Json -AsHashtable)
}
try {
    $word = New-Object -ComObject Word.Application
    $documents = $word.Documents
    if ([string]$word.Name -notmatch 'Microsoft Word' -or [int]$documents.Count -ne 0 -or [double]::Parse([string]$word.Version, [cultureinfo]::InvariantCulture) -lt 16) {
        throw 'Unexpected application identity or existing documents; no user application state was changed.'
    }
    $ownsApplication = $true
    $word.Visible = $false
    $word.DisplayAlerts = 0
    $oldSecurity = $word.AutomationSecurity
    $word.AutomationSecurity = 3
    $report.word_version = [string]$word.Version
    $report.word_build = [string]$word.Build
    $inputs = @(Get-ChildItem -LiteralPath (Join-Path $repo 'tests/fixtures/feature') -Filter '*.docx' | Sort-Object Name)
    foreach ($inputFile in $inputs) {
        $document = $null
        $reopened = $null
        $case = [ordered]@{ id = $inputFile.BaseName; source_sha256 = (Get-FileHash -LiteralPath $inputFile.FullName).Hash.ToLowerInvariant(); status = 'incomplete' }
        try {
            $before = Inspect-Cli $inputFile.FullName (Join-Path $outPath "$($case.id)-before.stderr")
            $copy = Join-Path $outPath "$($case.id)-word.docx"
            $pdf = Join-Path $outPath "$($case.id)-word.pdf"
            $missing = [Type]::Missing
            # Visible is argument 12. No conversion dialog, no recent-file entry,
            # read-only source, no repair mode or encoding dialog.
            $document = $documents.Open($inputFile.FullName, $false, $true, $false, $missing, $missing, $false, $missing, $missing, $missing, $missing, $false, $false, $false, $missing, $true)
            $wordText = [string]$document.Content.Text
            $case.word_text_before = $wordText
            $case.word_tables = [int]$document.Tables.Count
            $case.word_revisions = [int]$document.Revisions.Count
            $case.word_inline_shapes = [int]$document.InlineShapes.Count
            $case.word_floating_shapes = [int]$document.Shapes.Count
            $document.SaveAs2($copy, 12, $false, $missing, $false)
            $document.ExportAsFixedFormat($pdf, 17)
            $document.Close(0)
            [void][Runtime.InteropServices.Marshal]::ReleaseComObject($document)
            $document = $null
            $reopened = $documents.Open($copy, $false, $true, $false, $missing, $missing, $false, $missing, $missing, $missing, $missing, $false, $false, $false, $missing, $true)
            $case.word_text_after = [string]$reopened.Content.Text
            $case.word_text_equal = $case.word_text_after -ceq $wordText
            $reopened.Close(0)
            [void][Runtime.InteropServices.Marshal]::ReleaseComObject($reopened)
            $reopened = $null
            $after = Inspect-Cli $copy (Join-Path $outPath "$($case.id)-after.stderr")
            $case.joyword_projection_equal = $before.text -ceq $after.text
            $case.joyword_before = $before
            $case.joyword_after = $after
            $case.copy_sha256 = (Get-FileHash -LiteralPath $copy).Hash.ToLowerInvariant()
            $case.pdf_sha256 = (Get-FileHash -LiteralPath $pdf).Hash.ToLowerInvariant()
            $case.source_unchanged = (Get-FileHash -LiteralPath $inputFile.FullName).Hash.ToLowerInvariant() -ceq $case.source_sha256
            $case.status = if ($case.word_text_equal -and $case.joyword_projection_equal -and $case.source_unchanged) { 'passed-text-projection' } else { 'difference' }
        } catch {
            $case.status = 'failed'
            $case.error = $_.Exception.Message
        } finally {
            foreach ($opened in @($document, $reopened)) {
                if ($null -ne $opened) {
                    try { $opened.Close(0) } catch { }
                    [void][Runtime.InteropServices.Marshal]::ReleaseComObject($opened)
                }
            }
            $report.cases += $case
            $report | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $outPath 'summary.json') -Encoding utf8
            Write-Output "$($case.id): $($case.status)"
        }
    }
    $report.status = if (@($report.cases | Where-Object status -ne 'passed-text-projection').Count -eq 0) { 'passed-text-projection' } else { 'differences-or-failures' }
} finally {
    if ($null -ne $word -and $ownsApplication) {
        if ($null -ne $oldSecurity) { $word.AutomationSecurity = $oldSecurity }
        $word.Quit(0)
    }
    if ($null -ne $documents) { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($documents) }
    if ($null -ne $word) { [void][Runtime.InteropServices.Marshal]::ReleaseComObject($word) }
    $report | ConvertTo-Json -Depth 30 | Set-Content -LiteralPath (Join-Path $outPath 'summary.json') -Encoding utf8
}
if ($report.status -ne 'passed-text-projection') { exit 1 }
