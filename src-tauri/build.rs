use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let project_root = manifest_dir
        .parent()
        .expect("src-tauri should have a parent project root");

    // Embed @iconify-json/carbon icons.json into the binary via OUT_DIR so CI can
    // install frontend deps first, then compile without a hard-coded node_modules path.
    let icons_src = project_root.join("node_modules/@iconify-json/carbon/icons.json");
    let icons_dst = out_dir.join("carbon-icons.json");

    println!("cargo:rerun-if-changed={}", icons_src.display());
    println!(
        "cargo:rerun-if-changed={}",
        project_root.join("package.json").display()
    );

    if icons_src.is_file() {
        fs::copy(&icons_src, &icons_dst).unwrap_or_else(|err| {
            panic!(
                "failed to copy {} -> {}: {err}",
                icons_src.display(),
                icons_dst.display()
            )
        });
    } else if !icons_dst.is_file() {
        // Keep an empty placeholder so compile can still succeed in pure-Rust
        // contexts (e.g. docs tooling). Runtime falls back to filesystem discovery.
        fs::write(&icons_dst, b"{}").unwrap_or_else(|err| {
            panic!("failed to write placeholder {}: {err}", icons_dst.display())
        });
        println!(
            "cargo:warning=missing {}; writing empty placeholder. Run `bun install` at repo root before building for real icon data.",
            icons_src.display()
        );
    }

    // Read version from package.json for @iconify-json/carbon
    let package_json_path = project_root.join("package.json");
    if let Ok(raw) = fs::read_to_string(&package_json_path) {
        // Simple regex-free extraction: find '"@iconify-json/carbon": "<version>"'
        if let Some(start) = raw.find("\"@iconify-json/carbon\"") {
            let after_name = &raw[start..];
            if let Some(colon) = after_name.find(':') {
                let after_colon = &after_name[colon + 1..];
                // Skip whitespace and find opening quote
                let trimmed = after_colon.trim_start();
                if let Some(stripped) = trimmed.strip_prefix('"') {
                    if let Some(end_quote) = stripped.find('"') {
                        let version = &stripped[..end_quote];
                        println!("cargo:rustc-env=EMBEDDED_ICONS_CARBON_VERSION={version}");
                    }
                }
            }
        }
    }

    tauri_build::build()
}
