# 依次启动 huanshu-native（隔离目录）、截图、关闭。
# 用法: powershell -NoProfile -File tools/shots.ps1
$ErrorActionPreference = "Stop"

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
$Exe = Join-Path $Root "target\release\huanshu-native.exe"
$Tool = Join-Path $PSScriptRoot "screenshot.ps1"
$OutDir = Join-Path $Root "docs\native"
# `$Home` is read-only in PowerShell; use a different name.
$Sandbox = Join-Path $Root "target\sandbox-home"
$env:HUANSHU_HOME = $Sandbox

# prefix, args...
$Shots = @(
    @("home",        @()),
    @("board-32x9",  @("--open", "3")),
    @("board-48x9",  @("--open", "4")),
    @("board-images",@("--open", "1")),
    @("board-charts-edge", @("--open", "6")),
    @("board-light", @("--open", "5", "--theme", "light-notes")),
    @("present",     @("--open", "2", "--present"))
)

foreach ($shot in $Shots) {
    $prefix = $shot[0]
    $args = @($shot[1]) + @("--quit-after", "900")
    Write-Output "--- $prefix : $($args -join ' ')"
    $proc = Start-Process -FilePath $Exe -ArgumentList $args -PassThru -WindowStyle Normal
    Start-Sleep -Seconds 14
    & powershell -NoProfile -ExecutionPolicy Bypass -File $Tool -Prefix $prefix -OutDir $OutDir | Out-Null
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}
Write-Output "done"
