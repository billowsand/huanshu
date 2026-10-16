//! 临时校验程序：把一个 `.keynn` 包导进隔离沙箱，再读回来渲染一遍。
//!
//! ```text
//! cargo run -p huanshu-core --example keynn_check -- <file.keynn> <manifest.json>
//! ```

use std::path::PathBuf;
use std::sync::Mutex;

use anyhow::{bail, Context, Result};

use huanshu_core::services::storage;
use huanshu_core::state::storage_paths;
use huanshu_core::types::SlideBlueprint;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let package = PathBuf::from(args.next().context("usage: keynn_check <file.keynn> [assets.json]")?);
    let manifest = args.next().map(PathBuf::from);

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap()
        .to_path_buf();
    let home = root.join("target/keynn-check/sandbox");
    if home.exists() {
        std::fs::remove_dir_all(&home)?;
    }
    let paths = storage_paths(Some(&home));
    std::fs::create_dir_all(&paths.data_dir)?;
    unsafe { std::env::set_var("HUANSHU_HOME", &home) };

    let state = Mutex::new(huanshu_core::AppState::load());
    let bytes = std::fs::read(&package)?;
    let id = storage::import_project(&state, bytes, None).map_err(|e| anyhow::anyhow!(e))?;
    let project = huanshu_core::services::projects::get_project(&state, id)
        .map_err(|e| anyhow::anyhow!(e))?;
    println!("imported project #{id}: {}", project.name);
    println!("  md_content: {} 字", project.md_content.chars().count());

    let slides: Vec<SlideBlueprint> = serde_json::from_str(&project.blueprints_json)
        .context("blueprints 反序列化成 SlideBlueprint 失败")?;
    println!("  blueprints: {} 页（serde 校验通过）", slides.len());

    let mut kinds: Vec<String> = Vec::new();
    for slide in &slides {
        let name = serde_json::to_value(&slide.kind)?
            .as_str()
            .unwrap_or_default()
            .to_string();
        if !kinds.contains(&name) {
            kinds.push(name);
        }
    }
    println!("  kinds: {} 种 → {}", kinds.len(), kinds.join(", "));

    let json = serde_json::to_value(&slides)?;
    let deck: Vec<serde_json::Value> = json.as_array().unwrap().clone();
    let options = slide_typst::ExportOptions {
        title: Some(project.name.clone()),
        theme: "light-editorial".into(),
        aspect_ratio: "ratio_16x9".into(),
        assets: match &manifest {
            Some(path) => slide_typst::Assets::from_manifest(path)?,
            None => slide_typst::Assets::default(),
        },
        ..Default::default()
    };
    let renderer = slide_typst::Renderer::with_system_fonts();
    let rendered = renderer.render_pdf(&deck, &options)?;
    println!(
        "  rendered: {} pages, {} KB, overflows={}, missing_assets={}",
        rendered.pages,
        rendered.pdf.len() / 1024,
        rendered.overflows.len(),
        rendered.missing_assets.len()
    );
    for overflow in &rendered.overflows {
        println!("    overflow: {overflow:?}");
    }
    for missing in &rendered.missing_assets {
        println!("    missing: {missing:?}");
    }
    if rendered.pages != deck.len() {
        bail!("页码与页数不一致");
    }
    println!("OK");
    Ok(())
}
