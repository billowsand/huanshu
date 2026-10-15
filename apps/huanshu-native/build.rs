//! Two Windows build concerns live here.
//!
//! 1. Stack: Windows gives the main thread a 1 MiB stack. Unoptimised builds keep large stack
//!    frames (GPUI element trees are built in deeply nested calls), and the third step's editor
//!    form overflowed it in debug builds. Raise it to 8 MiB for every Windows MSVC build.
//! 2. Resources: the executable carries `icon.ico` and a version block, so Explorer, the taskbar
//!    and the uninstall entry show the app icon and "0.4.0" instead of a generic blank file.
//!    The icon is written with numeric id 1 on purpose - GPUI's Windows backend loads the window
//!    and taskbar icon with `LoadImageW(module, MAKEINTRESOURCE(1), ...)`, so the executable's
//!    icon *is* the window icon; there is no per-window icon option on this platform
//!    (`WindowOptions::icon` is documented X11-only).
//!
//! winresource writes the generated `.rc` as UTF-8 behind `#pragma code_page(65001)`, which MSVC
//! `rc.exe` honours, so the Chinese product name survives into the resource unchanged.

fn main() {
    let target = std::env::var("TARGET").unwrap_or_default();
    if !(target.contains("windows") && target.contains("msvc")) {
        return;
    }

    println!("cargo:rustc-link-arg-bins=/STACK:8388608");

    // Static CRT, mirroring what tauri_build does for auto-slidev-studio.exe:
    // without it the exe imports VCRUNTIME140.dll and the UCRT and needs the
    // VC++ 2015-2022 redistributable on a clean machine. Debug keeps the
    // dynamic CRT (libcmtd conflicts with prebuilt deps that link msvcrt).
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile != "debug" {
        println!("cargo:rustc-link-arg-bins=/DEFAULTLIB:libcmt.lib");
        println!("cargo:rustc-link-arg-bins=/DEFAULTLIB:libvcruntime.lib");
        println!("cargo:rustc-link-arg-bins=/DEFAULTLIB:ucrt.lib");
    }

    // Shared with the Tauri bundle (`bundle.icon`); the ico carries every size Explorer,
    // alt-tab and the taskbar ask for, so no downscaling is done here.
    let icon = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("src-tauri")
        .join("icons")
        .join("icon.ico");
    println!("cargo:rerun-if-changed={}", icon.display());

    let mut resource = winresource::WindowsResource::new();
    resource
        .set_icon(&icon.to_string_lossy())
        .set("ProductName", "幻述（原生版）")
        .set("FileDescription", "幻述（原生版）")
        .set("CompanyName", "Huanshu")
        .set("LegalCopyright", "Copyright © 2026 Huanshu")
        .set("OriginalFilename", "huanshu-native.exe")
        .set(
            "Comments",
            "Native GPUI build: no WebView2, shares the data directory with the Tauri build.",
        );
    resource
        .compile()
        .expect("embed the Windows icon and version resource");
}
