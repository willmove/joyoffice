param([Parameter(Mandatory)] [string] $Output)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing.Common
$null = New-Item -ItemType Directory -Path $Output -Force
$Output = (Resolve-Path -LiteralPath $Output).Path

$reference = [System.Drawing.Bitmap]::new(96, 96)
$screen = [System.Drawing.Graphics]::FromImage($reference)
$image = [System.Drawing.Bitmap]::new(8, 8)
$paint = [System.Drawing.Graphics]::FromImage($image)
$paint.Clear([System.Drawing.Color]::FromArgb(255, 180, 40, 80))
$paint.Dispose()

$matrix = [System.Drawing.Imaging.ColorMatrix]::new()
$matrix.Matrix00 = $matrix.Matrix11 = $matrix.Matrix22 = 0
$matrix.Matrix01 = $matrix.Matrix12 = $matrix.Matrix20 = 1
$matrix.Matrix33 = 0.5
$matrix.Matrix40 = 0.125
$colors = [System.Drawing.Imaging.Effects.ColorMatrixEffect]::new($matrix)

function Write-Matrix($canvas) {
    $canvas.PageUnit = [System.Drawing.GraphicsUnit]::Pixel
    $canvas.Clear([System.Drawing.Color]::White)
    $transform = [System.Drawing.Drawing2D.Matrix]::new(6, 0, 0, 6, 20, 20)
    try {
        $canvas.DrawImage($image, $colors, [System.Drawing.RectangleF]::new(0, 0, 8, 8),
            $transform, [System.Drawing.GraphicsUnit]::Pixel, $null)
    } finally {
        $transform.Dispose()
    }
}

try {
    $hdc = $screen.GetHdc()
    $filename = Join-Path $Output 'matrix-native.emf'
    try {
        $meta = [System.Drawing.Imaging.Metafile]::new($filename, $hdc,
            [System.Drawing.Rectangle]::new(0, 0, 96, 96),
            [System.Drawing.Imaging.MetafileFrameUnit]::Pixel,
            [System.Drawing.Imaging.EmfType]::EmfPlusOnly)
    } finally {
        $screen.ReleaseHdc($hdc)
    }
    try {
        $writer = [System.Drawing.Graphics]::FromImage($meta)
        try { Write-Matrix $writer } finally { $writer.Dispose() }
    } finally {
        $meta.Dispose()
    }
    Write-Matrix $screen
    $reference.Save((Join-Path $Output 'matrix-native.png'), [System.Drawing.Imaging.ImageFormat]::Png)
    $color = $reference.GetPixel(40, 40)
    $matrixSample = @($color.R, $color.G, $color.B)

    foreach ($y in 0..7) {
        foreach ($x in 0..7) {
            $pixel = if ($x -lt 4) { [System.Drawing.Color]::Red } else { [System.Drawing.Color]::Blue }
            $image.SetPixel($x, $y, $pixel)
        }
    }
    $swatches = foreach ($i in 0..3) {
        $attributes = [System.Drawing.Imaging.ImageAttributes]::new()
        $wrap = [System.Drawing.Drawing2D.WrapMode]@(4, 1, 0, 4)[$i]
        $top = $i * 20 + 5
        $attributes.SetWrapMode($wrap, [System.Drawing.Color]::FromArgb(255, 0, 220, 240), $i -eq 3)
        try {
            $screen.DrawImage($image, [System.Drawing.Rectangle]::new(10, $top, 80, 15),
                [single]-4, [single]0, [single]16, [single]8, [System.Drawing.GraphicsUnit]::Pixel, $attributes)
            $probes = foreach ($x in 20, 39, 60, 81) {
                $pixel = $reference.GetPixel($x, $top + 7)
                ,@($pixel.R, $pixel.G, $pixel.B)
            }
            ,$probes
        } finally {
            $attributes.Dispose()
        }
    }

    $impulse = [System.Drawing.Bitmap]::new(25, 25)
    $blur = [System.Drawing.Imaging.Effects.BlurEffect]::new(3.0, $false)
    try {
        $impulse.SetPixel(12, 12, [System.Drawing.Color]::Red)
        $impulse.ApplyEffect($blur, [System.Drawing.Rectangle]::new(0, 0, 25, 25))
        $blurX = @(0..4 | ForEach-Object { $impulse.GetPixel(12 + $_, 12).A })
        $blurY = @(0..4 | ForEach-Object { $impulse.GetPixel(12, 12 + $_).A })
    } finally {
        $blur.Dispose()
        $impulse.Dispose()
    }
    [ordered] @{ matrix = $matrixSample; attributes = $swatches; blurImpulse = @{ x = $blurX; y = $blurY } } |
        ConvertTo-Json -Depth 5 -Compress | Set-Content -LiteralPath (Join-Path $Output 'windows.json') -Encoding utf8
} finally {
    $colors.Dispose()
    $image.Dispose()
    $screen.Dispose()
    $reference.Dispose()
}
