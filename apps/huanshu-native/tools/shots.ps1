# Start huanshu-native in the sandbox once per route, screenshot it, close it.
# Usage: powershell -NoProfile -ExecutionPolicy Bypass -File apps/huanshu-native/tools/shots.ps1 [-Only home,settings]
# Output: docs/native/shell/<name>-0.png. Compare with docs/native/reference/ (the Tauri + Vue app).
# ASCII only: Windows PowerShell 5.1 misreads BOM-less UTF-8 scripts.
param(
    [string[]]$Only = @(),
    [string]$Theme = "dark-blue",
    [int]$Wait = 9
)
$ErrorActionPreference = "Stop"
# `-File` passes "a,b" as one string; split it ourselves.
$Only = @($Only | ForEach-Object { $_ -split "," } | Where-Object { $_ })

$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
$Exe = Join-Path $Root "target\release\huanshu-native.exe"
$Tool = Join-Path $PSScriptRoot "screenshot.ps1"
$OutDir = Join-Path $Root "docs\native\shell"
New-Item -ItemType Directory -Force $OutDir | Out-Null
# Never touch the real data: everything runs inside the sandbox.
$env:HUANSHU_HOME = Join-Path $Root "target\sandbox-home"

# name, route
$Shots = @(
    @("home", "home"),
    @("wizard", "wizard"),
    @("settings-appearance", "settings"),
    @("settings-models", "settings:models"),
    @("settings-generation", "settings:generation"),
    @("settings-storage", "settings:storage"),
    @("settings-about", "settings:about"),
    @("new-step1", "new"),
    @("project-step1", "project:2:1"),
    @("project-step2", "project:2:2"),
    @("project-step2-error", "project:8:2"),
    @("project-step3", "project:2:3"),
    @("project-step3-32x9", "project:3:3"),
    @("project-step3-images", "project:1:3")
)

foreach ($shot in $Shots) {
    $name = $shot[0]
    if ($Only.Count -gt 0 -and -not ($Only -contains $name)) { continue }
    $args = @("--route", $shot[1], "--theme", $Theme, "--quit-after", "120")
    Write-Output "--- $name : $($args -join ' ')"
    $proc = Start-Process -FilePath $Exe -ArgumentList $args -PassThru -WindowStyle Normal
    Start-Sleep -Seconds $Wait
    & powershell -NoProfile -ExecutionPolicy Bypass -File $Tool -Prefix $name -OutDir $OutDir -ProcessId $proc.Id | Out-Null
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Seconds 2
}

# Screens that need interaction are driven with drive.ps1 instead of --route:
#   name, route, steps, boot
# `present` is only reachable by clicking the third step's "present" button (~0.70, 0.065 of the
# window); the presentation opens fullscreen by itself, so two shots are taken there: pointer off
# the controls bar (the bar must stay hidden) and pointer on it (the bar fades in).
# The leading comma keeps the single tuple from being flattened by `@(...)`.
$Interactive = @(
    ,@("present", "project:2:3",
       "click:0.697,0.065;wait:6;move:0.5,0.4;wait:1.5;shot:present-full;move:0.5,0.97;wait:1.5;shot:present-full-hover",
       12)
)

foreach ($item in $Interactive) {
    $name = $item[0]
    if ($Only.Count -gt 0 -and -not ($Only -contains $name)) { continue }
    Write-Output "--- $name (drive.ps1) : $($item[1]) $($item[2])"
    & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path $PSScriptRoot "drive.ps1") `
        -Route $item[1] -Steps $item[2] -Out $OutDir -Theme $Theme -Boot $item[3]
    # The KindPicker shot is manual, in the same shape:
    #   powershell -NoProfile -ExecutionPolicy Bypass -File apps/huanshu-native/tools/drive.ps1 -Route project:2:3
    #     -Steps "click:0.233,0.965;wait:2;shot:project-step3-kind-picker" -Out docs/native/shell -Boot 10
    # which writes docs/native/shell/project-step3-kind-picker-0.png.
}
Write-Output "done"
