# Script a short interaction against huanshu-native (sandbox only) and screenshot along the way.
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File apps/huanshu-native/tools/drive.ps1 `
#     -Route settings -Steps "click:0.21,0.29;wait:1;shot:a;click:0.075,0.068;wait:2;shot:b;key:^s" -Out target/drive
# Steps (separated by ';'):
#   click:x,y   click at a fraction of the window (0..1 from the top-left corner)
#   move:x,y    just move the pointer there (no click); use it to hover an element
#   wheel:n     scroll the wheel n clicks (negative scrolls up) at the current pointer
#   key:k       SendKeys syntax: {ESC}, {RIGHT}, ^s (Ctrl+S), plain text
#   wait:s      sleep s seconds (decimals ok)
#   shot:name   screenshot to <Out>/<name>-0.png
# ASCII only: Windows PowerShell 5.1 misreads BOM-less UTF-8 scripts.
param(
    [string]$Route = "home",
    [string]$Steps = "",
    [string]$Out = "target\drive",
    [string]$Theme = "dark-blue",
    [int]$Boot = 8
)
$ErrorActionPreference = "Stop"
Add-Type @'
using System; using System.Runtime.InteropServices;
public class Drive {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(int f, int x, int y, int d, IntPtr e);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  public struct RECT { public int L, T, R, B; }
}
'@
[void][Drive]::SetProcessDPIAware()

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
$Exe = Join-Path $Root "target\release\huanshu-native.exe"
$Tool = Join-Path $PSScriptRoot "screenshot.ps1"
if (-not [System.IO.Path]::IsPathRooted($Out)) { $Out = Join-Path $Root $Out }
New-Item -ItemType Directory -Force $Out | Out-Null
# Never touch the real data.
$env:HUANSHU_HOME = Join-Path $Root "target\sandbox-home"

$proc = Start-Process -FilePath $Exe -ArgumentList "--route", $Route, "--theme", $Theme, "--quit-after", "300" -PassThru
Start-Sleep -Seconds $Boot
$proc.Refresh()
$handle = $proc.MainWindowHandle
$shell = New-Object -ComObject WScript.Shell

foreach ($step in ($Steps -split ";")) {
    $step = $step.Trim()
    if (-not $step) { continue }
    $kind, $arg = $step -split ":", 2
    switch ($kind) {
        "click" {
            [void][Drive]::SetForegroundWindow($handle)
            $rect = New-Object Drive+RECT
            [void][Drive]::GetWindowRect($handle, [ref]$rect)
            $fx, $fy = $arg -split ","
            $x = [int]($rect.L + ($rect.R - $rect.L) * [double]$fx)
            $y = [int]($rect.T + ($rect.B - $rect.T) * [double]$fy)
            [void][Drive]::SetCursorPos($x, $y)
            Start-Sleep -Milliseconds 150
            [Drive]::mouse_event(2, 0, 0, 0, [IntPtr]::Zero)
            [Drive]::mouse_event(4, 0, 0, 0, [IntPtr]::Zero)
        }
        # Move the pointer without clicking; a mouse move is what a GPUI hover needs.
        "move" {
            [void][Drive]::SetForegroundWindow($handle)
            $rect = New-Object Drive+RECT
            [void][Drive]::GetWindowRect($handle, [ref]$rect)
            $fx, $fy = $arg -split ","
            $x = [int]($rect.L + ($rect.R - $rect.L) * [double]$fx)
            $y = [int]($rect.T + ($rect.B - $rect.T) * [double]$fy)
            # Two steps: the platform only reports a move when the position changes.
            [void][Drive]::SetCursorPos($x + 4, $y + 4)
            Start-Sleep -Milliseconds 120
            [void][Drive]::SetCursorPos($x, $y)
        }
        "key" { [void][Drive]::SetForegroundWindow($handle); $shell.SendKeys($arg) }
        # Like key, but leaves the foreground window alone: the keystrokes go to
        # whichever window is already focused (e.g. a native file dialog opened by the app).
        "keynf" { $shell.SendKeys($arg) }
        # MOUSEEVENTF_WHEEL = 0x0800; the delta is in WHEEL_DELTA (120) units (negative = up).
        "wheel" {
            [void][Drive]::SetForegroundWindow($handle)
            [Drive]::mouse_event(0x0800, 0, 0, [int]$arg * 120, [IntPtr]::Zero)
        }
        "wait" { Start-Sleep -Milliseconds ([int]([double]$arg * 1000)) }
        "shot" { & powershell -NoProfile -ExecutionPolicy Bypass -File $Tool -Prefix $arg -OutDir $Out | Out-Null }
        default { Write-Output "unknown step $step" }
    }
}
$proc.Refresh()
Write-Output ("alive: " + (-not $proc.HasExited))
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
