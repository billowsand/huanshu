<#
.SYNOPSIS
    Native (GPUI) packaging script: release build -> portable exe -> NSIS installer -> dist-package/

.DESCRIPTION
    This packages apps/huanshu-native (the GPUI build). The Tauri build is a different
    artifact and a different script - see scripts/package.ps1.

    Output names:
      - huanshu-native_windows_x64.exe            portable single-file exe, double-click to run
      - HuanshuNative_<version>_x64-setup.exe     NSIS installer (currentUser, silent /S)

    The two installs are deliberately separate so installing the native build can never
    overwrite or uninstall the Tauri build:
      - Tauri build:  %LOCALAPPDATA%\Huanshu        uninstall key ...\Uninstall\Huanshu
      - native build: %LOCALAPPDATA%\HuanshuNative  uninstall key ...\Uninstall\HuanshuNative
    Both share the same application data directory on purpose: the native build is meant
    to take over from the Tauri build, so the data must stay where it is. Nothing here
    touches the data directory, and the NSIS script never touches %LOCALAPPDATA%\Huanshu.

    Runtime dependencies (from `dumpbin /dependents target/release/huanshu-native.exe`):
      * No WebView2 and no bundled browser: the UI is drawn by GPUI on DirectX 11
        (d3d11/dxgi/dcomp/dwmapi) and text goes through DirectWrite, all of which ship
        with Windows 10 and later.
      * Static CRT (since d4be460, mirroring tauri_build): the release exe does NOT
        import VCRUNTIME140.dll, so nothing extra to install - the api-ms-win-crt-*
        Universal CRT api sets it still imports are an OS component on Windows 10+.
        Debug builds keep the dynamic CRT (libcmtd conflicts with prebuilt deps).
      * LM Studio is an external dependency and is not packaged: the generation pipeline
        needs a model configured in the app's settings.

    Hard-won facts (do not rediscover these):
      * Build output lives in <repo>/target (the workspace root), NOT apps/huanshu-native/target.
      * makensis comes from the Tauri toolchain: %LOCALAPPDATA%\tauri\NSIS\makensis.exe (NSIS 3).
        It is only needed for the installer; without it the portable exe is still produced.
      * The generated .nsi is ASCII-only and every non-ASCII value is passed with
        `makensis -D...`: the Windows command line is UTF-16, so makensis receives correct
        Unicode. Measured: `-DPRODUCT_NAME=` for the Chinese product name arrives byte
        for byte (U+5E7B U+8FF0 U+FF08 U+539F U+751F U+7248 U+FF09), and a compiled
        installer writes the same string back out. NSIS itself reads the .nsi as ANSI.
      * This file must stay ASCII-only: Windows PowerShell 5.1 reads a BOM-less .ps1 as
        ANSI and turns non-ASCII text into parse errors.

.PARAMETER DebugProfile
    Build the debug profile (faster compile, larger binary, console window kept) for smoke checks.

.PARAMETER NoInstall
    Skip the installer and emit the portable executable only.

.EXAMPLE
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-native.ps1
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-native.ps1 -NoInstall
#>
[CmdletBinding()]
param(
    [switch]$DebugProfile,
    [switch]$NoInstall
)

$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
Set-Location $repo

function Step($msg) { Write-Host "`n=== $msg" -ForegroundColor Cyan }

$profile   = if ($DebugProfile) { 'debug' } else { 'release' }
$targetDir = Join-Path $repo 'target'                       # workspace root, NOT apps/huanshu-native/target
$manifest  = Join-Path $repo 'apps/huanshu-native/Cargo.toml'
$outDir    = Join-Path $repo 'dist-package'
$workDir   = Join-Path $repo 'target/package-native'
$exeName   = 'huanshu-native.exe'

# Product identity. The install directory and the uninstall key are ASCII and distinct from
# the Tauri build's "Huanshu"; the display name is Chinese and matches the version resource
# that apps/huanshu-native/build.rs embeds into the exe.
$productKey = 'HuanshuNative'
# U+5E7B U+8FF0 U+FF08 U+539F U+751F U+7248 U+FF09 literally means "Huanshu (native version)".
# Written as code points because this file has to stay ASCII-only.
$displayName = -join ([char]0x5E7B, [char]0x8FF0, [char]0xFF08, [char]0x539F, [char]0x751F, [char]0x7248, [char]0xFF09)

# 0. fail fast: cargo cannot overwrite a live image ("failed to remove file ... os error 5").
#    This also catches the installed copy, which has the same image name.
$running = @(Get-Process -Name 'huanshu-native' -ErrorAction SilentlyContinue)
if ($running.Count -gt 0) {
    $holders = ($running | ForEach-Object { "  pid $($_.Id)  $($_.Path)" }) -join "`n"
    throw "huanshu-native.exe is still running - close the app, then re-run this script.`n$holders"
}

# 1. version (the package version is the single source for the file name and the installer)
$raw = Get-Content $manifest -Raw
if ($raw -notmatch '(?m)^version\s*=\s*"([^"]+)"') { throw "no package version in $manifest" }
$version = $Matches[1]

$tauriConf = Join-Path $repo 'src-tauri/tauri.conf.json'
if (Test-Path $tauriConf) {
    $tauriVersion = (Get-Content $tauriConf -Raw | ConvertFrom-Json).version
    if ($tauriVersion -ne $version) {
        Write-Host "note: Tauri build is at $tauriVersion, native build at $version (keep them in sync)" -ForegroundColor Yellow
    }
}

# 2. build
$cargoArgs = @('build', '-p', 'huanshu-native')
if (-not $DebugProfile) { $cargoArgs += '--release' }
Step "cargo $($cargoArgs -join ' ')"
cargo @cargoArgs
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed' }

$exePath = Join-Path $targetDir "$profile/$exeName"
if (-not (Test-Path $exePath)) { throw "executable not found: $exePath" }

# 3. collect artifacts
Step "collect -> $outDir"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
New-Item -ItemType Directory -Force -Path $workDir | Out-Null
Copy-Item $exePath (Join-Path $outDir 'huanshu-native_windows_x64.exe') -Force

$setupName = "${productKey}_${version}_x64-setup.exe"
$setupTmp  = Join-Path $workDir $setupName

if ($NoInstall) {
    Write-Host 'no-install: installer skipped' -ForegroundColor DarkGray
} else {
    # NSIS from the Tauri toolchain first, then a standalone install, then PATH.
    $makensis = @(
        (Join-Path $env:LOCALAPPDATA 'tauri/NSIS/makensis.exe'),
        (Join-Path ${env:ProgramFiles(x86)} 'NSIS/makensis.exe'),
        (Join-Path $env:ProgramFiles 'NSIS/makensis.exe')
    ) | Where-Object { $_ -and (Test-Path $_) } | Select-Object -First 1
    if (-not $makensis) {
        $found = Get-Command makensis.exe -ErrorAction SilentlyContinue
        if ($found) { $makensis = $found.Source }
    }

    if (-not $makensis) {
        Write-Host 'makensis not found (looked in %LOCALAPPDATA%\tauri\NSIS, Program Files\NSIS, PATH).' -ForegroundColor Yellow
        Write-Host 'installer skipped - the portable exe above still runs as-is.' -ForegroundColor Yellow
    } else {
        Step "makensis $( & $makensis /VERSION ) - $([IO.Path]::GetFileName($makensis))"
        $nsiPath = Join-Path $workDir 'huanshu-native.nsi'
        # Single-quoted here-string: NSIS uses ${PRODUCT_NAME} / $INSTDIR itself, so
        # PowerShell must not interpolate anything in here.
        $nsi = @'
; Generated by scripts/package-native.ps1 - do not edit, do not commit.
; ASCII-only on purpose: values that need other scripts arrive through -D. NSIS reads this
; file as ANSI, while -D arguments come in over the UTF-16 Windows command line.
Unicode true
SetCompressor /SOLID lzma
!include "MUI2.nsh"

!define UNINST_KEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${PRODUCT_KEY}"

Name "${PRODUCT_NAME}"
OutFile "${OUT_FILE}"
InstallDir "$LOCALAPPDATA\${PRODUCT_KEY}"
InstallDirRegKey HKCU "${UNINST_KEY}" "InstallLocation"
RequestExecutionLevel user
VIProductVersion "${VERSION}.0"
; One set of keys per compiled language, otherwise makensis warns (9100) that the
; other language's block is incomplete and Explorer shows a half-empty Details tab.
!macro VIKeys LANGID
    VIAddVersionKey /LANG=${LANGID} "ProductName" "${PRODUCT_NAME}"
    VIAddVersionKey /LANG=${LANGID} "FileDescription" "${PRODUCT_NAME}"
    VIAddVersionKey /LANG=${LANGID} "FileVersion" "${VERSION}"
    VIAddVersionKey /LANG=${LANGID} "ProductVersion" "${VERSION}"
    VIAddVersionKey /LANG=${LANGID} "CompanyName" "Huanshu"
    VIAddVersionKey /LANG=${LANGID} "LegalCopyright" "Copyright (C) 2026 Huanshu"
!macroend
!insertmacro VIKeys 2052
!insertmacro VIKeys 1033

!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "SimpChinese"
!insertmacro MUI_LANGUAGE "English"

Section "install"
    SetOutPath "$INSTDIR"
    SetOverwrite on
    File "${EXE_PATH}"
    WriteUninstaller "$INSTDIR\uninstall.exe"

    CreateDirectory "$SMPROGRAMS"
    CreateShortcut "$SMPROGRAMS\${PRODUCT_NAME}.lnk" "$INSTDIR\${EXE_NAME}" "" "$INSTDIR\${EXE_NAME}" 0

    WriteRegStr HKCU "${UNINST_KEY}" "DisplayName" "${PRODUCT_NAME}"
    WriteRegStr HKCU "${UNINST_KEY}" "DisplayVersion" "${VERSION}"
    WriteRegStr HKCU "${UNINST_KEY}" "DisplayIcon" "$INSTDIR\${EXE_NAME}"
    WriteRegStr HKCU "${UNINST_KEY}" "Publisher" "Huanshu"
    WriteRegStr HKCU "${UNINST_KEY}" "InstallLocation" "$INSTDIR"
    WriteRegStr HKCU "${UNINST_KEY}" "UninstallString" '"$INSTDIR\uninstall.exe"'
    WriteRegStr HKCU "${UNINST_KEY}" "QuietUninstallString" '"$INSTDIR\uninstall.exe" /S'
    WriteRegStr HKCU "${UNINST_KEY}" "URLInfoAbout" "https://github.com/billowsand/huanshu"
    WriteRegDWORD HKCU "${UNINST_KEY}" "NoModify" 1
    WriteRegDWORD HKCU "${UNINST_KEY}" "NoRepair" 1
SectionEnd

; A section named "Uninstall" is what the compiler puts into the uninstaller.
Section "Uninstall"
    ; Only our own shortcut and our own key - the Tauri build's Huanshu.lnk and
    ; ...\Uninstall\Huanshu key belong to the other installer and are left alone.
    Delete "$SMPROGRAMS\${PRODUCT_NAME}.lnk"
    Delete "$INSTDIR\${EXE_NAME}"
    Delete "$INSTDIR\uninstall.exe"
    DeleteRegKey HKCU "${UNINST_KEY}"
    RMDir "$INSTDIR"
SectionEnd
'@
        Set-Content -Path $nsiPath -Value $nsi -Encoding ASCII

        $mkArgs = @(
            "-DPRODUCT_NAME=$displayName",
            "-DPRODUCT_KEY=$productKey",
            "-DVERSION=$version",
            "-DEXE_NAME=$exeName",
            "-DEXE_PATH=$exePath",
            "-DOUT_FILE=$setupTmp"
        )
        & $makensis @mkArgs $nsiPath
        if ($LASTEXITCODE -ne 0) { throw "makensis failed with exit code $LASTEXITCODE (script: $nsiPath)" }
        if (-not (Test-Path $setupTmp)) { throw "installer not produced: $setupTmp" }
        Copy-Item $setupTmp (Join-Path $outDir $setupName) -Force
    }
}

Step 'result'
Get-ChildItem $outDir | ForEach-Object {
    '{0,-46} {1,9:N2} MB' -f $_.Name, ($_.Length / 1MB)
}
Write-Host "`nPortable exe runs as-is. Installer is currentUser-scoped (%LOCALAPPDATA%\$productKey); silent install /S, silent uninstall (uninstall.exe /S)." -ForegroundColor DarkGray
Write-Host 'Runtime: no WebView2 needed (GPUI draws with DirectX); static CRT - no VC++ Redistributable needed on Windows 10+.' -ForegroundColor DarkGray
