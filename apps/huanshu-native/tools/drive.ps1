# Script a short interaction against huanshu-native (sandbox only) and screenshot along the way.
# Usage:
#   powershell -NoProfile -ExecutionPolicy Bypass -File apps/huanshu-native/tools/drive.ps1 `
#     -Route settings -Steps "click:0.21,0.29;wait:1;shot:a;click:0.075,0.068;wait:2;shot:b;key:^s" -Out target/drive
# Steps (separated by ';'):
#   click:x,y   click at a fraction of the window (0..1 from the top-left corner)
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
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint x, uint y, uint d, IntPtr e);
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
        "key" { [void][Drive]::SetForegroundWindow($handle); $shell.SendKeys($arg) }
        "wait" { Start-Sleep -Milliseconds ([int]([double]$arg * 1000)) }
        "shot" { & powershell -NoProfile -ExecutionPolicy Bypass -File $Tool -Prefix $arg -OutDir $Out | Out-Null }
        default { Write-Output "unknown step $step" }
    }
}
$proc.Refresh()
Write-Output ("alive: " + (-not $proc.HasExited))
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
