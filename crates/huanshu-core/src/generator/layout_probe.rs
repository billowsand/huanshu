//! On-demand probe of the layout pipeline against a live LLM (LM Studio).
//!
//! Runs enrich → layout candidates → deck-level assignment for a Markdown file, without
//! content generation or the Tauri UI, prints the assignment table plus stats, and writes
//! the same debug files as a real run. Use it to check real model behaviour and to compare
//! the effect of weight / rule changes on a fixed document.
//!
//! ```text
//! cargo test -p huanshu-core --lib layout_probe -- --ignored --nocapture
//! ```
//!
//! Environment overrides:
//! - `LAYOUT_PROBE_MD`: Markdown file (default `fixtures/layout-probe-sample.md`)
//! - `LAYOUT_PROBE_SETTINGS`: app settings JSON (default `%LOCALAPPDATA%/auto-slidev-studio/settings.json`)
//! - `LAYOUT_PROBE_ASPECT`: `16x9` (default) | `32x9` | `48x9`
//! - `LAYOUT_PROBE_OUT`: debug output directory (default `<temp>/layout-probe`)

use std::path::PathBuf;
use std::sync::Arc;

use crate::assets::AssetCatalog;
use crate::config::{GenerationConfig, LlmSettings};
use crate::generator::planning::{
    assign_layouts, derive_page_plans, enrich_one_page_plan, layout_assignment_report,
    layout_candidates_for_page,
};
use crate::generator::utils::{slide_kind_name, write_debug};
use crate::input::parse_markdown;
use crate::types::AspectRatio;

fn env_path(key: &str, default: impl FnOnce() -> PathBuf) -> PathBuf {
    std::env::var_os(key)
        .map(PathBuf::from)
        .unwrap_or_else(default)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs a running LM Studio; run with --ignored --nocapture"]
async fn layout_probe() {
    let md_path = env_path("LAYOUT_PROBE_MD", || {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/layout-probe-sample.md")
    });
    let settings_path = env_path("LAYOUT_PROBE_SETTINGS", || {
        PathBuf::from(std::env::var("LOCALAPPDATA").unwrap_or_default())
            .join("auto-slidev-studio/settings.json")
    });
    let out = env_path("LAYOUT_PROBE_OUT", || {
        std::env::temp_dir().join("layout-probe")
    });
    let aspect = match std::env::var("LAYOUT_PROBE_ASPECT").as_deref() {
        Ok("32x9") => AspectRatio::Ratio32x9,
        Ok("48x9") => AspectRatio::Ratio48x9,
        _ => AspectRatio::Ratio16x9,
    };
    std::fs::create_dir_all(&out).expect("create probe output dir");

    let settings: LlmSettings = serde_json::from_str(
        &std::fs::read_to_string(&settings_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", settings_path.display())),
    )
    .expect("parse settings JSON");
    let mut config = GenerationConfig::from_settings(&settings, out.clone(), aspect);
    config.debug_dir = out.clone();
    let config = Arc::new(config);
    let client = config.llm_client().expect("LLM client");

    let doc = Arc::new(
        parse_markdown(&std::fs::read_to_string(&md_path).expect("read markdown"))
            .expect("parse markdown"),
    );
    let base_plans = derive_page_plans(&doc, config.granularity);
    println!(
        "\n[layout_probe] {} | model {} | {:?} | {} pages | debug → {}",
        md_path.display(),
        config.llm.model,
        aspect,
        base_plans.len(),
        out.display()
    );

    let started = std::time::Instant::now();
    let sem = Arc::new(tokio::sync::Semaphore::new(config.concurrency.max(1)));
    let assets = Arc::new(AssetCatalog::from_project_files(&out));
    let handles: Vec<_> = base_plans
        .iter()
        .cloned()
        .map(|page| {
            let (client, config, doc, assets, sem, out) = (
                client.clone(),
                config.clone(),
                doc.clone(),
                assets.clone(),
                sem.clone(),
                out.clone(),
            );
            tokio::spawn(async move {
                let _permit = sem.acquire().await.unwrap();
                let enriched =
                    enrich_one_page_plan(&client, &config, &doc, &page, &assets, &out).await;
                let candidates =
                    layout_candidates_for_page(&client, &config, &enriched, &out).await;
                (enriched, candidates)
            })
        })
        .collect();
    let mut plans = Vec::new();
    let mut candidates = Vec::new();
    for handle in handles {
        let (plan, cands) = handle.await.expect("probe task");
        plans.push(plan);
        candidates.push(cands);
    }

    let assigned = assign_layouts(&plans, &candidates, aspect);
    let report = layout_assignment_report(&candidates, &assigned);
    write_debug(
        &out,
        "02-layout-assignment.json",
        &serde_json::to_string_pretty(&report).unwrap(),
    )
    .expect("write report");

    println!(
        "\n{:<4} {:<14} {:<34} {}",
        "page", "assigned", "LLM candidates (score)", "title"
    );
    for ((plan, cands), page) in assigned.iter().zip(&candidates).zip(&plans) {
        let listed = cands
            .iter()
            .map(|c| format!("{}:{}", slide_kind_name(&c.kind), c.score))
            .collect::<Vec<_>>()
            .join(" ");
        let marker = if cands.first().map(|c| &c.kind) == Some(&plan.kind) {
            " "
        } else {
            "*"
        };
        println!(
            "{:<4} {}{:<13} {:<34} {}",
            plan.page_id,
            marker,
            slide_kind_name(&plan.kind),
            listed,
            page.page_title
        );
    }
    println!("(* = traded down from the LLM's top choice for deck variety)");
    println!(
        "\nassigned:       {}",
        serde_json::to_string(&report["stats"]["assigned"]).unwrap()
    );
    println!(
        "llm_top_choice: {}",
        serde_json::to_string(&report["stats"]["llm_top_choice"]).unwrap()
    );
    println!(
        "fallback pages: {} | traded down: {} | elapsed {:.1}s\n",
        report["fallback_pages"],
        report["traded_down_pages"],
        started.elapsed().as_secs_f32()
    );

    assert_eq!(assigned.len(), base_plans.len());
}
