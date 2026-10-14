# Captures every visible top-level window of the spike process to PNG files.
#
# Usage:  powershell -NoProfile -File tools/screenshot.ps1 -Prefix board-dark
#
# The process is made DPI aware first: a DPI-unaware process is given virtualized
# coordinates and a rescaled screen, so the capture would be blurry and the sizes
# wrong on a 150 % display. Console windows of the process are skipped.
# ASCII only on purpose: Windows PowerShell 5.1 reads a BOM-less .ps1 as ANSI.

param(
    [string]$Prefix = "shot",
    [string]$Process = "gpui-kit-spike",
    [string]$OutDir = "",
    [double]$Scale = 1.0,
    [int]$Index = -1
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName System.Drawing

if (-not $OutDir) {
    $OutDir = Join-Path $PSScriptRoot "..\..\..\docs\spikes\gpui-kit"
}
if (-not (Test-Path $OutDir)) { New-Item -ItemType Directory -Force -Path $OutDir | Out-Null }
$OutDir = (Resolve-Path $OutDir).Path

Add-Type @"
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;

public class Win32Shots {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }

    public delegate bool EnumProc(IntPtr hWnd, IntPtr lParam);

    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
    [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr lParam);
    [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr hWnd, int command);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
    [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr hWnd, out uint pid);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr hWnd, StringBuilder text, int count);
    [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassNameW(IntPtr hWnd, StringBuilder text, int count);

    public static List<IntPtr> WindowsOf(uint wanted) {
        List<IntPtr> found = new List<IntPtr>();
        EnumWindows(delegate(IntPtr hWnd, IntPtr lParam) {
            uint pid;
            GetWindowThreadProcessId(hWnd, out pid);
            if (pid != wanted || !IsWindowVisible(hWnd)) { return true; }
            if (ClassNameOf(hWnd) == "ConsoleWindowClass") { return true; }
            RECT rect;
            if (GetWindowRect(hWnd, out rect) && rect.Right - rect.Left > 32 && rect.Bottom - rect.Top > 32) {
                found.Add(hWnd);
            }
            return true;
        }, IntPtr.Zero);
        return found;
    }

    public static string TitleOf(IntPtr hWnd) {
        StringBuilder sb = new StringBuilder(512);
        GetWindowTextW(hWnd, sb, sb.Capacity);
        return sb.ToString();
    }

    public static string ClassNameOf(IntPtr hWnd) {
        StringBuilder sb = new StringBuilder(256);
        GetClassNameW(hWnd, sb, sb.Capacity);
        return sb.ToString();
    }
}
"@

[void][Win32Shots]::SetProcessDPIAware()

$proc = Get-Process -Name $Process -ErrorAction SilentlyContinue
if (-not $proc) { throw "process $Process is not running" }
$handles = [Win32Shots]::WindowsOf([uint32]$proc.Id)
if ($handles.Count -eq 0) { throw "no visible window for pid $($proc.Id)" }

$i = 0
foreach ($handle in $handles) {
    $rect = New-Object Win32Shots+RECT
    [void][Win32Shots]::GetWindowRect($handle, [ref]$rect)
    $width = $rect.Right - $rect.Left
    $height = $rect.Bottom - $rect.Top
    $title = [Win32Shots]::TitleOf($handle)
    $class = [Win32Shots]::ClassNameOf($handle)
    $name = "$Prefix-$i.png"
    Write-Output "window $i class=$class size=${width}x${height} at $($rect.Left),$($rect.Top) title='$title' file=$name"
    if ($Index -eq -1 -or $Index -eq $i) {
        # A minimized or covered window would capture the desktop instead of the UI.
        if ([Win32Shots]::IsIconic($handle)) {
            [void][Win32Shots]::ShowWindow($handle, 9)
            Start-Sleep -Milliseconds 600
        }
        [void][Win32Shots]::SetForegroundWindow($handle)
        Start-Sleep -Milliseconds 400
        [void][Win32Shots]::GetWindowRect($handle, [ref]$rect)
        $width = $rect.Right - $rect.Left
        $height = $rect.Bottom - $rect.Top
        $source = New-Object System.Drawing.Bitmap $width, $height
        $graphics = [System.Drawing.Graphics]::FromImage($source)
        $graphics.CopyFromScreen($rect.Left, $rect.Top, 0, 0, $source.Size)
        $graphics.Dispose()
        $target = $source
        if ($Scale -ne 1.0) {
            $scaledW = [int][Math]::Round($width * $Scale)
            $scaledH = [int][Math]::Round($height * $Scale)
            $target = New-Object System.Drawing.Bitmap $scaledW, $scaledH
            $resize = [System.Drawing.Graphics]::FromImage($target)
            $resize.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
            $resize.DrawImage($source, 0, 0, $scaledW, $scaledH)
            $resize.Dispose()
        }
        $path = Join-Path $OutDir $name
        $target.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
        if ($Scale -ne 1.0) { $target.Dispose() }
        $source.Dispose()
        Write-Output "saved $path ($($width * $Scale) x $($height * $Scale))"
    }
    $i++
}
