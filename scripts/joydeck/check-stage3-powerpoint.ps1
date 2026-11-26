# Exercise the installed Microsoft PowerPoint and both JoyDeck readers with test copies.
param(
    [Parameter(Mandatory)] [string[]] $Samples,
    [Parameter(Mandatory)] [string] $Evidence,
    [Parameter(Mandatory)] [string] $FrozenCli,
    [Parameter(Mandatory)] [string] $CandidateCli,
    [string] $PowerPoint = "$env:ProgramFiles/Microsoft Office/root/Office16/POWERPNT.EXE",
    [switch] $ReportChartTypes,
    [switch] $ReportDrawingObjects,
    [switch] $ReportAnimationTimeline,
    [string[]] $ComboSeriesOrder
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

function Get-SequenceEffects($Sequence) {
    foreach ($effect in $Sequence) {
        $timing = $effect.Timing
        $trigger = if ($timing.TriggerType -eq 4) { [int] $timing.TriggerShape.Id }
        [ordered] @{
            target = [int] $effect.Shape.Id
            effect = [int] $effect.EffectType
            start = [int] $timing.TriggerType
            trigger = $trigger
            duration = [single] $timing.Duration
            delay = [single] $timing.TriggerDelayTime
            autoReverse = [bool] $timing.AutoReverse
        }
    }
}

function Get-DocumentShape($Presentation) {
    foreach ($slide in $Presentation.Slides) {
        $shapeTypes = @(foreach ($shape in $slide.Shapes) { [int] $shape.Type })
        $record = [ordered] @{
            id = [int] $slide.SlideID
            shapeTypes = $shapeTypes
            effects = [int] $slide.TimeLine.MainSequence.Count
            notes = [int] $slide.NotesPage.Shapes.Count
        }
        if ($ReportChartTypes) {
            $record.chartTypes = @(foreach ($shape in $slide.Shapes) {
                if ($shape.HasChart -eq -1) { [int] $shape.Chart.ChartType }
            })
        }
        if ($ReportDrawingObjects) {
            $record.drawingObjects = @(foreach ($shape in $slide.Shapes) {
                switch ([int] $shape.Type) {
                    24 {
                        [ordered] @{
                            id = [int] $shape.Id
                            layout = [string] $shape.SmartArt.Layout.Id
                            nodes = @(foreach ($node in $shape.SmartArt.AllNodes) {
                                [string] $node.TextFrame2.TextRange.Text
                            })
                        }
                    }
                    30 {
                        $model = $shape.Model3D
                        [ordered] @{
                            id = [int] $shape.Id
                            angles = @($model.RotationX, $model.RotationY, $model.RotationZ)
                            camera = @($model.CameraPositionX, $model.CameraPositionY, $model.CameraPositionZ)
                            fieldOfView = $model.FieldOfView
                        }
                    }
                    default {
                        if ($shape.HasTextFrame -eq -1 -and $shape.TextFrame.HasText -eq -1) {
                            [ordered] @{ id = [int] $shape.Id; text = [string] $shape.TextFrame.TextRange.Text }
                        }
                    }
                }
            })
        }
        if ($ReportAnimationTimeline) {
            $record.mainSequence = @(Get-SequenceEffects $slide.TimeLine.MainSequence)
            $record.interactiveSequences = @(foreach ($sequence in $slide.TimeLine.InteractiveSequences) {
                @{ effects = @(Get-SequenceEffects $sequence) }
            })
        }
        $record
    }
}

function Test-ComboOrder($FrozenModel, $CandidateModel) {
    $frozenDeck = ConvertFrom-Json $FrozenModel -AsHashtable -Depth 100
    $candidateDeck = ConvertFrom-Json $CandidateModel -AsHashtable -Depth 100
    $old = @($frozenDeck.slides.shapes.kind | Where-Object { $_.type -eq 'chart' -and $_.kind -eq 'combo' })
    $new = @($candidateDeck.slides.shapes.kind | Where-Object { $_.type -eq 'chart' -and $_.kind -eq 'combo' })
    if ($old.Count -ne 1 -or $new.Count -ne 1 -or
        $old[0].series.Count -ne $ComboSeriesOrder.Count -or
        ($new[0].series.name -join "`n") -cne ($ComboSeriesOrder -join "`n")) {
        return $false
    }
    $ordered = @(foreach ($name in $ComboSeriesOrder) {
        $series = @($old[0].series | Where-Object { $_.name -ceq $name })
        if ($series.Count -ne 1) { return $false }
        $series[0]
    })
    $old[0].series = $ordered
    (ConvertTo-Json -InputObject $frozenDeck -Depth 100 -Compress) -ceq
        (ConvertTo-Json -InputObject $candidateDeck -Depth 100 -Compress)
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
        if ((ConvertTo-Json -InputObject $before -Depth 8 -Compress) -cne
            (ConvertTo-Json -InputObject $after -Depth 8 -Compress)) {
            throw "PowerPoint changed the checked slide properties during SaveCopyAs: $sample"
        }
    } finally {
        $copy.Close()
    }
    $frozen = (& $FrozenCli content $output) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw "Frozen JoyDeck cannot reopen: $output" }
    $candidate = (& $CandidateCli content $output) -join "`n"
    if ($LASTEXITCODE -ne 0) { throw "Candidate JoyDeck cannot reopen: $output" }
    $orderOnly = $frozen -cne $candidate -and $ComboSeriesOrder -and (Test-ComboOrder $frozen $candidate)
    if (($frozen -cne $candidate -and -not $orderOnly) -or
        ((ConvertFrom-Json $candidate).slides).Count -ne $after.Count) {
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
        seriesOrderDifference = [bool] $orderOnly
    }
}

[ordered] @{
    version = (Get-Item -LiteralPath $PowerPoint).VersionInfo.FileVersion
    applicationVersion = $powerPointApp.Version
    frozenCliSha256 = (Get-FileHash -LiteralPath $FrozenCli -Algorithm SHA256).Hash.ToLowerInvariant()
    candidateCliSha256 = (Get-FileHash -LiteralPath $CandidateCli -Algorithm SHA256).Hash.ToLowerInvariant()
    cases = @($records)
} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $Evidence 'verification.json') -Encoding utf8
