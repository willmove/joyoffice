# Exercise the installed Microsoft PowerPoint and both JoyDeck readers with test copies.
param(
    [Parameter(Mandatory)] [string[]] $Samples,
    [Parameter(Mandatory)] [string] $Evidence,
    [Parameter(Mandatory)] [string] $FrozenCli,
    [Parameter(Mandatory)] [string] $CandidateCli,
    [string] $PowerPoint = "$env:ProgramFiles/Microsoft Office/root/Office16/POWERPNT.EXE"
)

$ErrorActionPreference = 'Stop'
$PowerPoint = (Resolve-Path -LiteralPath $PowerPoint).Path
$FrozenCli = (Resolve-Path -LiteralPath $FrozenCli).Path
$CandidateCli = (Resolve-Path -LiteralPath $CandidateCli).Path
$Samples = @($Samples | ForEach-Object { (Resolve-Path -LiteralPath $_).Path })
$Evidence = $ExecutionContext.SessionState.Path.GetUnresolvedProviderPathFromPSPath($Evidence)
if (Test-Path -LiteralPath $Evidence) {
    throw "Evidence directory already exists: $Evidence"
}
$null = New-Item -ItemType Directory -Path $Evidence

Add-Type @"
using System;
using System.Runtime.InteropServices;

public static class MicrosoftPowerPointRot
{
    [DllImport("oleaut32.dll", PreserveSig = false)]
    private static extern void GetActiveObject(ref Guid clsid, IntPtr reserved,
        [MarshalAs(UnmanagedType.IUnknown)] out object application);

    public static object ActiveApplication()
    {
        Guid clsid = new Guid("91493441-5A91-11CF-8700-00AA0060263B");
        GetActiveObject(ref clsid, IntPtr.Zero, out object application);
        return application;
    }
}
"@

function Get-PowerPoint {
    try {
        [MicrosoftPowerPointRot]::ActiveApplication()
    }
    catch [System.Runtime.InteropServices.COMException] {
        $null
    }
}

function Get-DocumentShape($Presentation) {
    foreach ($slide in $Presentation.Slides) {
        $shapeTypes = @(foreach ($shape in $slide.Shapes) { [int] $shape.Type })
        [ordered] @{
            id = [int] $slide.SlideID
            shapeTypes = $shapeTypes
            effects = [int] $slide.TimeLine.MainSequence.Count
            notes = [int] $slide.NotesPage.Shapes.Count
        }
    }
}

$powerPointApp = Get-PowerPoint
$warmup = $null
if ($null -eq $powerPointApp) {
    $warmup = Join-Path $Evidence ('powerpoint-warmup' + [IO.Path]::GetExtension($Samples[0]))
    Copy-Item -LiteralPath $Samples[0] -Destination $warmup
    Start-Process -FilePath $PowerPoint -ArgumentList ('"' + $warmup + '"') -WindowStyle Normal
    for ($attempt = 0; $attempt -lt 30 -and $null -eq $powerPointApp; $attempt++) {
        Start-Sleep -Milliseconds 500
        $powerPointApp = Get-PowerPoint
    }
}
if ($null -eq $powerPointApp -or $powerPointApp.Version -ne '16.0') {
    throw 'Microsoft PowerPoint 16 did not register an active application'
}
if ($null -ne $warmup) {
    foreach ($presentation in $powerPointApp.Presentations) {
        if ($presentation.FullName -eq $warmup) {
            $presentation.Close()
            break
        }
    }
}

$records = foreach ($sample in $Samples) {
    $index = [array]::IndexOf($Samples, $sample) + 1
    $stem = [IO.Path]::GetFileNameWithoutExtension($sample)
    $output = Join-Path $Evidence ('{0:00}-{1}-powerpoint.pptx' -f $index, $stem)
    $inputHash = (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash.ToLowerInvariant()
    $presentation = $powerPointApp.Presentations.Open($sample, -1, 0, 0)
    try {
        $before = @(Get-DocumentShape $presentation)
        if ($before.Count -eq 0) {
            throw "PowerPoint opened no slides: $sample"
        }
        $presentation.SaveCopyAs($output, 24)
    } finally {
        $presentation.Close()
    }
    $copy = $powerPointApp.Presentations.Open($output, -1, 0, 0)
    try {
        $after = @(Get-DocumentShape $copy)
        if ((ConvertTo-Json -InputObject $before -Depth 4 -Compress) -cne
            (ConvertTo-Json -InputObject $after -Depth 4 -Compress)) {
            throw "PowerPoint changed slide IDs, shape types, effects, or notes during SaveCopyAs: $sample"
        }
    } finally {
        $copy.Close()
    }
    $frozen = (& $FrozenCli content $output) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw "Frozen JoyDeck cannot reopen: $output" }
    $candidate = (& $CandidateCli content $output) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw "Candidate JoyDeck cannot reopen: $output" }
    if ($frozen -cne $candidate -or ((ConvertFrom-Json $candidate).slides).Count -ne $after.Count) {
        throw "JoyDeck readers disagree after PowerPoint save: $output"
    }
    if ($inputHash -ne (Get-FileHash -LiteralPath $sample -Algorithm SHA256).Hash) {
        throw "PowerPoint changed the input instead of saving a copy: $sample"
    }
    Write-Host ("PowerPoint round trip passed: {0}, {1} slides" -f $stem, $before.Count)
    [ordered] @{
        input = ('{0}/{1}' -f (Split-Path (Split-Path $sample) -Leaf), (Split-Path $sample -Leaf))
        inputSha256 = $inputHash
        output = Split-Path $output -Leaf
        outputSha256 = (Get-FileHash -LiteralPath $output -Algorithm SHA256).Hash.ToLowerInvariant()
        slides = @($before)
    }
}

[ordered] @{
    version = (Get-Item -LiteralPath $PowerPoint).VersionInfo.FileVersion
    applicationVersion = $powerPointApp.Version
    frozenCliSha256 = (Get-FileHash -LiteralPath $FrozenCli -Algorithm SHA256).Hash.ToLowerInvariant()
    candidateCliSha256 = (Get-FileHash -LiteralPath $CandidateCli -Algorithm SHA256).Hash.ToLowerInvariant()
    cases = @($records)
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $Evidence 'verification.json') -Encoding utf8
