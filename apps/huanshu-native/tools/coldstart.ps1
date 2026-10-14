# 冷启动到首页出现：release 构建跑 N 次，取中位数。
# 用法: powershell -NoProfile -File tools/coldstart.ps1 [-Runs 5]
param([int]$Runs = 5)

$ErrorActionPreference = "Stop"
$Root = (Resolve-Path (Join-Path $PSScriptRoot "..\..\..")).Path
$Exe = Join-Path $Root "target\release\huanshu-native.exe"
$env:HUANSHU_HOME = Join-Path $Root "target\sandbox-home"

$samples = @()
for ($i = 0; $i -lt $Runs; $i++) {
    $proc = Start-Process -FilePath $Exe -ArgumentList @("--quit-after", "900") -PassThru -WindowStyle Normal `
        -RedirectStandardError (Join-Path $env:TEMP "coldstart-$i.log")
    $deadline = (Get-Date).AddSeconds(60)
    $ms = $null
    while ((Get-Date) -lt $deadline) {
        Start-Sleep -Milliseconds 50
        if (Test-Path (Join-Path $env:TEMP "coldstart-$i.log")) {
            $hit = Select-String -Path (Join-Path $env:TEMP "coldstart-$i.log") -Pattern "\[coldstart\] first-frame ([0-9.]+)" | Select-Object -Last 1
            if ($hit) { $ms = [double]$hit.Matches[0].Groups[1].Value; break }
        }
    }
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 400
    if ($null -eq $ms) { Write-Output "run $i : no [coldstart] line"; continue }
    $samples += $ms
    Write-Output ("run {0}: {1:N1} ms" -f $i, $ms)
}
if ($samples.Count -gt 0) {
    $sorted = $samples | Sort-Object
    $mid = [int][Math]::Floor($sorted.Count / 2)
    $median = if ($sorted.Count % 2) { $sorted[$mid] } else { ($sorted[$mid - 1] + $sorted[$mid]) / 2 }
    Write-Output ("median of {0}: {1:N1} ms" -f $sorted.Count, $median)
}
