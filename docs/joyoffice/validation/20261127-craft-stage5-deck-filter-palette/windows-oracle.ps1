param([Parameter(Mandatory)] [string] $Output)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing.Common
$null = New-Item -ItemType Directory -Path $Output -Force
$Output = (Resolve-Path -LiteralPath $Output).Path
$bounds = [System.Drawing.Rectangle]::new(0, 0, 100, 100)
$reference = [System.Drawing.Bitmap]::new(100, 100)
$screen = [System.Drawing.Graphics]::FromImage($reference)
$screen.PageUnit = 'Pixel'

function Write-Case($Name, [scriptblock] $Paint) {
    $hdc = $screen.GetHdc()
    try {
        $meta = [System.Drawing.Imaging.Metafile]::new((Join-Path $Output "$Name.emf"), $hdc,
            $bounds, [System.Drawing.Imaging.MetafileFrameUnit]::Pixel,
            [System.Drawing.Imaging.EmfType]::EmfPlusOnly)
    } finally { $screen.ReleaseHdc($hdc) }
    try {
        $recording = [System.Drawing.Graphics]::FromImage($meta)
        try {
            $recording.PageUnit = 'Pixel'
            & $Paint $recording
        } finally { $recording.Dispose() }
    } finally { $meta.Dispose() }
    & $Paint $screen
    $reference.Save((Join-Path $Output "$Name.png"), 'Png')
}

function Observe-Blur($Name, [float] $Radius, $Source, [float] $Scale, [float] $Origin) {
    $effect = [System.Drawing.Imaging.Effects.BlurEffect]::new($Radius, $false)
    $map = [System.Drawing.Drawing2D.Matrix]::new($Scale, 0, 0, $Scale, $Origin, $Origin)
    try {
        Write-Case $Name {
            param($canvas)
            $canvas.Clear([System.Drawing.Color]::Black)
            $canvas.InterpolationMode = 'NearestNeighbor'
            $rect = [System.Drawing.RectangleF]::new(0, 0, $Source.Width, $Source.Height)
            $canvas.DrawImage($Source, $effect, $rect, $map, 'Pixel', $null)
        }
        if ($Name.StartsWith('blur-')) {
            $filtered = $Source.Clone()
            try {
                $filtered.ApplyEffect($effect)
                $filtered.Save((Join-Path $Output "$Name-bitmap.png"), 'Png')
            } finally { $filtered.Dispose() }
        }
    } finally {
        $map.Dispose()
        $effect.Dispose()
    }
}

try {
    $steps = [System.Drawing.Bitmap]::new(601, 601)
    $g = [System.Drawing.Graphics]::FromImage($steps)
    try {
        $g.Clear([System.Drawing.Color]::Black)
        $g.FillRectangle([System.Drawing.Brushes]::Red, 300, 0, 301, 300)
        $g.FillRectangle([System.Drawing.Brushes]::Lime, 0, 300, 300, 301)
        $g.FillRectangle([System.Drawing.Brushes]::Yellow, 300, 300, 301, 301)
    } finally { $g.Dispose() }
    try {
        Observe-Blur 'blur-19_5' 19.5 $steps 0.15 5
        Observe-Blur 'blur-20' 20 $steps 0.15 5
        Observe-Blur 'blur-32' 32 $steps 0.15 5
        Observe-Blur 'blur-255' 255 $steps 0.15 5
    } finally { $steps.Dispose() }

    $format = [System.Drawing.Imaging.PixelFormat]::Format32bppPArgb
    $color = [System.Drawing.Color]::FromArgb(64, 250, 145, 73)
    $flat = [System.Drawing.Bitmap]::new(76, 76, $format)
    $g = [System.Drawing.Graphics]::FromImage($flat)
    try { $g.Clear($color) } finally { $g.Dispose() }
    $samples = [ordered] @{}
    try {
        foreach ($radius in @(3, 20)) {
            $label = "alpha-$radius"
            Observe-Blur $label $radius $flat 1 12
            $filtered = $flat.Clone()
            $effect = [System.Drawing.Imaging.Effects.BlurEffect]::new($radius, $false)
            try {
                $filtered.ApplyEffect($effect)
                $region = [System.Drawing.Rectangle]::new(38, 38, 1, 1)
                $data = $filtered.LockBits($region, 'ReadOnly', $format)
                try {
                    $bytes = [byte[]]::new(4)
                    [Runtime.InteropServices.Marshal]::Copy($data.Scan0, $bytes, 0, 4)
                    $samples[$label] = @($bytes[2], $bytes[1], $bytes[0], $bytes[3])
                } finally { $filtered.UnlockBits($data) }
            } finally { $effect.Dispose(); $filtered.Dispose() }
        }
    } finally { $flat.Dispose() }

    $palettes = [ordered] @{}
    foreach ($depth in @(4, 8)) {
        $bitmap = [System.Drawing.Bitmap]::new(8, 8, "Format${depth}bppIndexed")
        try { $palettes["$depth"] = @($bitmap.Palette.Entries | ForEach-Object { ,@($_.R, $_.G, $_.B) }) }
        finally { $bitmap.Dispose() }
    }
    @{ premultiplied = $samples; palettes = $palettes } | ConvertTo-Json -Depth 6 -Compress |
        Set-Content -LiteralPath (Join-Path $Output 'native-values.json') -Encoding utf8

    $font = [System.Drawing.Font]::new('Arial', 23, 'Regular', 'Pixel')
    $start = [System.Drawing.Color]::FromArgb(0, 255, 0, 0)
    $end = [System.Drawing.Color]::FromArgb(128, 255, 0, 0)
    $gradient = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
        [System.Drawing.RectangleF]::new(0, 0, 99, 100), $start, $end,
        [System.Drawing.Drawing2D.LinearGradientMode]::Horizontal)
    $hatch = [System.Drawing.Drawing2D.HatchBrush]::new('Horizontal', $end, $start)
    try {
        Write-Case 'brush-string' {
            param($canvas)
            $canvas.Clear([System.Drawing.Color]::Lime)
            $canvas.TextRenderingHint = 'AntiAlias'
            $canvas.DrawString('HHHHH', $font, $gradient, [System.Drawing.PointF]::new(5, 2))
            $canvas.CompositingMode = 'SourceCopy'
            $canvas.DrawString('HHHHH', $font, $gradient, [System.Drawing.PointF]::new(5, 32))
            $canvas.DrawString('HHHHH', $font, $hatch, [System.Drawing.PointF]::new(5, 62))
        }
    } finally { $hatch.Dispose(); $gradient.Dispose(); $font.Dispose() }
} finally {
    $screen.Dispose()
    $reference.Dispose()
}
