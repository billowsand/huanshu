use std::env;
use std::fs;
use std::path::PathBuf;

// Embed @iconify-json/carbon (the icon set the Vue templates use) the same way
// src-tauri/build.rs does, so CI can `bun install` first and compile afterwards.
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/slide-typst should live two levels below the repo root");

    let icons_src = project_root.join("node_modules/@iconify-json/carbon/icons.json");
    let icons_dst = out_dir.join("carbon-icons.json");
    println!("cargo:rerun-if-changed={}", icons_src.display());
    println!("cargo:rerun-if-changed=typst");

    if icons_src.is_file() {
        fs::copy(&icons_src, &icons_dst).expect("copy carbon icons.json");
    } else if !icons_dst.is_file() {
        fs::write(&icons_dst, b"{\"icons\":{}}").expect("write icon placeholder");
        println!(
            "cargo:warning=missing {}; icons will render as blanks. Run `bun install` at the repo root.",
            icons_src.display()
        );
    }
}
