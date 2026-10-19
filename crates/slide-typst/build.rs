use std::env;
use std::fs;
use std::path::PathBuf;

// Embed the vendored Carbon icon set (`assets/carbon/icons.json`) so `icons.rs` can turn
// `/icons/<set>/<name>/<rrggbb>.svg` requests into standalone SVG. The file is committed, so
// the crate builds with a Rust toolchain alone — no `bun install` in front of `cargo build`.
fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/slide-typst should live two levels below the repo root");

    let icons_src = project_root.join("assets/carbon/icons.json");
    let icons_dst = out_dir.join("carbon-icons.json");
    println!("cargo:rerun-if-changed={}", icons_src.display());
    println!("cargo:rerun-if-changed=typst");

    // A missing icon set would render every template icon blank; fail the build instead.
    fs::copy(&icons_src, &icons_dst).unwrap_or_else(|err| {
        panic!(
            "failed to copy {} -> {}: {err}\n\
             The vendored Carbon icon set is missing; restore assets/carbon/icons.json.",
            icons_src.display(),
            icons_dst.display()
        )
    });
}
