//! Embed the vendored Carbon icon set into the binary at compile time.
//!
//! The icons live in `assets/carbon/icons.json` (committed, Apache-2.0) rather than in
//! `node_modules`, so the whole workspace builds with nothing but a Rust toolchain — no
//! `bun install` in front of `cargo build`. `OUT_DIR` keeps the `include_bytes!` in
//! `embedded_icons.rs` working without hard-coding a repo-relative path at compile time.

use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    // crates/huanshu-core -> crates -> repo root
    let project_root = manifest_dir
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/huanshu-core should live two levels below the project root");

    let icons_src = project_root.join("assets/carbon/icons.json");
    let icons_dst = out_dir.join("carbon-icons.json");

    println!("cargo:rerun-if-changed={}", icons_src.display());
    println!(
        "cargo:rerun-if-changed={}",
        project_root.join("assets/carbon/info.json").display()
    );

    // A missing icon set is a broken build, not something to paper over with an empty
    // placeholder: every template icon and every icon-recommendation query would silently
    // render blank. `crates/huanshu-core/tests/icons.rs` asserts the set is populated.
    fs::copy(&icons_src, &icons_dst).unwrap_or_else(|err| {
        panic!(
            "failed to copy {} -> {}: {err}\n\
             The vendored Carbon icon set is missing; restore assets/carbon/icons.json.",
            icons_src.display(),
            icons_dst.display()
        )
    });

    // Report the vendored icon-set version (from info.json, e.g. 11.77.0) so the UI can say
    // which set is baked in. Previously this was scraped out of the npm package.json.
    let info_path = project_root.join("assets/carbon/info.json");
    if let Ok(raw) = fs::read_to_string(&info_path) {
        if let Some(version) = json_string_value(&raw, "version") {
            println!("cargo:rustc-env=EMBEDDED_ICONS_CARBON_VERSION={version}");
        }
    }
}

/// Pull a top-level string field out of a small JSON blob without a parser dependency:
/// finds `"key"`, then the first quoted string after the following colon.
fn json_string_value<'a>(raw: &'a str, key: &str) -> Option<&'a str> {
    let needle = format!("\"{key}\"");
    let after_key = &raw[raw.find(&needle)? + needle.len()..];
    let after_colon = after_key[after_key.find(':')? + 1..].trim_start();
    let stripped = after_colon.strip_prefix('"')?;
    let end = stripped.find('"')?;
    Some(&stripped[..end])
}
