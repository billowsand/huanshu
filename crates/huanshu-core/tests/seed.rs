//! `services::seed` 播种的两个演示项目。
//!
//! 对临时目录跑一遍播种，再从库里读回来断言：没有 LM Studio 时，第二步要看的看板、详情、
//! 失败段与「选择重试布局」对话框全靠这份数据。沙箱是临时目录，真实的 `%LOCALAPPDATA%`
//! 一概不碰。

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use huanshu_core::events::GenerationLogEvent;
use huanshu_core::generation_progress;
use huanshu_core::generator::normalize::{
    apply_component_defaults, make_fallback_slide, normalize_lengths, normalize_tones,
};
use huanshu_core::icon::IconIndex;
use huanshu_core::services::{generation, projects, seed};
use huanshu_core::state::{AppState, SharedState};
use huanshu_core::types::{AspectRatio, LayoutPlan, PagePlan, SlideBlueprint, SlideKind};
use huanshu_core::validate::{validate_blueprints, ValidationIssue};
use serde_json::{json, Value};

/// `HUANSHU_HOME` 是进程级变量，几个测试不能同时改它。
static ENV_LOCK: Mutex<()> = Mutex::new(());

/// 一个临时沙箱：`AppState::load()` 落在这里，测试结束时连同数据目录一起删。
struct Sandbox {
    _lock: MutexGuard<'static, ()>,
    state: Option<SharedState>,
    home: PathBuf,
}

impl Sandbox {
    fn new(label: &str) -> Self {
        let lock = ENV_LOCK.lock().unwrap_or_else(|error| error.into_inner());
        let home =
            std::env::temp_dir().join(format!("huanshu-seed-{label}-{}", uuid::Uuid::new_v4()));
        // SAFETY: 有上面那把锁串行，且只在本进程内改。
        unsafe { std::env::set_var("HUANSHU_HOME", &home) };
        let state = Mutex::new(AppState::load());
        Sandbox {
            _lock: lock,
            state: Some(state),
            home,
        }
    }

    fn state(&self) -> &SharedState {
        self.state.as_ref().expect("沙箱在测试结束前一直有效")
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        // 先关掉 SQLite 连接（`AppState` 的字段在 Drop 体之后才销毁，所以在这里把它交出去），
        // 否则 Windows 上目录里还占着文件，删不掉。
        drop(self.state.take());
        let _ = std::fs::remove_dir_all(&self.home);
    }
}

/// 牌组里的内容页数：用看板自己的算法（`generation_progress::slide_items`）数一遍，
/// 而不是只信 seed 的返回值。
fn board_item_count(deck: &[SlideBlueprint], logs: &[GenerationLogEvent]) -> usize {
    let blueprints: Vec<Option<Value>> = deck
        .iter()
        .map(|slide| serde_json::to_value(slide).ok())
        .collect();
    generation_progress::slide_items(&blueprints, logs, &BTreeMap::new()).len()
}

/// 仓库根目录下的 fixture：`crates/huanshu-core/tests` 往上两级。
fn fixture(relative: &str) -> Vec<Value> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("读 {} 失败：{error}", path.display()));
    serde_json::from_str(&raw)
        .unwrap_or_else(|error| panic!("解析 {} 失败：{error}", path.display()))
}

/// 真实的那份图标索引（内嵌的 carbon 图标）：校验时才不会因为索引为空而误报。
fn icon_index() -> IconIndex {
    IconIndex::load(Path::new(".")).expect("载入内嵌图标索引")
}

/// 只留某一页上的校验问题。
fn issues_on(issues: &[ValidationIssue], slide_index: usize) -> Vec<&str> {
    issues
        .iter()
        .filter(|issue| issue.slide_index == slide_index)
        .map(|issue| issue.message.as_str())
        .collect()
}

#[test]
fn the_failed_run_demo_serves_the_board_detail_and_retry_dialog() {
    let sandbox = Sandbox::new("failed-run");
    let state = sandbox.state();

    let seeded = seed::seed_generation_demo(state).expect("播种「生成记录演示」");
    let run_info = seeded.run.as_ref().expect("这次播种带着一次运行");
    assert_eq!(seeded.name, seed::GENERATION_DEMO_PROJECT);
    assert_eq!(seeded.content_pages, 5);
    // 封面 + 目录 + 五页内容 + 结尾。
    assert_eq!(seeded.pages, 8);

    // 项目在库里。
    let names: Vec<String> = projects::list_projects(state)
        .expect("列项目")
        .into_iter()
        .map(|summary| summary.name)
        .collect();
    assert!(names.contains(&seed::GENERATION_DEMO_PROJECT.to_string()));

    // 运行以 error 结束。
    let run = generation::get_latest_generation_run(state, seeded.id)
        .expect("读最近一次运行")
        .expect("有运行记录");
    assert_eq!(run.id, run_info.id);
    assert_eq!(run.status, "error");
    assert_eq!(run.current_stage.as_deref(), Some("error"));
    assert!(run.finished_at.is_some(), "运行要写成结束状态");
    assert!(run.source_markdown.contains("园区能耗治理方案"));

    // 日志：条数、规划日志、逐页日志、page_error 与失败的 retry。
    let logs = generation::get_generation_logs(state, run.id).expect("读日志");
    assert_eq!(logs.len(), run_info.logs);
    assert_eq!(logs.len(), 18);

    // 组装标记：流水线把牌组写进项目之后记的那条 `assembling` / `assembled`。少了它
    // `reached_assembly` 为假，`get_generation_deck` 读不到牌组、单页重试会被 core 挡下
    // （「整副幻灯片尚未组装完成，请使用整副重试」）。
    assert!(
        logs.iter()
            .any(|log| log.stage == "assembling" && log.kind == "assembled"),
        "演示运行要有组装标记，否则「重试这一页」用不了"
    );
    assert_eq!(
        generation::get_generation_deck(state, seeded.id)
            .expect("读这次运行组装的牌组")
            .len(),
        seeded.pages,
        "组装标记在时，`get_generation_deck` 要读到这次运行的牌组"
    );

    let planning = logs
        .iter()
        .find(|log| log.detail.get("layout_plans").is_some())
        .expect("规划日志里有 layout_plans");
    assert_eq!(planning.stage, "layout_plan");
    assert_eq!(planning.kind, "status");
    assert!(planning.important);
    let plans: Vec<PagePlan> =
        serde_json::from_value(planning.detail["page_plans"].clone()).expect("page_plans 可读");
    let layouts: Vec<LayoutPlan> =
        serde_json::from_value(planning.detail["layout_plans"].clone()).expect("layout_plans 可读");
    assert_eq!(plans.len(), seeded.content_pages);
    assert_eq!(layouts.len(), seeded.content_pages);

    // 页面规划日志也带 page_plans；输入日志带 aspect_ratio（重试对话框从这里取比例）。
    assert!(logs.iter().any(|log| {
        log.stage == "page_plan" && log.kind == "status" && log.detail.get("page_plans").is_some()
    }));
    assert!(logs
        .iter()
        .any(|log| log.kind == "input" && log.detail.get("aspect_ratio").is_some()));

    let slide_logs: Vec<&GenerationLogEvent> =
        logs.iter().filter(|log| log.kind == "slide").collect();
    assert_eq!(slide_logs.len(), seeded.content_pages - 1);
    for log in &slide_logs {
        assert_eq!(log.stage, "content");
        assert!(log.important);
        for key in [
            "page_plan",
            "layout_plan",
            "blueprint",
            "system_prompt",
            "user_prompt",
            "raw_output",
            "parsed_output",
        ] {
            assert!(log.detail.get(key).is_some(), "content 日志缺 {key}");
        }
    }

    let errors: Vec<&GenerationLogEvent> =
        logs.iter().filter(|log| log.kind == "page_error").collect();
    assert_eq!(errors.len(), 1, "失败页只写一条 page_error");
    assert_eq!(errors[0].stage, "content");
    assert_eq!(errors[0].slide_index, Some(run_info.failed_page as i32));
    assert!(errors[0].detail.get("page_plan").is_some());

    let retries: Vec<&GenerationLogEvent> = logs.iter().filter(|log| log.kind == "retry").collect();
    assert_eq!(retries.len(), 1, "失败的单页重试只写一条 retry");
    assert_eq!(retries[0].slide_index, Some(run_info.failed_page as i32));
    assert_eq!(retries[0].detail["retry_succeeded"], json!(false));
    assert_eq!(retries[0].detail["deck_ready"], json!(false));
    assert!(retries[0].detail.get("layout_plan").is_some());

    // 从日志恢复出来的页状态：失败页是 error（看板上显示「生成失败」）。
    let statuses = generation_progress::statuses_from_logs(&logs);
    let failed_status = statuses
        .get(&(run_info.failed_page as i64))
        .expect("失败页有状态");
    assert_eq!(failed_status.stage, "error");
    assert!(failed_status.previous_stage.is_some());

    // 牌组：可反序列化，页数对得上，看板条目数 = 内容页数（封面、目录、结尾不算）。
    let project = projects::get_project(state, seeded.id).expect("读项目");
    let deck: Vec<SlideBlueprint> =
        serde_json::from_str(&project.blueprints_json).expect("牌组可反序列化");
    assert_eq!(deck.len(), seeded.pages);
    assert_eq!(deck[0].kind, SlideKind::Cover);
    assert_eq!(deck.last().unwrap().kind, SlideKind::Closing);
    assert_eq!(board_item_count(&deck, &logs), seeded.content_pages);

    // 失败那一页是兜底蓝图：与流水线 `make_fallback_slide` 加同一套默认值修复的产物一致。
    let failed_slide_index = run_info.failed_page + 2; // 封面与目录在前面
    let mut expected_fallback = make_fallback_slide(
        &plans[run_info.failed_page],
        Some(&layouts[run_info.failed_page]),
        AspectRatio::Ratio16x9,
    );
    apply_component_defaults(&mut expected_fallback);
    normalize_lengths(&mut expected_fallback);
    normalize_tones(&mut expected_fallback);
    assert_eq!(
        serde_json::to_value(&deck[failed_slide_index]).unwrap(),
        serde_json::to_value(&expected_fallback).unwrap(),
        "失败页应当是兜底蓝图"
    );

    // 这次运行以 error 结束的原因就在牌组里：兜底页过不了最终校验，其余页都能过。
    let issues = validate_blueprints(&deck, &icon_index(), &HashSet::new());
    assert!(
        !issues_on(&issues, failed_slide_index).is_empty(),
        "兜底页要有校验问题：{issues:?}"
    );
    assert_eq!(
        issues.len(),
        issues_on(&issues, failed_slide_index).len(),
        "只有兜底页不该通过校验：{issues:?}"
    );

    // 那条失败日志（`generate_slides` 的收尾）要能解释这次失败。
    let error_log = logs
        .iter()
        .find(|log| log.stage == "error")
        .expect("有生成失败日志");
    let message = error_log.detail["error"]
        .as_str()
        .expect("错误信息是字符串");
    assert!(message.contains("最终校验未通过"), "{message}");
    assert!(message.contains("第 3 页"), "{message}");

    // 「选择重试布局」：界面打开项目之后才拿得到可用版式（`get_retry_layouts` 看的是当前项目）。
    projects::open_project(state, seeded.id).expect("打开项目");
    let options =
        generation::get_retry_layouts(state, run.id, run_info.failed_page).expect("读可用版式");
    assert!(
        options.kinds.len() > 1,
        "下拉里要有多个版式：{:?}",
        options.kinds
    );
    assert!(
        options.kinds.contains(&options.current_kind),
        "当前版式要在候选里：{:?} / {:?}",
        options.current_kind,
        options.kinds
    );
    assert_eq!(options.current_kind, layouts[run_info.failed_page].kind);
}

#[test]
fn the_long_deck_has_sixty_distinct_content_pages() {
    let sandbox = Sandbox::new("long-deck");
    let state = sandbox.state();

    // 「全部版式」的牌组就是复制来源（example 里也是读这个 fixture）。
    let base = fixture("fixtures/all-layout-blueprints.json");
    let seeded = seed::seed_long_deck(state, &base).expect("播种「长牌组」");
    assert_eq!(seeded.name, seed::LONG_DECK_PROJECT);
    assert!(
        seeded.content_pages >= 55,
        "内容页只有 {} 页",
        seeded.content_pages
    );

    let names: Vec<String> = projects::list_projects(state)
        .expect("列项目")
        .into_iter()
        .map(|summary| summary.name)
        .collect();
    assert!(names.contains(&seed::LONG_DECK_PROJECT.to_string()));

    let project = projects::get_project(state, seeded.id).expect("读项目");
    let deck: Vec<SlideBlueprint> =
        serde_json::from_str(&project.blueprints_json).expect("牌组可反序列化");
    assert_eq!(deck.len(), seeded.pages);
    assert_eq!(deck[0].kind, SlideKind::Cover);
    assert_eq!(deck[1].kind, SlideKind::Overview);
    assert_eq!(deck.last().unwrap().kind, SlideKind::Closing);

    // 看板条目数（去掉封面、目录、结尾）与 seed 报的内容页数一致，且够量虚拟化。
    assert_eq!(board_item_count(&deck, &[]), seeded.content_pages);
    assert!(board_item_count(&deck, &[]) >= 55);

    // 每页的标题都不重复（不要 60 页同一个标题），章节号也是换着来的。
    let titles: HashSet<&str> = deck.iter().map(|slide| slide.title.as_str()).collect();
    assert_eq!(titles.len(), deck.len(), "标题有重复");
    assert!(deck[2].title.starts_with("01.01"), "{}", deck[2].title);
    assert_eq!(deck[2].section.as_deref(), Some("01"));
    assert_eq!(deck[2 + 15].section.as_deref(), Some("02"));

    // 蓝图全部通过最终校验（与真实生成后同一套规则）。
    let issues = validate_blueprints(&deck, &icon_index(), &HashSet::new());
    assert!(issues.is_empty(), "{issues:?}");
}
