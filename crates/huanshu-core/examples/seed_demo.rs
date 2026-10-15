//! 往一个**隔离目录**里写入演示项目，供原生界面与 Tauri 版真机验证使用。
//!
//! ```text
//! cargo run -p huanshu-core --example seed_demo -- <目录> [--force]
//! ```
//!
//! 目录就是那个 `HUANSHU_HOME`：本进程把它当作沙箱根，绝不碰用户的
//! `%LOCALAPPDATA%/auto-slidev-studio`。为防手滑，目标是系统数据目录、bootstrap 所在目录、
//! 或空串时直接拒绝运行；目录里已有数据库时也要 `--force`。
//!
//! 数据一律经 `huanshu_core::services` 写入，不写 SQL——与真实界面走同一条路径。

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::Value;

use huanshu_core::services::{media, projects, seed};
use huanshu_core::state::{storage_paths, SharedState, StoragePaths};
use huanshu_core::types::AspectRatio;

fn main() -> Result<()> {
    let mut force = false;
    let mut target: Option<PathBuf> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--force" => force = true,
            other if other.starts_with("--") => bail!("unknown flag: {other}"),
            other => {
                if target.is_some() {
                    bail!("only one target directory is accepted");
                }
                target = Some(PathBuf::from(other));
            }
        }
    }
    let Some(home) = target else {
        bail!("usage: seed_demo <directory> [--force]");
    };

    let paths = guard_target(&home, force)?;
    // 素材导入要调多模态模型（会 await），因此整个播种跑在一个 tokio 运行时里。
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(seed_all(paths))
}

/// 拒绝会写到用户真实数据上的目标目录，并在需要时清空旧沙箱。
fn guard_target(home: &Path, force: bool) -> Result<StoragePaths> {
    if home.as_os_str().is_empty() {
        bail!("refusing to seed an empty target directory");
    }
    // 绝对化：用户传的可能是 `target\sandbox-home`，与系统路径比较前必须先解析。
    let absolute = std::fs::canonicalize(home)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_default().join(home));
    let system = storage_paths(None);
    // Compare against the *directory* that holds bootstrap.json: `home` is a directory, and a
    // sandbox rooted there would overwrite (with --force: delete) the real bootstrap file.
    let bootstrap_dir = system
        .bootstrap
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    for (what, candidate) in [
        ("data directory", &system.data_dir),
        ("bootstrap directory", &bootstrap_dir),
    ] {
        if std::fs::canonicalize(candidate).is_ok_and(|real| real == absolute) {
            bail!("refusing to seed the real {what}: {}", candidate.display());
        }
    }

    let paths = storage_paths(Some(home));
    if paths.data_dir.join("projects.db").exists() {
        if !force {
            bail!(
                "{} already holds a database; pass --force to rebuild it",
                paths.data_dir.display()
            );
        }
        std::fs::remove_dir_all(&paths.data_dir)
            .with_context(|| format!("remove {}", paths.data_dir.display()))?;
        std::fs::remove_file(&paths.bootstrap).ok();
    }
    std::fs::create_dir_all(&paths.data_dir)
        .with_context(|| format!("create {}", paths.data_dir.display()))?;
    Ok(paths)
}

/// 仓库根目录：本 crate 在 `crates/huanshu-core`。
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/huanshu-core sits two levels below the repo root")
        .to_path_buf()
}

/// 蓝图的样例来自渲染 fixture——它们本来就是各种版式的代表用例。
fn fixture(relative: &str) -> Result<Vec<Value>> {
    let path = repo_root().join(relative);
    let raw = std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))
}

async fn seed_all(paths: StoragePaths) -> Result<()> {
    // 让本进程自己就落在沙箱里：`AppState::load()` 读 `HUANSHU_HOME`。
    let home = paths
        .bootstrap
        .parent()
        .expect("bootstrap.json always has a parent")
        .to_path_buf();
    unsafe { std::env::set_var("HUANSHU_HOME", &home) };
    let state = std::sync::Mutex::new(huanshu_core::AppState::load());
    println!(
        "data directory: {}",
        state.lock().unwrap().app_settings.data_dir
    );

    let layouts = fixture("fixtures/all-layout-blueprints.json")?;
    let charts = fixture("crates/slide-typst/tests/fixtures/charts.json")?;
    let chart_edges = fixture("crates/slide-typst/tests/fixtures/charts-edge.json")?;
    let stress = fixture("crates/slide-typst/tests/fixtures/stress.json")?;

    // 图片项目要先建出来，素材才有地方放。
    let images_id = projects::create_project(
        &state,
        "图片".to_string(),
        "素材库图片、图集，以及缺图与坏图的占位。".to_string(),
    )
    .map_err(|e| anyhow::anyhow!("图片: {e}"))?;
    let mapping = seed_images(&state, images_id).await?;
    let image_slides = spotlight_with_library_ids(&mapping)?;

    let mut created = vec![
        seed_project(
            &state,
            "全部版式 16:9",
            AspectRatio::Ratio16x9,
            &layouts,
            "每种版式一页，标准比例。",
        )?,
        seed_project(
            &state,
            "全部版式 32:9",
            AspectRatio::Ratio32x9,
            &layouts,
            "同一份蓝图，超宽比例（32:9）。",
        )?,
        seed_project(
            &state,
            "全部版式 48:9",
            AspectRatio::Ratio48x9,
            &layouts,
            "同一份蓝图，全景比例（48:9）。",
        )?,
        seed_project(
            &state,
            "图表",
            AspectRatio::Ratio16x9,
            &charts,
            "五种图表各一页。",
        )?,
        seed_project(
            &state,
            "图表边界",
            AspectRatio::Ratio16x9,
            &chart_edges,
            "图表的边界数据：负值、千分位、30 个类目、单点、全零等。",
        )?,
        seed_project(
            &state,
            "压力",
            AspectRatio::Ratio16x9,
            &stress,
            "刻意塞满的内容，用来观察裁切与自动缩放。",
        )?,
        (
            "图片".to_string(),
            images_id,
            write_deck(
                &state,
                "图片",
                images_id,
                AspectRatio::Ratio16x9,
                &image_slides,
            )?,
        ),
    ];

    // 第二步「AI 生成」的两个演示项目：一次以 error 结束的运行，以及一副 60 页的长牌组。
    let generation_demo = seed::seed_generation_demo(&state)
        .map_err(|e| anyhow::anyhow!("{}: {e}", seed::GENERATION_DEMO_PROJECT))?;
    let long_deck = seed::seed_long_deck(&state, &layouts)
        .map_err(|e| anyhow::anyhow!("{}: {e}", seed::LONG_DECK_PROJECT))?;
    created.push((
        generation_demo.name.clone(),
        generation_demo.id,
        generation_demo.pages,
    ));
    created.push((long_deck.name.clone(), long_deck.id, long_deck.pages));

    // 沙箱直接进首页：否则 `--route` 会被首次向导拦住（`--route wizard` 照样能进向导）。
    {
        let st = state.lock().unwrap();
        let db = st.db.lock().unwrap();
        huanshu_core::db::mark_first_run_done(&db).context("mark first run done")?;
    }

    println!("\n{:<4} {:<16} {}", "id", "name", "slides");
    for (name, id, pages) in created {
        println!("{id:<4} {name:<16} {pages}");
    }
    if let Some(run) = &generation_demo.run {
        println!(
            "\n{}：run #{} 状态 error，日志 {} 条，第 {} 页生成失败（牌组里是兜底蓝图）",
            generation_demo.name,
            run.id,
            run.logs,
            run.failed_page + 1
        );
    }
    Ok(())
}

/// 建一个项目：写入牌组，并把每页的 `aspect_ratio` 与项目比例对齐。
fn seed_project(
    state: &SharedState,
    name: &str,
    ratio: AspectRatio,
    slides: &[Value],
    markdown: &str,
) -> Result<(String, i64, usize)> {
    let id = projects::create_project(state, name.to_string(), markdown.to_string())
        .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
    let pages = write_deck(state, name, id, ratio, slides)?;
    Ok((name.to_string(), id, pages))
}

/// 把牌组写进一个**已存在**的项目——素材要先落到它名下，图片项目因此只有一条记录。
fn write_deck(
    state: &SharedState,
    name: &str,
    id: i64,
    ratio: AspectRatio,
    slides: &[Value],
) -> Result<usize> {
    let slides: Vec<Value> = slides
        .iter()
        .map(|slide| {
            let mut slide = slide.clone();
            slide["aspect_ratio"] = serde_json::to_value(ratio)?;
            Ok(slide)
        })
        .collect::<Result<_, serde_json::Error>>()?;
    projects::update_project_aspect_ratio(state, id, ratio)
        .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
    let json = serde_json::to_string_pretty(&slides)?;
    projects::update_project_blueprints(state, id, json)
        .map_err(|e| anyhow::anyhow!("{name}: {e}"))?;
    Ok(slides.len())
}

/// 把渲染 fixture 里的样例图片导入素材库，返回旧 id 到新 id 的映射。
///
/// 坏图（`broken.png`）照样导入——它本来就该显示占位；fixture 里引用了却没有文件的
/// `media:99` 不在映射里，保持原样，也继续显示占位。
async fn seed_images(state: &SharedState, project_id: i64) -> Result<Vec<(i64, i64)>> {
    let fixture_dir = repo_root().join("crates/slide-typst/tests/fixtures");
    let assets: Value = serde_json::from_str(
        &std::fs::read_to_string(fixture_dir.join("assets.json")).context("read assets.json")?,
    )
    .context("parse assets.json")?;

    let mut entries: Vec<(String, String)> = assets
        .as_object()
        .expect("assets.json is an object")
        .iter()
        .map(|(reference, path)| (reference.clone(), path.as_str().unwrap().to_string()))
        .collect();
    entries.sort();

    let mut mapping = Vec::new();
    for (reference, relative) in entries {
        // 项目目录里的文件（figure/…）由 deck 自己的目录解析，原样保留引用即可。
        let Some(old_id) = reference
            .strip_prefix("media:")
            .and_then(|id| id.parse::<i64>().ok())
        else {
            continue;
        };
        let path = fixture_dir.join(&relative);
        let data = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
        match media::import_media_asset(
            state,
            media::ImportMediaRequest {
                project_id,
                slide_index: None,
                filename: Path::new(&relative)
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string(),
                mime_type: mime_for(&relative),
                data,
                media_type: "image".to_string(),
            },
        )
        .await
        {
            Ok(asset) => mapping.push((old_id, asset.id)),
            Err(error) => eprintln!("skipping {relative}: {error}"),
        }
    }
    Ok(mapping)
}

fn mime_for(path: &str) -> Option<String> {
    match Path::new(path).extension()?.to_str()? {
        "png" => Some("image/png".into()),
        "jpg" | "jpeg" => Some("image/jpeg".into()),
        "webp" => Some("image/webp".into()),
        "bmp" => Some("image/bmp".into()),
        "svg" => Some("image/svg+xml".into()),
        _ => None,
    }
}

/// 把 fixture 里的 `media:<旧 id>` 改写成素材库分配到的 `media:<新 id>`。
fn spotlight_with_library_ids(mapping: &[(i64, i64)]) -> Result<Vec<Value>> {
    let mut slides = fixture("crates/slide-typst/tests/fixtures/spotlight.json")?;
    for slide in &mut slides {
        let raw = serde_json::to_string(slide)?;
        let mut rewritten = raw;
        for (old, new) in mapping {
            rewritten = rewritten.replace(&format!("\"media:{old}\""), &format!("\"media:{new}\""));
        }
        *slide = serde_json::from_str(&rewritten)?;
    }
    Ok(slides)
}
