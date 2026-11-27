param([Parameter(Mandatory)] [string] $Output)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing.Common
New-Item -ItemType Directory -Path $Output | Out-Null
$Output = (Resolve-Path -LiteralPath $Output).Path

# Two colors and an independent native repetition check, without shipping runtime GDI+ code.
$side = 32
$swatches = foreach ($style in 6..52) {
    $bitmap = [System.Drawing.Bitmap]::new($side, $side)
    $canvas = [System.Drawing.Graphics]::FromImage($bitmap)
    $brush = [System.Drawing.Drawing2D.HatchBrush]::new(
        [System.Drawing.Drawing2D.HatchStyle]$style, [System.Drawing.Color]::Lime, [System.Drawing.Color]::Magenta)
    try {
        $canvas.FillRectangle($brush, 0, 0, $side, $side)
        $bits = foreach ($y in 0..($side - 1)) {
            foreach ($x in 0..($side - 1)) {
                $pixel = $bitmap.GetPixel($x, $y)
                if ($pixel.ToArgb() -eq [System.Drawing.Color]::Lime.ToArgb()) { 1 }
                elseif ($pixel.ToArgb() -eq [System.Drawing.Color]::Magenta.ToArgb()) { 0 }
                else { throw "Unexpected native color for hatch $style" }
            }
        }
        [ordered]@{style = $style; side = $side; bits = $bits}
    } finally {
        $brush.Dispose()
        $canvas.Dispose()
        $bitmap.Dispose()
    }
}
$swatches | ConvertTo-Json -Depth 5 -Compress | Set-Content -LiteralPath (Join-Path $Output 'hatches.json') -Encoding utf8

# Generate a real GDI+ stream containing a filled cap at the start and a stroked cap at the end.
$bitmap = [System.Drawing.Bitmap]::new(128, 128)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$hdc = $graphics.GetHdc()
try {
    $metafile = [System.Drawing.Imaging.Metafile]::new(
        (Join-Path $Output 'windows-caps.emf'), $hdc, [System.Drawing.Rectangle]::new(0, 0, 128, 128),
        [System.Drawing.Imaging.MetafileFrameUnit]::Pixel, [System.Drawing.Imaging.EmfType]::EmfPlusOnly)
} finally {
    $graphics.ReleaseHdc($hdc)
    $graphics.Dispose()
    $bitmap.Dispose()
}
$canvas = [System.Drawing.Graphics]::FromImage($metafile)
$fill = [System.Drawing.Drawing2D.GraphicsPath]::new()
$line = [System.Drawing.Drawing2D.GraphicsPath]::new()
$fill.AddPolygon([System.Drawing.PointF[]]@(
    [System.Drawing.PointF]::new(-1, -1), [System.Drawing.PointF]::new(3, -1), [System.Drawing.PointF]::new(0, 4)))
$line.AddLines([System.Drawing.PointF[]]@(
    [System.Drawing.PointF]::new(-2, 1), [System.Drawing.PointF]::new(0, 3), [System.Drawing.PointF]::new(2, 1)))
$start = [System.Drawing.Drawing2D.CustomLineCap]::new($fill, $null, [System.Drawing.Drawing2D.LineCap]::Flat, 0)
$end = [System.Drawing.Drawing2D.CustomLineCap]::new($null, $line, [System.Drawing.Drawing2D.LineCap]::Flat, 0)
$pen = [System.Drawing.Pen]::new([System.Drawing.Color]::Red, 5)
try {
    $pen.CustomStartCap = $start
    $pen.CustomEndCap = $end
    $canvas.DrawLine($pen, 30, 66, 82, 66)
} finally {
    $pen.Dispose()
    $end.Dispose()
    $start.Dispose()
    $line.Dispose()
    $fill.Dispose()
    $canvas.Dispose()
    $metafile.Dispose()
}
