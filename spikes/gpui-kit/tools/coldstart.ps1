# Measures cold start of the spike (and of any other exe given -Exe) several times.
#
# For each run it reports up to three numbers (all milliseconds):
#   ext = Start-Process -> a visible window of that process is enumerable
#   int = the app's own "[coldstart] first-frame" line, if it prints one
#   paint = -> the window bitmap stops being uniform, i.e. real content is on screen.
#           This is the only number that can be compared across toolkits: Tauri's
#           window appears long before WebView2 has painted anything.
#
# Usage: powershell -NoProfile -File tools/coldstart.ps1 -Runs 5
#        powershell -NoProfile -File tools/coldstart.ps1 -Exe ..\..\..\target\release\auto-slidev-studio.exe
# ASCII only on purpose: Windows PowerShell 5.1 reads a BOM-less .ps1 as ANSI.

param(
    [string]$Exe = "",
    [int]$Runs = 5,
    [int]$MinWidth = 1000,
    [int]$MinHeight = 600,
    [double]$Timeout = 90,
    [double]$ContentSigma = 10.0
)

$ErrorActionPreference = "Stop"

if (-not $Exe) {
    $Exe = Join-Path $PSScriptRoot "..\target\release\gpui-kit-spike.exe"
}
$Exe = (Resolve-Path $Exe).Path
$temp = Join-Path $env:TEMP ("coldstart-" + [Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $temp | Out-Null

Add-Type -AssemblyName System.Drawing

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public class ColdWin {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
    public delegate bool EnumProc(IntPtr h, IntPtr l);
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
    public static IntPtr BigWindow(uint want, int minW, int minH) {
        IntPtr found = IntPtr.Zero;
        EnumWindows(delegate(IntPtr h, IntPtr l) {
            uint pid; GetWindowThreadProcessId(h, out pid);
            if (pid != want || !IsWindowVisible(h)) { return true; }
            RECT r; GetWindowRect(h, out r);
            if (r.Right - r.Left >= minW && r.Bottom - r.Top >= minH) { found = h; return false; }
            return true;
        }, IntPtr.Zero);
        return found;
    }
}
"@

$ext = @()
$int = @()
$paint = @()
for ($run = 1; $run -le $Runs; $run++) {
    $err = Join-Path $temp "run-$run.err"
    $out = Join-Path $temp "run-$run.out"
    $watch = [System.Diagnostics.Stopwatch]::StartNew()
    $proc = Start-Process -FilePath $Exe -ArgumentList "--quit-after", "6" -PassThru -RedirectStandardError $err -RedirectStandardOutput $out
    $visibleAt = -1.0
    $paintedAt = -1.0
    $baseline = -1.0
    $handle = [IntPtr]::Zero
    while ($watch.Elapsed.TotalSeconds -lt $Timeout) {
        if ($handle -eq [IntPtr]::Zero) {
            $handle = [ColdWin]::BigWindow([uint32]$proc.Id, $MinWidth, $MinHeight)
            if ($handle -ne [IntPtr]::Zero) { $visibleAt = $watch.Elapsed.TotalMilliseconds }
        } else {
            $rect = New-Object ColdWin+RECT
            [void][ColdWin]::GetWindowRect($handle, [ref]$rect)
            $w = $rect.Right - $rect.Left
            $h = $rect.Bottom - $rect.Top
            if ($w -gt 0 -and $h -gt 0) {
                $bitmap = New-Object System.Drawing.Bitmap $w, $h
                $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
                $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $bitmap.Size)
                $graphics.Dispose()
                $sum = 0.0; $sumSq = 0.0; $count = 0
                # Only the inner area: the outer ring can hold the drop shadow and
                # whatever is behind the window, which would look like content.
                $x0 = [int]($w * 0.12); $x1 = [int]($w * 0.88)
                $y0 = [int]($h * 0.12); $y1 = [int]($h * 0.88)
                for ($y = $y0; $y -lt $y1; $y += 23) {
                    for ($x = $x0; $x -lt $x1; $x += 23) {
                        $pixel = $bitmap.GetPixel($x, $y)
                        $luma = 0.299 * $pixel.R + 0.587 * $pixel.G + 0.114 * $pixel.B
                        $sum += $luma; $sumSq += $luma * $luma; $count++
                    }
                }
                $bitmap.Dispose()
                if ($count -gt 0) {
                    $mean = $sum / $count
                    $variance = ($sumSq / $count) - ($mean * $mean)
                    if ($baseline -lt 0) { $baseline = $mean }
                    # An unpainted window is transparent: the capture shows whatever is
                    # behind it, so variance alone would report content that is not the
                    # app. The mean has to move away from that first sample as well.
                    $shifted = [Math]::Abs($mean - $baseline) -gt 6.0
                    if ($variance -gt 0 -and [Math]::Sqrt($variance) -gt $ContentSigma -and $shifted) {
                        $paintedAt = $watch.Elapsed.TotalMilliseconds
                        break
                    }
                }
            }
        }
        if ($proc.HasExited) { break }
        Start-Sleep -Milliseconds 25
    }
    $watch.Stop()
    if (-not $proc.HasExited) { Start-Sleep -Milliseconds 200; if (-not $proc.HasExited) { Stop-Process -Id $proc.Id -Force } }
    $line = Select-String -Path $err -Pattern "\[coldstart\] first-frame" -ErrorAction SilentlyContinue | Select-Object -First 1
    $internal = $null
    if ($line) {
        $match = [regex]::Match($line.Line, "([0-9.]+) ms")
        if ($match.Success) { $internal = [double]$match.Groups[1].Value }
    }
    $ext += $visibleAt
    if ($internal) { $int += $internal }
    if ($paintedAt -gt 0) { $paint += $paintedAt }
    Write-Output ("run {0}: ext={1:N1} ms paint={2} int={3}" -f $run, $visibleAt, $(if ($paintedAt -gt 0) { "{0:N1} ms" -f $paintedAt } else { "n/a" }), $(if ($internal) { "{0:N1} ms" -f $internal } else { "n/a" }))
}

function Median([double[]]$values) {
    if ($values.Count -eq 0) { return [double]::NaN }
    $sorted = $values | Sort-Object
    $mid = [int]($sorted.Count / 2)
    if ($sorted.Count % 2 -eq 0) { return ($sorted[$mid - 1] + $sorted[$mid]) / 2 }
    return $sorted[$mid]
}

Write-Output ("{0}: external median {1:N1} ms over {2} runs ({3})" -f (Split-Path $Exe -Leaf), (Median $ext), $ext.Count, (($ext | ForEach-Object { "{0:N0}" -f $_ }) -join ", "))
if ($paint.Count -gt 0) {
    Write-Output ("{0}: first-content median {1:N1} ms over {2} runs ({3})" -f (Split-Path $Exe -Leaf), (Median $paint), $paint.Count, (($paint | ForEach-Object { "{0:N0}" -f $_ }) -join ", "))
}
if ($int.Count -gt 0) {
    Write-Output ("{0}: internal first-frame median {1:N1} ms over {2} runs ({3})" -f (Split-Path $Exe -Leaf), (Median $int), $int.Count, (($int | ForEach-Object { "{0:N0}" -f $_ }) -join ", "))
}
