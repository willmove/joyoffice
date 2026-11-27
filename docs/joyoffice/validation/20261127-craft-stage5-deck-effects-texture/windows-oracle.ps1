param([Parameter(Mandatory)] [string] $Output)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing.Common
Add-Type -CompilerOptions '/unsafe' -TypeDefinition @'
using System;
using System.Reflection;
using System.Runtime.InteropServices;

public static class NativeDeckEffect
{
    [DllImport("gdiplus")] static extern int GdipCreateEffect(Guid id, out IntPtr effect);
    [DllImport("gdiplus")] static extern int GdipDeleteEffect(IntPtr effect);
    [DllImport("gdiplus")] static extern int GdipSetEffectParameters(IntPtr effect, IntPtr data, int size);
    [DllImport("gdiplus")] static extern int GdipCreateBitmapFromScan0(int w, int h, int stride, int format, IntPtr data, out IntPtr bitmap);
    [DllImport("gdiplus")] static extern int GdipBitmapSetPixel(IntPtr bitmap, int x, int y, int color);
    [DllImport("gdiplus")] static extern int GdipBitmapGetPixel(IntPtr bitmap, int x, int y, out int color);
    [DllImport("gdiplus")] static extern int GdipBitmapCreateApplyEffect(IntPtr[] input, int count, IntPtr effect, IntPtr roi, [Out] int[] outputRect, out IntPtr output, int useAux, IntPtr aux, IntPtr auxSize);
    [DllImport("gdiplus")] static extern int GdipGetImageWidth(IntPtr image, out int width);
    [DllImport("gdiplus")] static extern int GdipGetImageHeight(IntPtr image, out int height);
    [DllImport("gdiplus")] static extern int GdipDisposeImage(IntPtr bitmap);
    [DllImport("gdiplus")] static extern int GdipDrawDriverString(IntPtr graphics, ushort[] glyphs, int count, IntPtr font, IntPtr brush, float[] positions, int options, IntPtr matrix);

    static unsafe IntPtr NativePointer(object value, string name)
    {
        var pointer = value.GetType().GetProperty(name, BindingFlags.Instance | BindingFlags.NonPublic);
        return (IntPtr)Pointer.Unbox(pointer.GetValue(value));
    }

    public static void DriverString(object graphics, object font, object brush, float[] positions)
    {
        // Arial glyph index 43 is H. Submit known glyph IDs so the native renderer skips shaping.
        ushort[] glyphs = { 43, 43, 43, 43 };
        Check("DrawDriverString", GdipDrawDriverString(NativePointer(graphics, "NativeGraphics"),
            glyphs, glyphs.Length, NativePointer(font, "NativeFont"), NativePointer(brush, "NativeBrush"),
            positions, 0, IntPtr.Zero));
    }

    static void Check(string name, int result)
    {
        if (result != 0) throw new InvalidOperationException(name + " returned " + result);
    }

    public static int[][] Apply(string id, int[] args, int width, int height, int[] colors, bool redEye)
    {
        IntPtr regions = IntPtr.Zero, data = IntPtr.Zero, source = IntPtr.Zero, dest = IntPtr.Zero, effect = IntPtr.Zero;
        try
        {
            Check("CreateBitmap", GdipCreateBitmapFromScan0(width, height, 0, 0x0026200a, IntPtr.Zero, out source));
            for (var y = 0; y < height; y++)
                for (var x = 0; x < width; x++)
                    Check("SetPixel", GdipBitmapSetPixel(source, x, y, colors[y * width + x]));
            Check("CreateEffect", GdipCreateEffect(Guid.Parse(id), out effect));
            int size = args.Length * 4;
            if (redEye)
            {
                regions = Marshal.AllocHGlobal(size - 4);
                Marshal.Copy(args, 1, regions, args.Length - 1);
                size += 2 * IntPtr.Size - 4;
                data = Marshal.AllocHGlobal(size);
                Marshal.WriteIntPtr(data, new IntPtr(args[0]));
                Marshal.WriteIntPtr(data, IntPtr.Size, regions);
            }
            else
            {
                data = Marshal.AllocHGlobal(size);
                Marshal.Copy(args, 0, data, args.Length);
            }
            Check("SetEffectParameters", GdipSetEffectParameters(effect, data, size));
            var rect = new int[4];
            Check("CreateApplyEffect", GdipBitmapCreateApplyEffect(new[] { source }, 1, effect, IntPtr.Zero, rect, out dest, 0, IntPtr.Zero, IntPtr.Zero));
            int w, h;
            Check("GetWidth", GdipGetImageWidth(dest, out w));
            Check("GetHeight", GdipGetImageHeight(dest, out h));
            var pixels = new int[w * h];
            for (var y = 0; y < h; y++)
                for (var x = 0; x < w; x++)
                    Check("GetPixel", GdipBitmapGetPixel(dest, x, y, out pixels[y * w + x]));
            return new[] { new[] { w, h }, rect, pixels };
        }
        finally
        {
            if (dest != IntPtr.Zero) GdipDisposeImage(dest);
            if (source != IntPtr.Zero) GdipDisposeImage(source);
            if (effect != IntPtr.Zero) GdipDeleteEffect(effect);
            if (regions != IntPtr.Zero) Marshal.FreeHGlobal(regions);
            if (data != IntPtr.Zero) Marshal.FreeHGlobal(data);
        }
    }
}
'@

$null = New-Item -ItemType Directory -Path $Output -Force
$Output = (Resolve-Path -LiteralPath $Output).Path
$reference = [Drawing.Bitmap]::new(100, 100)
$screen = [Drawing.Graphics]::FromImage($reference)
$screen.PageUnit = 'Pixel'

function Write-Case($Name, [scriptblock] $Paint) {
    $hdc = $screen.GetHdc()
    try {
        $bounds = [Drawing.Rectangle]::new(0, 0, 100, 100)
        $meta = [Drawing.Imaging.Metafile]::new((Join-Path $Output "$Name.emf"), $hdc,
            $bounds, [Drawing.Imaging.MetafileFrameUnit]::Pixel, [Drawing.Imaging.EmfType]::EmfPlusOnly)
    } finally { $screen.ReleaseHdc($hdc) }
    try {
        $recording = [Drawing.Graphics]::FromImage($meta)
        try {
            $recording.PageUnit = 'Pixel'
            & $Paint $recording
        } finally { $recording.Dispose() }
    } finally { $meta.Dispose() }
    & $Paint $screen
    $reference.Save((Join-Path $Output "$Name.png"), 'Png')
}

function FloatBits([single] $Number) { [BitConverter]::SingleToInt32Bits($Number) }

$ids = @{
    blur = '633C80A4-1843-482B-9EF2-BE2834C5FDD4'
    sharpen = '63CBF3EE-C526-402C-8F71-62C540BF5142'
    brightness = 'D3A1DBE1-8EC4-4C17-9F4C-EA97AD1C343D'
    curve = 'DD6A0022-58E4-4A67-9D9B-D48EB881A53D'
    levels = '99C354EC-2A31-4F3A-8C34-17A803B33A25'
    balance = '537E597D-251E-48DA-9664-29CA496B70F8'
    hsl = '8B2DD6C3-EB07-4D87-A5F0-7108E26A9C5F'
    tint = '1077AF00-2848-4441-9489-44AD4C2D7A2C'
    redeye = '74D29D05-69A4-4266-9549-3CC52836B632'
}

$effects = [ordered] @{}
function Observe($Label, $Kind, [int[]] $Params, [int] $W, [int] $H, [int[]] $Colors) {
    try {
        $effects[$Label] = [NativeDeckEffect]::Apply($ids[$Kind], $Params, $W, $H, $Colors, $Kind -eq 'redeye')
    } catch {
        $effects[$Label] = $_.Exception.GetBaseException().Message
        Write-Verbose "$Label ${Kind} rejected: $($effects[$Label])"
    }
}

try {
    $ramp = [int[]]::new(256 * 4)
    for ($i = 0; $i -lt 256; $i++) {
        $ramp[$i] = [Drawing.Color]::FromArgb(255, $i, $i, $i).ToArgb()
        $ramp[$i + 256] = [Drawing.Color]::FromArgb(255, $i, (255 - $i), (($i * 7) % 256)).ToArgb()
        $ramp[$i + 512] = [Drawing.Color]::FromArgb(127, $i, 83, 165).ToArgb()
        $ramp[$i + 768] = [Drawing.Color]::FromArgb(255, $i, 32, 12).ToArgb()
    }
    foreach ($value in @(-100, -90, -75, -60, -50, -40, -30, -20, -10, 0, 10, 20, 30, 40, 50, 60, 75, 90, 100)) {
        Observe "contrast_$value" brightness @(0, $value) 256 4 $ramp
        Observe "brightness_$value" brightness @($value, 0) 256 4 $ramp
        Observe "midtone_$value" levels @(100, $value, 0) 256 4 $ramp
        Observe "balance_$value" balance @($value, 0, 0) 256 4 $ramp
        Observe "saturation_$value" hsl @(0, $value, 0) 256 4 $ramp
        Observe "light_$value" hsl @(0, 0, $value) 256 4 $ramp
        Observe "tint_$value" tint @(0, $value) 256 4 $ramp
    }
    Observe 'balance_mixed' balance @(21, -35, 62) 256 4 $ramp
    Observe 'hsl_mixed' hsl @(73, 31, -23) 256 4 $ramp
    Observe 'hsl_rotate' hsl @(73, 0, 0) 256 4 $ramp
    Observe 'tint_red' tint @(-120, 50) 256 4 $ramp
    $legacyColor = [Drawing.Color]::FromArgb(255, 64, 100, 140).ToArgb()
    Observe 'legacy_tint' tint @(-140, 30) 8 8 ([int[]] (@($legacyColor) * 64))
    $legacyEye = [Drawing.Color]::FromArgb(255, 200, 40, 60).ToArgb()
    Observe 'legacy_eye' redeye @(1, 0, 0, 4, 8) 8 8 ([int[]] (@($legacyEye) * 64))
    $legacyBlur = [int[]]::new(64)
    for ($y = 2; $y -le 5; $y++) {
        for ($x = 2; $x -le 5; $x++) { $legacyBlur[$y * 8 + $x] = [Drawing.Color]::Red.ToArgb() }
    }
    Observe 'legacy_expand' blur @((FloatBits 1), 1) 8 8 $legacyBlur
    Observe 'combined_bc' brightness @(30, 35) 256 4 $ramp
    Observe 'combined_bc_neg' brightness @(-30, 35) 256 4 $ramp
    Observe 'combined_bc_flat' brightness @(30, -35) 256 4 $ramp
    Observe 'levels_combined' levels @(85, 20, 10) 256 4 $ramp
    foreach ($value in @(15, 50, 85)) {
        Observe "highlight_$value" levels @($value, 0, 0) 256 4 $ramp
        Observe "shadow_$value" levels @(100, 0, $value) 256 4 $ramp
    }
    for ($mode = 0; $mode -lt 8; $mode++) {
        foreach ($strength in @(-100, -75, -60, -50, -30, -25, 0, 25, 30, 50, 60, 75, 100, 150, 255)) {
            Observe "curve_${mode}_$strength" curve @($mode, 0, $strength) 256 4 $ramp
        }
    }
    Observe 'curve_single' curve @(0, 2, 31) 256 4 $ramp

    $w = 61; $h = 61
    $spatial = [int[]]::new($w * $h)
    for ($y = 0; $y -lt $h; $y++) {
        for ($x = 0; $x -lt $w; $x++) {
            $r = if ($x -lt 30) { 96 } else { 160 }
            $g = if ($y -lt 30) { 64 } else { 164 }
            $b = if ($x -eq 30 -and $y -eq 30) { 220 } else { 128 }
            $a = if ($x -lt 10) { 128 } else { 255 }
            $spatial[$y * $w + $x] = [Drawing.Color]::FromArgb($a, $r, $g, $b).ToArgb()
        }
    }
    foreach ($radius in @(0.5, 1, 2, 3, 6, 10, 19.5, 20, 32)) {
        Observe "sharp_$radius" sharpen @((FloatBits $radius), (FloatBits 60)) $w $h $spatial
        foreach ($expand in @(0, 1)) {
            Observe "blur_${radius}_$expand" blur @((FloatBits $radius), $expand) $w $h $spatial
        }
    }
    foreach ($amount in @(10, 20, 25, 30, 40, 50, 60, 70, 75, 80, 90, 95, 100)) {
        Observe "sharp_3_$amount" sharpen @((FloatBits 3), (FloatBits $amount)) $w $h $spatial
    }
    $red = [int[]]::new(64 * 64)
    $eyeColors = @(
        @(255, 240, 30, 40), @(255, 160, 70, 70), @(255, 140, 70, 110), @(255, 150, 120, 50),
        @(255, 220, 75, 110), @(255, 80, 30, 35), @(255, 190, 140, 145), @(128, 230, 74, 49),
        @(255, 255, 0, 0), @(255, 250, 200, 85), @(255, 110, 65, 12), @(255, 170, 90, 10),
        @(255, 120, 80, 110), @(255, 120, 110, 100), @(255, 225, 168, 85), @(255, 250, 250, 250))
    for ($y = 0; $y -lt 64; $y++) {
        for ($x = 0; $x -lt 64; $x++) {
            $c = $eyeColors[4 * ($y -shr 4) + ($x -shr 4)]
            $red[$y * 64 + $x] = [Drawing.Color]::FromArgb($c[0], $c[1], $c[2], $c[3]).ToArgb()
        }
    }
    Observe 'redeye' redeye @(1, 5, 5, 58, 58) 64 64 $red
    Observe 'redeye_multiple' redeye @(2, 5, 5, 58, 58, 12, 12, 44, 44) 64 64 $red
    $pupils = [int[]]::new(64 * 64)
    for ($y = 0; $y -lt 64; $y++) {
        for ($x = 0; $x -lt 64; $x++) {
            $d = [Math]::Min(($x - 21) * ($x - 21), ($x - 45) * ($x - 45)) + ($y - 32) * ($y - 32)
            $c = if ($d -le 16) { @(234, 31, 45) } elseif ($d -le 64) { @(96, 72, 58) } else { @(209, 159, 127) }
            $pupils[$y * 64 + $x] = [Drawing.Color]::FromArgb(255, $c[0], $c[1], $c[2]).ToArgb()
        }
    }
    Observe 'pupils' redeye @(2, 11, 22, 32, 43, 35, 22, 56, 43) 64 64 $pupils
    Observe 'pupils_offcenter' redeye @(1, 5, 17, 30, 45) 64 64 $pupils
    $effects.inputs = @{ ramp = $ramp; spatial = $spatial; red = $red; pupils = $pupils }
    ConvertTo-Json -InputObject $effects -Depth 5 -Compress |
        Set-Content -LiteralPath (Join-Path $Output 'native-effects.json') -Encoding utf8

    $solid = [Drawing.Bitmap]::new(22, 20)
    $g = [Drawing.Graphics]::FromImage($solid)
    try { $g.Clear([Drawing.Color]::FromArgb(255, 220, 80, 32)) } finally { $g.Dispose() }
    $stripes = [Drawing.Bitmap]::new(80, 48)
    for ($y = 0; $y -lt 48; $y++) {
        for ($x = 0; $x -lt 80; $x++) {
            $r = if (($x % 4) -eq 0) { 255 } else { 0 }
            $g = if (($y % 4) -lt 2) { 255 } else { 0 }
            $b = if ($x -ge 40) { 192 } else { 32 }
            $stripes.SetPixel($x, $y, [Drawing.Color]::FromArgb(255, $r, $g, $b))
        }
    }
    try {
        Write-Case 'expanded' {
            param($canvas)
            $canvas.Clear([Drawing.Color]::Transparent)
            $canvas.InterpolationMode = 'NearestNeighbor'
            $canvas.CompositingMode = 'SourceCopy'
            foreach ($item in @(@(3, 12, 14, 0), @(20, 58, 14, 0), @(3, 12, 60, 6), @(20, 58, 60, 15))) {
                $fx = [Drawing.Imaging.Effects.BlurEffect]::new($item[0], $true)
                $map = [Drawing.Drawing2D.Matrix]::new(1, 0, 0, 1, $item[1], $item[2])
                $p = $item[3]
                try {
                    $canvas.DrawImage($solid, $fx, [Drawing.RectangleF]::new(-$p, -$p, 22 + 2 * $p, 20 + 2 * $p),
                        $map, 'Pixel', $null)
                } finally { $map.Dispose(); $fx.Dispose() }
            }
        }
        Write-Case 'downsample' {
            param($canvas)
            $canvas.Clear([Drawing.Color]::White)
            $fx = [Drawing.Imaging.Effects.BlurEffect]::new(3, $false)
            try {
                $modes = @('NearestNeighbor', 'Bilinear', 'Bicubic', 'HighQualityBilinear', 'HighQualityBicubic')
                for ($i = 0; $i -lt $modes.Count; $i++) {
                    $canvas.InterpolationMode = $modes[$i]
                    foreach ($item in @(@(7, $null), @(40, $fx))) {
                        $map = [Drawing.Drawing2D.Matrix]::new(.25, 0, 0, .25, $item[0], 3 + $i * 19)
                        try {
                            $canvas.DrawImage($stripes, $item[1], [Drawing.RectangleF]::new(0, 0, 80, 48),
                                $map, 'Pixel', $null)
                        } finally { $map.Dispose() }
                    }
                }
            } finally { $fx.Dispose() }
        }
    } finally { $solid.Dispose(); $stripes.Dispose() }

    $font = [Drawing.Font]::new('Arial', 26, 'Regular', 'Pixel')
    $tile = [Drawing.Bitmap]::new(12, 8)
    for ($y = 0; $y -lt 8; $y++) {
        for ($x = 0; $x -lt 12; $x++) {
            $color = if ($x -lt 4) { [Drawing.Color]::FromArgb(128, 255, 0, 0) }
                elseif ($x -lt 8) { [Drawing.Color]::Transparent }
                else { [Drawing.Color]::FromArgb(255, 0, 0, 255) }
            $tile.SetPixel($x, $y, $color)
        }
    }
    try {
        Write-Case 'texture-glyphs' {
            param($canvas)
            $canvas.Clear([Drawing.Color]::Lime)
            $canvas.TextRenderingHint = 'AntiAlias'
            foreach ($row in @(@(0, 26, 'Tile', 3), @(1, 56, 'TileFlipX', 3), @(1, 86, 'Clamp', 18))) {
                $canvas.CompositingMode = [Drawing.Drawing2D.CompositingMode] $row[0]
                $brush = [Drawing.TextureBrush]::new($tile, [Drawing.Drawing2D.WrapMode] $row[2])
                try {
                    $brush.ScaleTransform(1.5, 2.0)
                    $brush.TranslateTransform($row[3], 1, 'Append')
                    $y = $row[1]
                    [NativeDeckEffect]::DriverString($canvas, $font, $brush, [single[]] @(7, $y, 23, $y, 41, $y, 64, $y))
                } finally { $brush.Dispose() }
            }
        }
    } finally { $tile.Dispose(); $font.Dispose() }
} finally {
    $screen.Dispose()
    $reference.Dispose()
}
