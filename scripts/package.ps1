<#
.SYNOPSIS
    Huanshu packaging script: frontend build -> Tauri NSIS installer -> portable exe -> dist-package/

.DESCRIPTION
    Output names match CI (.github/workflows/release.yml):
      - Huanshu_<version>_x64-setup.exe      NSIS installer (currentUser, silent /S)
      - auto-slidev-studio_windows_x64.exe   portable single-file exe

    Hard-won facts (do not rediscover these):
      * The Tauri CLI is a devDependency: use `bunx tauri`. There is no cargo-tauri here,
        so `cargo tauri build` fails with "no such command: tauri".
      * <repo>/Cargo.toml is a workspace root, so build output lives in <repo>/target,
        NOT src-tauri/target.
      * Frontend output dir is src-tauri/dist-studio (vite.config.ts build.outDir), which is
        what tauri.conf.json build.frontendDist points at. `tauri build` runs
        beforeBuildCommand (`bun run build`) on its own.
      * bundle.targets lists nsis/appimage/deb; only nsis can be produced on Windows,
        so pass --bundles nsis explicitly.
      * Keep this file ASCII-only. Windows PowerShell 5.1 reads BOM-less .ps1 as ANSI and
        mangles non-ASCII text into parse errors.

.PARAMETER SkipFrontend
    Skip `bun run build` and reuse the existing src-tauri/dist-studio (Rust-only changes).

.PARAMETER DebugProfile
    Build the debug profile (faster compile, larger binary) for smoke checks.

.PARAMETER NoInstall
    Skip the installer, emit the executable only (equivalent to --no-bundle).

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1 -SkipFrontend -NoInstall
#>
[CmdletBinding()]
param(
    [switch]$SkipFrontend,
    [switch]$DebugProfile,
    [switch]$NoInstall
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

function Step($msg) { Write-Host "`n=== $msg" -ForegroundColor Cyan }

$profile = if ($DebugProfile) { 'debug' } else { 'release' }
$targetDir = Join-Path $repo 'target'          # workspace root, NOT src-tauri/target
$confPath  = Join-Path $repo 'src-tauri/tauri.conf.json'
$conf      = Get-Content $confPath -Raw | ConvertFrom-Json
$version   = $conf.version
$product   = $conf.productName
$outDir    = Join-Path $repo 'dist-package'

# 0. fail fast: cargo cannot overwrite a live image ("failed to remove file ... os error 5")
$running = @(Get-Process -Name 'auto-slidev-studio' -ErrorAction SilentlyContinue)
if ($running.Count -gt 0) {
    $holders = ($running | ForEach-Object { "  pid $($_.Id)  $($_.Path)" }) -join "`n"
    throw "auto-slidev-studio.exe is still running - close the app, then re-run this script.`n$holders"
}

# 1. frontend
if (-not $SkipFrontend) {
    Step 'bun install (ensure deps)'
    bun install
    Step 'bun run build  ->  src-tauri/dist-studio'
    bun run build            # = vue-tsc && vite build
    if ($LASTEXITCODE -ne 0) { throw 'frontend build failed (fix vue-tsc errors first)' }
} else {
    Write-Host 'skip frontend build' -ForegroundColor DarkGray
}

# 2. tauri build (bundler runs beforeBuildCommand itself)
$tauriArgs = @('tauri', 'build')
if ($DebugProfile)     { $tauriArgs += '--debug' }
if ($NoInstall) { $tauriArgs += '--no-bundle' } else { $tauriArgs += @('--bundles', 'nsis') }

Step "bunx $($tauriArgs -join ' ')"
bunx @tauriArgs
if ($LASTEXITCODE -ne 0) { throw 'tauri build failed' }

# 3. collect artifacts
Step "collect -> $outDir"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

$binSrc = Join-Path $targetDir "$profile/auto-slidev-studio.exe"
if (-not (Test-Path $binSrc)) { throw "executable not found: $binSrc" }
Copy-Item $binSrc (Join-Path $outDir 'auto-slidev-studio_windows_x64.exe') -Force

$setupSrc = Join-Path $targetDir "$profile/bundle/nsis/${product}_${version}_x64-setup.exe"
if ($NoInstall) {
    Write-Host 'no-bundle: installer skipped' -ForegroundColor DarkGray
} elseif (Test-Path $setupSrc) {
    Copy-Item $setupSrc (Join-Path $outDir "${product}_${version}_x64-setup.exe") -Force
} else {
    throw "installer not found: $setupSrc"
}

Step 'result'
Get-ChildItem $outDir | ForEach-Object {
    '{0,-46} {1,9:N2} MB' -f $_.Name, ($_.Length / 1MB)
}
Write-Host "`nPortable exe runs as-is. Installer is currentUser-scoped (%LOCALAPPDATA%\$product); silent install /S, silent uninstall /S." -ForegroundColor DarkGray
