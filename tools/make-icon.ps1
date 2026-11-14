# Genera l'icona dell'app: assets\vanallen.ico (16-256 px) e assets\icon-64.png.
#
# Disegnata a mano con System.Drawing: tessera scura arrotondata, Terra al centro,
# due anelli luminosi (le fasce di Van Allen). Nessuna dipendenza esterna.
#
#   powershell -ExecutionPolicy Bypass -File tools\make-icon.ps1

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

$root = Split-Path -Parent $PSScriptRoot
$assets = Join-Path $root 'assets'
New-Item -ItemType Directory -Force -Path $assets | Out-Null

function Color([string]$hex, [int]$alpha = 255) {
    $c = [System.Drawing.ColorTranslator]::FromHtml($hex)
    [System.Drawing.Color]::FromArgb($alpha, $c.R, $c.G, $c.B)
}

function New-IconBitmap([int]$S) {
    $bmp = New-Object System.Drawing.Bitmap($S, $S, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::AntiAlias
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.Clear([System.Drawing.Color]::Transparent)

    # tessera arrotondata
    $m = [single]($S * 0.015)
    $r = [single]($S * 0.21)
    $w0 = [single]($S - 2 * $m)
    $rect = [System.Drawing.RectangleF]::new($m, $m, $w0, $w0)
    $d = $r * 2
    $tile = New-Object System.Drawing.Drawing2D.GraphicsPath
    $tile.AddArc($rect.X, $rect.Y, $d, $d, 180, 90)
    $tile.AddArc($rect.Right - $d, $rect.Y, $d, $d, 270, 90)
    $tile.AddArc($rect.Right - $d, $rect.Bottom - $d, $d, $d, 0, 90)
    $tile.AddArc($rect.X, $rect.Bottom - $d, $d, $d, 90, 90)
    $tile.CloseFigure()
    $g.FillPath((New-Object System.Drawing.SolidBrush((Color '#202020'))), $tile)

    $border = New-Object System.Drawing.Pen((Color '#3D3D3D'), [single]($S * 0.006))
    $g.DrawPath($border, $tile)

    $cx = [single]($S * 0.5)
    $cy = [single]($S * 0.5)

    # alone diffuso degli anelli (disegnato prima, sotto la Terra)
    $hx = [single]($S * 0.350); $hy = [single]($S * 0.125)
    $haloPen = New-Object System.Drawing.Pen((Color '#4CC2FF' 32), [single]($S * 0.075))
    $g.DrawEllipse($haloPen, $cx - $hx, $cy - $hy, $hx * 2, $hy * 2)

    # anello esterno (viola tenue)
    $ringPen = New-Object System.Drawing.Pen((Color '#C586FF' 120), [single]($S * 0.016))
    $rx = [single]($S * 0.415); $ry = [single]($S * 0.158)
    $g.DrawEllipse($ringPen, $cx - $rx, $cy - $ry, $rx * 2, $ry * 2)

    # anello principale (ciano)
    $mainPen = New-Object System.Drawing.Pen((Color '#4CC2FF' 240), [single]($S * 0.046))
    $mainPen.StartCap = 'Round'
    $mainPen.EndCap = 'Round'
    $rx2 = [single]($S * 0.305); $ry2 = [single]($S * 0.108)
    $g.DrawEllipse($mainPen, $cx - $rx2, $cy - $ry2, $rx2 * 2, $ry2 * 2)

    # riflesso interno (teal)
    $innerPen = New-Object System.Drawing.Pen((Color '#A7E4FF' 170), [single]($S * 0.014))
    $ryShift = [single]($S * 0.008)
    $g.DrawEllipse($innerPen, $cx - $rx2, $cy - $ry2 - $ryShift, $rx2 * 2, $ry2 * 2)

    # Terra
    $er = [single]($S * 0.105)
    $earth = [System.Drawing.RectangleF]::new(($cx - $er), ($cy - $er), ($er * 2), ($er * 2))
    $grad = [System.Drawing.Drawing2D.LinearGradientBrush]::new(
        $earth, (Color '#8FD0F5'), (Color '#2A5378'), [System.Drawing.Drawing2D.LinearGradientMode]::Vertical)
    $g.FillEllipse($grad, $earth)
    $limb = New-Object System.Drawing.Pen((Color '#BFE6FA' 200), [single]($S * 0.010))
    $g.DrawEllipse($limb, $earth)

    # alone luminoso attorno alla Terra
    $glow = New-Object System.Drawing.Pen((Color '#9BD9FF' 55), [single]($S * 0.030))
    $g.DrawEllipse($glow, ($cx - $er * 1.35), ($cy - $er * 1.35), ($er * 2.7), ($er * 2.7))

    $g.Dispose()
    return $bmp
}

# disegna grande e riduce: bordi più puliti alle misure piccole
function New-Icon([int]$S) {
    $big = New-IconBitmap ($S * 4)
    $out = New-Object System.Drawing.Bitmap($S, $S, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($out)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.DrawImage($big, ([System.Drawing.Rectangle]::new(0, 0, $S, $S)))
    $g.Dispose()
    $big.Dispose()
    return $out
}

function Get-PngBytes([System.Drawing.Bitmap]$bmp) {
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $bytes = $ms.ToArray()
    $ms.Dispose()
    return , $bytes
}

$sizes = @(16, 24, 32, 48, 64, 128, 256)
$images = @()
foreach ($s in $sizes) {
    $b = New-Icon $s
    $images += , @($s, (Get-PngBytes $b))
    if ($s -eq 64) { $b.Save((Join-Path $assets 'icon-64.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    if ($s -eq 256) { $b.Save((Join-Path $assets 'icon-256.png'), [System.Drawing.Imaging.ImageFormat]::Png) }
    $b.Dispose()
}

$ico = Join-Path $assets 'vanallen.ico'
$fs = [System.IO.File]::Create($ico)
$w = New-Object System.IO.BinaryWriter($fs)

$w.Write([UInt16]0)                 # riservato
$w.Write([UInt16]1)                 # tipo: icona
$w.Write([UInt16]$images.Count)     # numero immagini

$offset = 6 + 16 * $images.Count
foreach ($img in $images) {
    $s = $img[0]
    $bytes = $img[1]
    $w.Write([Byte]$(if ($s -ge 256) { 0 } else { $s }))
    $w.Write([Byte]$(if ($s -ge 256) { 0 } else { $s }))
    $w.Write([Byte]0)               # palette
    $w.Write([Byte]0)               # riservato
    $w.Write([UInt16]1)             # piani
    $w.Write([UInt16]32)            # bit per pixel
    $w.Write([UInt32]$bytes.Length)
    $w.Write([UInt32]$offset)
    $offset += $bytes.Length
}
foreach ($img in $images) { $w.Write([Byte[]]$img[1]) }

$w.Flush()
$w.Dispose()
$fs.Dispose()

"icona: $ico (" + (Get-Item $ico).Length + " byte, " + $images.Count + " misure)"
