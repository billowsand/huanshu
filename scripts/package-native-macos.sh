#!/usr/bin/env bash
# 原生版（apps/huanshu-native）的 macOS 打包：.app → 签名 → DMG + zip，产物汇总到 dist-package/。
#
#   scripts/package-native-macos.sh                 # 本机架构，ad-hoc 签名，出 DMG + zip
#   scripts/package-native-macos.sh --universal     # arm64 + x86_64 合成通用二进制
#   scripts/package-native-macos.sh --no-dmg        # 只出 .app 和 zip（快，冒烟用）
#   scripts/package-native-macos.sh --skip-build    # 复用已编好的 target/…/release/huanshu-native
#
# 签名与公证（可选，分发给别人时需要；不设则 ad-hoc 签名，只适合本机或自己的机器）：
#   HUANSHU_SIGN_IDENTITY="Developer ID Application: 姓名 (TEAMID)"   # 正式签名（开 hardened runtime）
#   HUANSHU_NOTARY_PROFILE="huanshu-notary"   # `xcrun notarytool store-credentials` 存下的钥匙串条目
#
# 产物（<arch> 是 arm64 / x86_64 / universal）：
#   dist-package/幻述.app                               可直接拖进「应用程序」
#   dist-package/HuanshuNative_<version>_macos_<arch>.dmg   安装镜像（含「应用程序」快捷方式）
#   dist-package/huanshu-native_macos_<arch>.zip         免安装压缩包（ditto，保留签名）
#
# 版本号只有一个来源：apps/huanshu-native/Cargo.toml 的 version（与 Windows 打包脚本一致）。

set -euo pipefail

UNIVERSAL=0
MAKE_DMG=1
SKIP_BUILD=0
for arg in "$@"; do
    case "$arg" in
        --universal) UNIVERSAL=1 ;;
        --no-dmg) MAKE_DMG=0 ;;
        --skip-build) SKIP_BUILD=1 ;;
        -h | --help)
            sed -n '2,20p' "$0"
            exit 0
            ;;
        *)
            echo "未知参数：$arg（--help 看用法）" >&2
            exit 2
            ;;
    esac
done

step() { printf '\n\033[36m==> %s\033[0m\n' "$*"; }
die() {
    printf '\033[31m错误：%s\033[0m\n' "$*" >&2
    exit 1
}

[[ "$(uname -s)" == "Darwin" ]] || die "只能在 macOS 上运行（Windows 用 scripts/package-native.ps1）"

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

BIN_NAME="huanshu-native"
APP_DISPLAY="幻述"
BUNDLE_ID="com.auto-slidev.studio.native"
ICNS="$REPO/src-tauri/icons/icon.icns"
MANIFEST="$REPO/apps/huanshu-native/Cargo.toml"
OUT="$REPO/dist-package"

# ── 1. 版本号与前置检查 ─────────────────────────────────────────────────────
VERSION="$(sed -n 's/^version[[:space:]]*=[[:space:]]*"\([^"]*\)".*/\1/p' "$MANIFEST" | head -n 1)"
[[ -n "$VERSION" ]] || die "读不到 $MANIFEST 的 version"
TAURI_VERSION="$(sed -n 's/.*"version":[[:space:]]*"\([^"]*\)".*/\1/p' "$REPO/src-tauri/tauri.conf.json" | head -n 1)"
if [[ -n "$TAURI_VERSION" && "$TAURI_VERSION" != "$VERSION" ]]; then
    echo "注意：Tauri 版是 $TAURI_VERSION，原生版是 $VERSION（发版时保持一致）"
fi
[[ -f "$ICNS" ]] || die "缺图标 $ICNS"
command -v cargo >/dev/null || die "找不到 cargo"
xcode-select -p >/dev/null 2>&1 || die "没装 Xcode Command Line Tools（xcode-select --install）"
if [[ ! -f "$REPO/node_modules/@iconify-json/carbon/icons.json" ]]; then
    die "缺 node_modules/@iconify-json/carbon（先在仓库根目录 bun install，否则图标全是空白）"
fi
# 运行中的副本不影响 cargo 覆盖（macOS 允许），但会让人误以为测的是新包：提示一下。
if pgrep -x "$BIN_NAME" >/dev/null 2>&1; then
    echo "注意：$BIN_NAME 正在运行（pid $(pgrep -x "$BIN_NAME" | tr '\n' ' ')），验收前记得关掉重开"
fi

HOST_ARCH="$(uname -m)" # arm64 / x86_64
if [[ $UNIVERSAL == 1 ]]; then
    ARCH_LABEL="universal"
    TARGETS=(aarch64-apple-darwin x86_64-apple-darwin)
else
    ARCH_LABEL="$HOST_ARCH"
    TARGETS=()
fi
step "幻述（原生版）$VERSION · macOS $ARCH_LABEL"

# ── 2. 编译 ────────────────────────────────────────────────────────────────
BINARY=""
if [[ $UNIVERSAL == 1 ]]; then
    PARTS=()
    for target in "${TARGETS[@]}"; do
        if [[ $SKIP_BUILD == 0 ]]; then
            if command -v rustup >/dev/null && ! rustup target list --installed | grep -qx "$target"; then
                step "rustup target add $target"
                rustup target add "$target"
            fi
            step "cargo build --release -p $BIN_NAME --target $target"
            cargo build --release --locked -p "$BIN_NAME" --target "$target"
        fi
        PARTS+=("$REPO/target/$target/release/$BIN_NAME")
    done
    BINARY="$REPO/target/universal-apple-darwin/release/$BIN_NAME"
    mkdir -p "$(dirname "$BINARY")"
    step "lipo → 通用二进制"
    lipo -create -output "$BINARY" "${PARTS[@]}"
else
    if [[ $SKIP_BUILD == 0 ]]; then
        step "cargo build --release -p $BIN_NAME"
        cargo build --release --locked -p "$BIN_NAME"
    fi
    BINARY="$REPO/target/release/$BIN_NAME"
fi
[[ -x "$BINARY" ]] || die "没有可执行文件 $BINARY"
lipo -info "$BINARY"

# ── 3. 组装 .app ───────────────────────────────────────────────────────────
mkdir -p "$OUT"
APP="$OUT/$APP_DISPLAY.app"
step "组装 $APP"
rm -rf "$APP"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BINARY" "$APP/Contents/MacOS/$BIN_NAME"
cp "$ICNS" "$APP/Contents/Resources/AppIcon.icns"
printf 'APPL????' >"$APP/Contents/PkgInfo"

# CFBundleName 决定菜单栏第一项与「关于」里的名字；DevelopmentRegion 设成中文，
# 系统自动加的菜单项（服务、全屏等）才显示中文。
cat >"$APP/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleDevelopmentRegion</key>
    <string>zh_CN</string>
    <key>CFBundleAllowMixedLocalizations</key>
    <true/>
    <key>CFBundleExecutable</key>
    <string>$BIN_NAME</string>
    <key>CFBundleIdentifier</key>
    <string>$BUNDLE_ID</string>
    <key>CFBundleName</key>
    <string>$APP_DISPLAY</string>
    <key>CFBundleDisplayName</key>
    <string>$APP_DISPLAY</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleInfoDictionaryVersion</key>
    <string>6.0</string>
    <key>CFBundleShortVersionString</key>
    <string>$VERSION</string>
    <key>CFBundleVersion</key>
    <string>$VERSION</string>
    <key>LSMinimumSystemVersion</key>
    <string>11.0</string>
    <key>LSApplicationCategoryType</key>
    <string>public.app-category.productivity</string>
    <key>NSHighResolutionCapable</key>
    <true/>
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
    <key>NSHumanReadableCopyright</key>
    <string>Copyright © 2026 Huanshu</string>
</dict>
</plist>
PLIST
plutil -lint "$APP/Contents/Info.plist" >/dev/null

# ── 4. 签名 ────────────────────────────────────────────────────────────────
# Apple Silicon 不运行没有签名的二进制；改动过的包（换了 Info.plist）必须整体重签。
if [[ -n "${HUANSHU_SIGN_IDENTITY:-}" ]]; then
    step "codesign（$HUANSHU_SIGN_IDENTITY，hardened runtime）"
    codesign --force --options runtime --timestamp --sign "$HUANSHU_SIGN_IDENTITY" "$APP"
else
    step "codesign（ad-hoc：只适合本机；分发请设 HUANSHU_SIGN_IDENTITY）"
    codesign --force --sign - "$APP"
fi
codesign --verify --strict --verbose=2 "$APP"

# ── 5. zip（免安装）────────────────────────────────────────────────────────
ZIP="$OUT/${BIN_NAME}_macos_${ARCH_LABEL}.zip"
step "ditto → $(basename "$ZIP")"
rm -f "$ZIP"
ditto -c -k --sequesterRsrc --keepParent "$APP" "$ZIP"

# ── 6. DMG ─────────────────────────────────────────────────────────────────
if [[ $MAKE_DMG == 1 ]]; then
    DMG="$OUT/HuanshuNative_${VERSION}_macos_${ARCH_LABEL}.dmg"
    step "hdiutil → $(basename "$DMG")"
    STAGE="$(mktemp -d)"
    cp -R "$APP" "$STAGE/"
    ln -s /Applications "$STAGE/应用程序"
    hdiutil create -quiet -volname "$APP_DISPLAY $VERSION" -srcfolder "$STAGE" -ov -format UDZO "$DMG"
    rm -rf "$STAGE"
    if [[ -n "${HUANSHU_SIGN_IDENTITY:-}" ]]; then
        codesign --force --timestamp --sign "$HUANSHU_SIGN_IDENTITY" "$DMG"
    fi
    # 公证：需要正式签名；通过后把票据钉进 DMG，离线也能过 Gatekeeper。
    if [[ -n "${HUANSHU_NOTARY_PROFILE:-}" ]]; then
        [[ -n "${HUANSHU_SIGN_IDENTITY:-}" ]] || die "公证需要正式签名（设 HUANSHU_SIGN_IDENTITY）"
        step "notarytool submit（$HUANSHU_NOTARY_PROFILE）"
        xcrun notarytool submit "$DMG" --keychain-profile "$HUANSHU_NOTARY_PROFILE" --wait
        xcrun stapler staple "$DMG"
    fi
fi

# ── 7. 汇总 ────────────────────────────────────────────────────────────────
step "完成"
ls -lh "$OUT" | grep -E "$APP_DISPLAY.app|HuanshuNative_${VERSION}_macos|${BIN_NAME}_macos" || true
if [[ -z "${HUANSHU_SIGN_IDENTITY:-}" ]]; then
    cat <<'NOTE'

ad-hoc 签名的包在别的 Mac 上首次打开会被 Gatekeeper 拦下：右键 →「打开」，
或「系统设置 → 隐私与安全性」里点「仍要打开」。正式分发请用 Developer ID 签名并公证。
NOTE
fi
