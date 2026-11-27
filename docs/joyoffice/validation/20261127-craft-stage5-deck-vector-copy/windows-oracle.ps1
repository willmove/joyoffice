param(
    [Parameter(Mandatory)] [string] $Output,
    [string] $Wmf
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing.Common
Add-Type @'
using System;
using System.Runtime.InteropServices;

public static class Stage5Gdi
{
    [DllImport("gdi32")] private static extern int SetROP2(IntPtr dc, int mode);
    [DllImport("gdi32")] private static extern IntPtr CreatePen(int style, int width, uint color);
    [DllImport("gdi32")] private static extern IntPtr CreateSolidBrush(uint color);
    [DllImport("gdi32")] private static extern IntPtr SelectObject(IntPtr dc, IntPtr obj);
    [DllImport("gdi32")] private static extern IntPtr GetStockObject(int index);
    [DllImport("gdi32")] private static extern bool DeleteObject(IntPtr obj);
    [DllImport("gdi32")] private static extern bool Rectangle(IntPtr dc, int x0, int y0, int x1, int y1);
    [DllImport("gdi32")] private static extern uint GetPixel(IntPtr dc, int x, int y);
    [DllImport("gdi32")] private static extern IntPtr SetMetaFileBitsEx(uint count, byte[] bits);
    [DllImport("gdi32")] private static extern bool PlayMetaFile(IntPtr dc, IntPtr metafile);
    [DllImport("gdi32")] private static extern bool DeleteMetaFile(IntPtr metafile);

    public static uint[] Rop(IntPtr dc, int mode)
    {
        var backdrop = CreateSolidBrush(0x3355aa);
        var brush = CreateSolidBrush(0xcc963c);
        var pen = CreatePen(0, 8, 0x1ac3ed);
        var oldPen = SelectObject(dc, GetStockObject(8));
        var oldBrush = SelectObject(dc, backdrop);
        try
        {
            SetROP2(dc, 13);
            if (!Rectangle(dc, 0, 0, 40, 40)) throw new InvalidOperationException("GDI background");
            SelectObject(dc, pen);
            SelectObject(dc, brush);
            SetROP2(dc, mode);
            if (!Rectangle(dc, 10, 10, 30, 30)) throw new InvalidOperationException("GDI rectangle");
            return new[] { GetPixel(dc, 20, 20), GetPixel(dc, 12, 20), GetPixel(dc, 7, 20), GetPixel(dc, 4, 20) };
        }
        finally
        {
            SetROP2(dc, 13);
            SelectObject(dc, oldPen);
            SelectObject(dc, oldBrush);
            DeleteObject(backdrop);
            DeleteObject(brush);
            DeleteObject(pen);
        }
    }

    public static uint[][] Wmf(IntPtr dc, byte[] bytes, int[][] points)
    {
        var backdrop = CreateSolidBrush(0xffffff);
        var oldPen = SelectObject(dc, GetStockObject(8));
        var oldBrush = SelectObject(dc, backdrop);
        var metafile = SetMetaFileBitsEx((uint)bytes.Length, bytes);
        try
        {
            Rectangle(dc, 0, 0, 100, 100);
            if (metafile == IntPtr.Zero || !PlayMetaFile(dc, metafile))
                throw new InvalidOperationException("GDI WMF playback");
            var result = new uint[points.Length][];
            for (int i = 0; i < points.Length; i++)
            {
                uint value = GetPixel(dc, points[i][0], points[i][1]);
                result[i] = new[] { value & 255, (value >> 8) & 255, (value >> 16) & 255 };
            }
            return result;
        }
        finally
        {
            SelectObject(dc, oldPen);
            SelectObject(dc, oldBrush);
            DeleteObject(backdrop);
            if (metafile != IntPtr.Zero) DeleteMetaFile(metafile);
        }
    }
}
'@

$null = New-Item -ItemType Directory -Path $Output -Force
$Output = (Resolve-Path -LiteralPath $Output).Path
$bounds = [System.Drawing.Rectangle]::new(0, 0, 100, 100)
$reference = [System.Drawing.Bitmap]::new(100, 100)
$screen = [System.Drawing.Graphics]::FromImage($reference)
$screen.PageUnit = [System.Drawing.GraphicsUnit]::Pixel

function Write-Metafile($Name, [scriptblock] $Paint) {
    $hdc = $screen.GetHdc()
    $file = Join-Path $Output "$Name.emf"
    try {
        $meta = [System.Drawing.Imaging.Metafile]::new($file, $hdc, $bounds,
            [System.Drawing.Imaging.MetafileFrameUnit]::Pixel, [System.Drawing.Imaging.EmfType]::EmfPlusOnly)
    } finally {
        $screen.ReleaseHdc($hdc)
    }
    try {
        $canvas = [System.Drawing.Graphics]::FromImage($meta)
        $canvas.PageUnit = [System.Drawing.GraphicsUnit]::Pixel
        try { & $Paint $canvas } finally { $canvas.Dispose() }
    } finally {
        $meta.Dispose()
    }
}

function Write-Case($Name, [scriptblock] $Paint) {
    Write-Metafile $Name $Paint
    & $Paint $screen
    $reference.Save((Join-Path $Output "$Name.png"), [System.Drawing.Imaging.ImageFormat]::Png)
}

try {
    if ($Wmf) {
        $file = (Resolve-Path -LiteralPath $Wmf).Path
        $data = [IO.File]::ReadAllBytes($file)
        $points = [int[][]](Get-Content -LiteralPath (Join-Path $Output 'wmf-points.json') -Raw | ConvertFrom-Json)
        $hdc = $screen.GetHdc()
        try { $values = [Stage5Gdi]::Wmf($hdc, $data, $points) } finally { $screen.ReleaseHdc($hdc) }
        ConvertTo-Json -InputObject $values -Depth 5 -Compress |
            Set-Content -LiteralPath (Join-Path $Output 'wmf-windows.json') -Encoding utf8
        return
    }

    $impulse = [System.Drawing.Bitmap]::new(17, 17)
    $back = [System.Drawing.Graphics]::FromImage($impulse)
    $back.Clear([System.Drawing.Color]::Black)
    $back.Dispose()
    $impulse.SetPixel(8, 8, [System.Drawing.Color]::Red)
    try {
        Write-Case 'blur' {
            param($canvas)
            $canvas.Clear([System.Drawing.Color]::White)
            $canvas.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::NearestNeighbor
            $layouts = @(
                @(0.75, 2, 0, 0, 2, 5, 5), @(3, 2, 0, 0, 2, 54, 5),
                @(6.5, 2, 0, 0, 2, 5, 54), @(3, 0, 2, -2, 0, 88, 54)
            )
            foreach ($layout in $layouts) {
                $filter = [System.Drawing.Imaging.Effects.BlurEffect]::new($layout[0], $false)
                $map = [System.Drawing.Drawing2D.Matrix]::new($layout[1], $layout[2],
                    $layout[3], $layout[4], $layout[5], $layout[6])
                try {
                    $canvas.DrawImage($impulse, $filter, [System.Drawing.RectangleF]::new(0, 0, 17, 17),
                        $map, [System.Drawing.GraphicsUnit]::Pixel, $null)
                } finally {
                    $map.Dispose()
                    $filter.Dispose()
                }
            }
        }
    } finally {
        $impulse.Dispose()
    }

    $red = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(128, 255, 0, 0))
    $transparent = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::Transparent)
    $pen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(128, 0, 0, 255), 6)
    try {
        Write-Metafile 'child' {
            param($canvas)
            $canvas.FillRectangle($red, 10, 10, 35, 35)
            $canvas.FillRectangle($transparent, 55, 10, 35, 35)
            $canvas.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
            $canvas.FillRectangle($transparent, 10, 55, 35, 35)
        }
        $child = [System.Drawing.Imaging.Metafile]::new((Join-Path $Output 'child.emf'))
        try {
            Write-Case 'copy' {
                param($canvas)
                $canvas.Clear([System.Drawing.Color]::Lime)
                $saved = $canvas.Save()
                $canvas.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
                $canvas.FillEllipse($red, 4, 5, 19, 19)
                $canvas.FillRectangle($transparent, 26, 5, 19, 19)
                $canvas.DrawLine($pen, 52, 14, 95, 14)
                $canvas.DrawImage($child, [System.Drawing.Rectangle]::new(10, 48, 84, 48))
                $canvas.Restore($saved)
                $canvas.FillRectangle($red, 26, 30, 19, 12)
                $saved = $canvas.Save()
                $canvas.SetClip([System.Drawing.Rectangle]::new(4, 30, 19, 12))
                $canvas.Clear([System.Drawing.Color]::Transparent)
                $canvas.Restore($saved)
            }
        } finally {
            $child.Dispose()
        }
    } finally {
        $pen.Dispose()
        $transparent.Dispose()
        $red.Dispose()
    }

    $hdc = $screen.GetHdc()
    try {
        $rop = foreach ($mode in 1..16) {
            $values = foreach ($pixel in [Stage5Gdi]::Rop($hdc, $mode)) {
                ,@(($pixel -band 255), (($pixel -shr 8) -band 255), (($pixel -shr 16) -band 255))
            }
            ,$values
        }
    } finally {
        $screen.ReleaseHdc($hdc)
    }
    ConvertTo-Json -InputObject $rop -Depth 5 -Compress |
        Set-Content -LiteralPath (Join-Path $Output 'rop2-windows.json') -Encoding utf8
} finally {
    $screen.Dispose()
    $reference.Dispose()
}
