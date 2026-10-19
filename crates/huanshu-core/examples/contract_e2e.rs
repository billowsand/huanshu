//! Live LM Studio regression, using production generation, persistence and export services.
//! cargo run -p huanshu-core --example contract_e2e -- matrix target/lmstudio-e2e/run
//! cargo run -p huanshu-core --example contract_e2e -- pipeline target/lmstudio-e2e/run
//! Optional: LMSTUDIO_BASE_URL, LMSTUDIO_MODEL, E2E_ONLY (comma-separated case names).
use anyhow::{anyhow, ensure, Context, Result};
use futures_util::{stream, StreamExt};
use huanshu_core::{
    assets::AssetCatalog,
    config::{GenerationConfig, LlmSettings},
    events::{CoreEvent, EventSink},
    generator::{planning, slides},
    icon::IconIndex,
    input::parse_markdown,
    services::{export, generation, projects},
    slide_contract, slide_edit,
    state::{AppState, SharedState},
    types::{AspectRatio, ChartSpec, LayoutPlan, PagePlan, SlideBlueprint, SlideKind},
    validate::validate_blueprints,
};
use serde_json::{json, Value};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::Instant,
};

#[derive(Default)]
struct Sink(Mutex<Vec<Value>>);
impl EventSink for Sink {
    fn emit(&self, event: CoreEvent) {
        let (name, payload) = event.wire();
        if name != "gen:log" {
            println!(
                "EVENT {name} {}",
                payload.get("message").unwrap_or(&Value::Null)
            );
        }
        self.0
            .lock()
            .unwrap()
            .push(json!({"name":name,"payload":payload}));
    }
}
fn write(path: &Path, value: &Value) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}
fn settings() -> LlmSettings {
    let mut settings = LlmSettings::default();
    let base =
        std::env::var("LMSTUDIO_BASE_URL").unwrap_or_else(|_| "http://127.0.0.1:12345".into());
    settings.llm.base_url = base.clone();
    settings.embedding.base_url = base.clone();
    settings.multimodal.base_url = base;
    settings.llm.model =
        std::env::var("LMSTUDIO_MODEL").unwrap_or_else(|_| "qwen/qwen3.5-9b".into());
    // No unrelated image matching model is loaded by the text pipeline test.
    settings.multimodal.model = settings.llm.model.clone();
    if let Ok(key) = std::env::var("E2E_API_KEY") {
        settings.llm.api_key = key.clone();
        settings.embedding.api_key = key.clone();
        settings.multimodal.api_key = key;
    }
    if let Ok(model) = std::env::var("E2E_EMBEDDING_MODEL") {
        settings.embedding.model = model;
    }
    settings.concurrency = std::env::var("E2E_CONCURRENCY")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2);
    settings.repair_rounds = 4;
    settings
}
fn create_state(out: &Path, settings: &LlmSettings) -> SharedState {
    let home = out.join("isolated-home");
    // This standalone process never opens the user's normal data directory.
    unsafe {
        std::env::set_var("HUANSHU_HOME", &home);
    }
    let mut state = AppState::load();
    state.settings = settings.clone();
    state.project_dir = out.to_path_buf();
    Mutex::new(state)
}
fn chart(kind: &str) -> ChartSpec {
    serde_json::from_value(json!({
        "type":kind,"title":"季度交付量","source":"user_input","source_note":"隔离测试数据",
        "data":[{"quarter":"一季度","value":12,"other":8},{"quarter":"二季度","value":18,"other":10},{"quarter":"三季度","value":24,"other":14}],
        "encoding":if kind=="donut" {json!({"label":"quarter","value":"value"})} else {json!({"x":"quarter","y":"value"})},
        "series":if matches!(kind,"grouped_bar"|"stacked_area") {json!([{"key":"value","name":"本年"},{"key":"other","name":"上年"}])} else {json!([])},
        "emphasis":{"highlight":["三季度"]}
    })).unwrap()
}
fn cases() -> Vec<(String, SlideKind, String, Option<ChartSpec>)> {
    use SlideKind::*;
    let mut cases: Vec<_> = [
        (SectionIntro,"章节导览：即将介绍目标、治理和运营三个模块，分别解释各模块关注的问题。"),
        (FeatureGrid,"三项能力并列：数据采集支持批处理，治理执行质量检查，服务提供统一接口。每张卡片请填写 subtitle、body、items、risk、conclusion、footer_tag 和 footer_tone，保持每项简短。"),
        (Spotlight,"图片是数据服务架构。右侧严格使用三个面板：flow 表示采集、治理、服务；matrix 表示质量、安全、效率、成本；steps 表示明确责任、试点验证、推广上线。三个面板分别使用这三种 kind，条目统一放在 items。"),
        (SplitLayers,"架构分为采集层、治理层、服务层。左侧解释数据责任、质量验收、权限审计，右侧展示技术分层。请填写 layers_title 为技术分层。"),
        (SectionList,"三项顺序检查：先统一数据口径，再完成接入验证，最后记录验收结果。每项解释验收标准。"),
        (FocusExample,"关注点是先看成本、再看效率、最后看风险。案例为工单自动分类：先核对标注成本，再测分类耗时，最后检查误分类。请填写 ranking_title、example_title、example_body 及三条 ranking。points 不要编号。"),
        (OutcomeGrid,"三项成果：准备时间从2天降为4小时，漏项率从12%降为3%，一次验收通过率达到95%。每项给出简短 tag 表示已验证、可复用或持续跟踪。"),
        (CenterGrid,"核心观点：统一标准，持续交付。四个支撑点为目标明确、数据可信、流程可追踪、风险可回滚。"),
        (Timeline,"发展历程：2023年完成普查，2024年上线一期，2025年形成基础库，2026年进入常态化运营。"),
        (StepFlow,"三个线性步骤：发现问题并采集证据，完成验证并记录结果，推广上线并持续监控。"),
        (Process,"三个阶段：摸底阶段包含盘点系统和明确责任；试点阶段包含接入数据和核验质量；运营阶段包含监测指标和复盘问题。请填写全页 summary 和每个阶段 subtitle、highlight。"),
        (Compare,"集中式与联邦式两种方案对比：集中式口径统一但改造成本高，联邦式接入快但治理难。按成本、质量、权限三项比较，分别给出结论。mode 使用 versus。"),
        (IssueStack,"三个问题：数据口径不一致导致反复核对；系统接口不统一导致重复接入；责任不明确导致故障恢复慢。每个问题列出两项具体痛点。"),
        (Swot,"优势是统筹明确、已有云平台；劣势是专业人员不足、质量参差；机会是业务需求增长、预算支持；威胁是安全风险、部门协同阻力。每象限给出摘要，再给出综合策略。"),
    ].into_iter().map(|(kind,text)| (kind.as_str().to_string(),kind,text.to_string(),None)).collect();
    cases.push(("spotlight-text".into(),Spotlight,"图片展示服务架构。仅使用一个 text 面板，用 body 解释架构意图和 highlight 表达关键结论，不填写 items。".into(),None));
    for kind in ["bar", "line", "donut", "grouped_bar", "stacked_area"] {
        cases.push((format!("chart-{kind}"),ChartFocus,"对季度交付量给出三条解读，引用原数据，不编造数值。不要输出 chart，系统会附加原始图表。".into(),Some(chart(kind))));
    }
    cases
}

fn verify_and_render(
    state: &SharedState,
    id: i64,
    deck: &[SlideBlueprint],
    out: &Path,
) -> Result<Value> {
    let schema = huanshu_core::editor_schema::editor_schema();
    let icons = IconIndex::load(out)?;
    let assets = AssetCatalog::collect(out, &[]);
    let issues = validate_blueprints(deck, &icons, assets.refs());
    ensure!(issues.is_empty(), "final validation: {issues:?}");
    let values: Vec<Value> = deck
        .iter()
        .map(serde_json::to_value)
        .collect::<std::result::Result<_, _>>()?;
    for slide in &values {
        let errors = slide_edit::semantic_errors(&schema, slide);
        ensure!(
            errors.is_empty(),
            "editor rejects generated JSON: {errors:?}"
        );
        let mut edited = slide.clone();
        slide_edit::set_at_path(
            &mut edited,
            "title",
            json!(format!("回写验证 {}", slide["title"].as_str().unwrap())),
        );
        ensure!(
            slide_edit::filter_slide_json(&schema, &edited) == edited,
            "form/JSON view loses fields"
        );
        let typed: SlideBlueprint = serde_json::from_value(edited.clone())?;
        ensure!(
            serde_json::to_value(typed)? == edited,
            "typed round trip changes fields"
        );
    }
    projects::update_project_blueprints(state, id, serde_json::to_string(&values)?)
        .map_err(|e| anyhow!(e))?;
    let stored = projects::get_project(state, id).map_err(|e| anyhow!(e))?;
    ensure!(
        serde_json::from_str::<Vec<Value>>(&stored.blueprints_json)? == values,
        "storage round trip differs"
    );
    write(&out.join("final-deck.json"), &json!(values))?;
    let mut renders = Vec::new();
    for aspect in [
        AspectRatio::Ratio16x9,
        AspectRatio::Ratio32x9,
        AspectRatio::Ratio48x9,
    ] {
        projects::update_project_aspect_ratio(state, id, aspect).map_err(|e| anyhow!(e))?;
        for theme in slide_typst::SUPPORTED_THEMES {
            let label = format!("{aspect:?}-{theme}");
            println!("RENDER {label}: {} pages", values.len());
            let report = export::export_pdf(
                state,
                Some(id),
                Some(values.clone()),
                out.join(format!("{label}.pdf")).display().to_string(),
                Some(theme.to_string()),
            )
            .map_err(|e| anyhow!(e))?;
            let pixels = export::render_slide_pixels(
                state,
                Some(id),
                values.clone(),
                Some(theme.to_string()),
                640,
                None,
            )
            .map_err(|e| anyhow!(e))?;
            let item = json!({"label":label,"pdf":report,"pixel_pages":pixels.pages.len(),"pixel_overflow":pixels.overflow,"pixel_missing_assets":pixels.missing_assets});
            renders.push(item);
            write(&out.join("render-report.json"), &json!(renders))?;
            ensure!(
                report.pages == values.len() && pixels.pages.len() == values.len(),
                "page count mismatch"
            );
            ensure!(
                report.overflow_slides.is_empty() && pixels.overflow.is_empty(),
                "overflow in {label}: {:?}",
                report.overflow_slides
            );
            ensure!(
                report.missing_assets.is_empty() && pixels.missing_assets.is_empty(),
                "missing image in {label}"
            );
            if aspect == AspectRatio::Ratio16x9 && *theme == "light-notes" {
                let (_, options, _) = export::prepare_export_values(
                    state,
                    Some(id),
                    Some(values.clone()),
                    Some(theme.to_string()),
                )
                .map_err(|e| anyhow!(e))?;
                let pngs = export::renderer().render_png(&values, &options, 960, None)?;
                for page in pngs.pages {
                    std::fs::write(
                        out.join(format!("page-{:02}.png", page.index + 1)),
                        page.png,
                    )?;
                }
            }
        }
    }
    Ok(
        json!({"slides":values.len(),"renders":renders,"storage":"passed","editor_roundtrip":"passed"}),
    )
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().collect();
    let mode = args.get(1).map(String::as_str).unwrap_or("matrix");
    let out = PathBuf::from(
        args.get(2)
            .context("provide an isolated output directory")?,
    );
    std::fs::create_dir_all(&out)?;
    let out = std::fs::canonicalize(out)?;
    ensure!(
        out.starts_with(std::fs::canonicalize(
            std::env::current_dir()?.join("target")
        )?),
        "output must be inside workspace target/"
    );
    let settings = settings();
    unsafe {
        std::env::set_var("HUANSHU_MODEL_AUDIT", out.join("model-audit.jsonl"));
    }
    let state = create_state(&out, &settings);
    let started = Instant::now();
    let mut safe_settings = settings.clone();
    safe_settings.llm.api_key.clear();
    safe_settings.embedding.api_key.clear();
    safe_settings.multimodal.api_key.clear();
    write(
        &out.join("run-config.json"),
        &json!({"mode":mode,"settings":safe_settings}),
    )?;
    if mode == "vision" {
        let config =
            GenerationConfig::from_settings(&settings, out.clone(), AspectRatio::Ratio16x9);
        let image = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../slide-typst/tests/fixtures/assets/arch.png");
        let caption = config
            .multimodal_client()?
            .generate_image_caption(&settings.multimodal.model, &image.display().to_string())
            .await?;
        ensure!(!caption.trim().is_empty(), "empty image caption");
        write(
            &out.join("vision-result.json"),
            &json!({"model":settings.multimodal.model,"caption":caption}),
        )?;
    } else if mode == "retry" {
        let id = 1;
        projects::open_project(&state, id).map_err(|e| anyhow!(e))?;
        let run = generation::get_latest_generation_run(&state, id)
            .map_err(|e| anyhow!(e))?
            .context("no stored run")?;
        let before = generation::get_generation_deck(&state, id).map_err(|e| anyhow!(e))?;
        let index = before
            .iter()
            .position(|slide| slide.kind == SlideKind::ChartFocus)
            .context("no chart page")?;
        let sink = Arc::new(Sink::default());
        generation::retry_generation_page(
            sink.clone(),
            &state,
            run.id,
            index - 2,
            Some(SlideKind::ChartFocus),
        )
        .await
        .map_err(|e| anyhow!(e))?;
        let after = generation::get_generation_deck(&state, id).map_err(|e| anyhow!(e))?;
        ensure!(before.len() == after.len(), "retry changed deck size");
        for i in 0..before.len() {
            if i == index {
                ensure!(
                    serde_json::to_value(&before[i].chart)?
                        == serde_json::to_value(&after[i].chart)?,
                    "retry changed source chart"
                );
            } else {
                ensure!(
                    serde_json::to_value(&before[i])? == serde_json::to_value(&after[i])?,
                    "retry changed an unrelated page"
                );
            }
        }
        write(
            &out.join("retry-events.json"),
            &json!(*sink.0.lock().unwrap()),
        )?;
        let report = verify_and_render(&state, id, &after, &out)?;
        write(&out.join("retry-summary.json"), &report)?;
    } else if mode == "repair" {
        let root = std::env::current_dir()?.join("target/lmstudio-e2e/fixed");
        let mut deck = Vec::new();
        for name in ["process", "chart-line"] {
            deck.push(serde_json::from_slice::<SlideBlueprint>(&std::fs::read(
                root.join(name).join("final.json"),
            )?)?);
        }
        let id = projects::create_project(&state, "反馈修复回归".into(), "# 测试".into())
            .map_err(|e| anyhow!(e))?;
        projects::update_project_blueprints(&state, id, serde_json::to_string(&deck)?)
            .map_err(|e| anyhow!(e))?;
        projects::open_project(&state, id).map_err(|e| anyhow!(e))?;
        projects::update_project_aspect_ratio(&state, id, AspectRatio::Ratio32x9)
            .map_err(|e| anyhow!(e))?;
        for (index,feedback) in ["保留三个阶段及全部内容，为每个阶段填写非空 subtitle；summary 改为先摸底、再试点、后运营。", "保留原始图表，明确说明两个相邻区间各增加6，环比增速从50%降至33.33%，不要说增速加快。"].iter().enumerate() {
            let repaired=generation::repair_slide(&state,index,feedback.to_string()).await.map_err(|e|anyhow!(e))?;
            ensure!(repaired.kind==deck[index].kind && repaired.section==deck[index].section,"repair changed structural identity");
            ensure!(repaired.aspect_ratio==Some(AspectRatio::Ratio32x9),"repair ignored selected project ratio");
            ensure!(serde_json::to_value(&repaired.chart)?==serde_json::to_value(&deck[index].chart)?,"repair changed source data");
            if index==0 { ensure!(repaired.phases.iter().all(|p| p.subtitle.as_ref().is_some_and(|s|!s.is_empty())),"repair omitted requested subtitles"); }
            deck[index]=repaired;
        }
        let report = verify_and_render(&state, id, &deck, &out)?;
        write(&out.join("summary.json"), &report)?;
    } else if mode == "pipeline" {
        let source = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/layout-probe-sample.md"),
        )?;
        let id = projects::create_project(&state, "真实流水线回归".into(), source.clone())
            .map_err(|e| anyhow!(e))?;
        projects::open_project(&state, id).map_err(|e| anyhow!(e))?;
        let sink = Arc::new(Sink::default());
        let result = generation::generate_slides(
            sink.clone(),
            &state,
            source,
            None,
            Some("h3".into()),
            Some(AspectRatio::Ratio16x9),
        )
        .await;
        write(&out.join("events.json"), &json!(*sink.0.lock().unwrap()))?;
        let run = generation::get_latest_generation_run(&state, id)
            .map_err(|e| anyhow!(e))?
            .unwrap();
        let logs = generation::get_generation_logs(&state, run.id).map_err(|e| anyhow!(e))?;
        write(&out.join("generation-logs.json"), &json!(logs))?;
        write(
            &out.join("pipeline-result.json"),
            &json!({"result":result,"elapsed_secs":started.elapsed().as_secs_f64()}),
        )?;
        result.map_err(|e| anyhow!(e))?;
        let deck = generation::get_generation_deck(&state, id).map_err(|e| anyhow!(e))?;
        let report = verify_and_render(&state, id, &deck, &out)?;
        write(&out.join("summary.json"), &report)?;
    } else {
        std::fs::create_dir_all(out.join("figure"))?;
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../slide-typst/tests/fixtures/assets/arch.png"),
            out.join("figure/contract.png"),
        )?;
        let config =
            GenerationConfig::from_settings(&settings, out.clone(), AspectRatio::Ratio16x9);
        let client = config.llm_client()?;
        let embedding = config.embedding_client()?;
        let icons = IconIndex::load(&out)?;
        let assets = AssetCatalog::collect(&out, &[]);
        let mut report = Vec::new();
        let mut deck = Vec::new();
        let mut plans = Vec::new();
        let selected: Vec<_> = cases()
            .into_iter()
            .enumerate()
            .filter(|(_, (name, _, _, _))| {
                !std::env::var("E2E_ONLY").is_ok_and(|only| !only.split(',').any(|s| s == name))
            })
            .collect();
        let results=stream::iter(selected).map(|(i,(name,kind,text,chart))| {
            let (client,embedding,config,icons,assets,out)=(&client,&embedding,&config,&icons,&assets,&out);
            async move {
            let page: PagePlan = serde_json::from_value(
                json!({"page_id":format!("{}",i+1),"section_title":"数据平台建设","page_title":format!("测试 {name}"),"objective":text,"key_points":[text],"takeaway":"以可验证的结果推动改进","source_excerpt":text,"section_number":"01","preferred_assets":if kind==SlideKind::Spotlight {vec!["figure/contract.png"]} else {vec![]},"chart":chart}),
            )?;
            let doc = parse_markdown(&format!("# 数据平台建设回归\n\n## 测试章节\n\n{text}\n"))?;
            let layout = LayoutPlan {
                page_id: page.page_id.clone(),
                kind: kind.clone(),
                title: page.page_title.clone(),
                subtitle: None,
                section_label: None,
                reason: text,
            };
            let case_dir = out.join(&name);
            std::fs::create_dir_all(&case_dir)?;
            println!("CASE {name}: start");
            let before = Instant::now();
            let result = slides::generate_single_page_pipeline(
                &client,
                &embedding,
                &config,
                &doc,
                page.clone(),
                layout,
                i,
                &icons,
                &assets,
                &case_dir,
                Arc::new(|i, stage, _| println!("STAGE {i} {stage:?}")),
            )
            .await;
            Ok::<_,anyhow::Error>((i,name,kind,page,case_dir,before.elapsed().as_secs_f64(),result))
        }}).buffer_unordered(settings.concurrency.max(1)).collect::<Vec<_>>().await;
        let mut results = results.into_iter().collect::<Result<Vec<_>>>()?;
        results.sort_by_key(|r| r.0);
        for (_, name, kind, page, case_dir, seconds, result) in results {
            match result {
                Ok(slide) => {
                    let errors = slide_contract::errors(
                        &huanshu_core::editor_schema::editor_schema(),
                        &serde_json::to_value(&slide)?,
                        false,
                    );
                    let chart_equal = page
                        .chart
                        .as_ref()
                        .map(|expected| {
                            serde_json::to_value(expected).unwrap()
                                == serde_json::to_value(slide.chart.as_ref().unwrap()).unwrap()
                        })
                        .unwrap_or(true);
                    let used_fallback =
                        std::fs::read_dir(&case_dir)?
                            .filter_map(Result::ok)
                            .any(|entry| {
                                entry
                                    .file_name()
                                    .to_string_lossy()
                                    .ends_with("-fallback.json")
                            });
                    let passed =
                        slide.kind == kind && errors.is_empty() && chart_equal && !used_fallback;
                    write(&case_dir.join("final.json"), &serde_json::to_value(&slide)?)?;
                    report.push(json!({"case":name,"passed":passed,"actual_kind":slide.kind,"errors":errors,"chart_preserved":chart_equal,"used_fallback":used_fallback,"seconds":seconds}));
                    deck.push(slide);
                    plans.push(page);
                }
                Err(error) => {
                    println!("CASE {name}: FAILED {error:#}");
                    report.push(json!({"case":name,"passed":false,"error":format!("{error:#}"),"seconds":seconds}));
                }
            }
            write(&out.join("case-report.json"), &json!(report))?;
        }
        let failures = report.iter().filter(|r| r["passed"] != true).count();
        // Include production cover/overview/closing assembly in the matrix deck too.
        let doc = parse_markdown(
            "# 数据平台建设回归\n\n## 目标\n\n覆盖内容字段、图表、图片与多种版式。\n",
        )?;
        let assembled =
            planning::assemble_slides(&client, &config, &doc, &plans, deck, &settings.llm.model)
                .await;
        let id = projects::create_project(&state, "逐版式回归".into(), "# 测试".into())
            .map_err(|e| anyhow!(e))?;
        let rendered = verify_and_render(&state, id, &assembled, &out);
        write(
            &out.join("summary.json"),
            &json!({"cases":report,"failures":failures,"render":rendered.as_ref().map(|r|r.clone()).map_err(|e|format!("{e:#}")),"seconds":started.elapsed().as_secs_f64()}),
        )?;
        rendered?;
        ensure!(
            failures == 0,
            "{failures} generation cases failed; see case-report.json"
        );
    }
    println!(
        "PASS {mode} in {:.1}s; artifacts: {}",
        started.elapsed().as_secs_f64(),
        out.display()
    );
    Ok(())
}
