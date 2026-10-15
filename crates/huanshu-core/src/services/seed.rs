//! 沙箱演示数据：第二步「AI 生成」专用的两个项目。
//!
//! `examples/seed_demo.rs` 把它们写进一个隔离目录（`HUANSHU_HOME`），供原生界面真机验证：
//!
//! - [`seed_generation_demo`]：一次以 `error` 结束的生成运行。规划日志带 `page_plans` +
//!   `layout_plans`，逐页 `content` / `slide` 日志带 `page_plan` / `layout_plan` / `blueprint`，
//!   另有一页 `page_error` 与一次失败的单页 `retry`（`retry_succeeded: false`），失败页在牌组
//!   里用兜底蓝图。没有 LM Studio 也能截图看第二步的看板、详情、失败段与「选择重试布局」对话
//!   框——`generation::get_retry_layouts` 读的就是这些日志。
//! - [`seed_long_deck`]：把「全部版式」的内容页复制到 60 页，用来量看板可见区的要图与内存。
//!
//! 日志的 `stage` / `kind` / `detail` 键照 [`generation`](crate::services::generation) 里写日志
//! 的地方抄，出处注在每一条旁边；演示内容（标题、要点、prompt 与报错文本）全部自己编，不来自
//! 任何真实数据库或调试目录。
//!
//! 有一处刻意的补充：`system_prompt` / `user_prompt` / `raw_output` / `parsed_output` 四个键来自
//! 原版第二步的「查看当前页调试信息」（`Step2Generate.vue` 的 `selectedDebugText` 就地读
//! `contentLog.detail`）。流水线本身把提示词与模型原文写进调试文件（`{prefix}.system.txt` /
//! `.user.txt` / `.raw.txt` / `.parsed.json`），并没有写进日志的 `detail`；演示数据按界面读的
//! 那四个键来写，好让这一段也能在没有模型的情况下验证。

use std::collections::HashSet;
use std::path::PathBuf;

use rusqlite::Connection;
use serde_json::{json, Value};

use crate::db;
use crate::generator::layout_rules::ineligibility;
use crate::generator::normalize::{
    apply_component_defaults, make_fallback_slide, normalize_lengths, normalize_tones,
};
use crate::generator::planning::{
    chapter_number, layout_assignment_report, layout_assignment_summary,
};
use crate::generator::utils::slide_kind_name;
use crate::icon::IconIndex;
use crate::input::parse_markdown;
use crate::state::SharedState;
use crate::types::{AspectRatio, LayoutCandidate, LayoutPlan, PagePlan, SlideBlueprint, SlideKind};
use crate::validate::{validate_blueprints, ValidationIssue};

use super::generation::{describe_deck_issues, invalid_content_pages};
use super::{projects, Result};

/// 「生成记录演示」项目名。
pub const GENERATION_DEMO_PROJECT: &str = "生成记录演示";
/// 「长牌组」项目名。
pub const LONG_DECK_PROJECT: &str = "长牌组";
/// 两个演示项目都用 16:9：它们要看的是看板与日志，不是比例适配。
const DEMO_ASPECT: AspectRatio = AspectRatio::Ratio16x9;
/// 「长牌组」的目标内容页数（不含封面、目录、结尾）：看板虚拟化要 60 页上下才量得出来。
const LONG_DECK_CONTENT_PAGES: usize = 60;
/// 「生成记录演示」里失败的内容页下标（0 基，即看板第 3 张）。
const FAILED_PAGE: usize = 2;

/// 一次播种的结果：够 example 打印一行表格、集成测试做断言。
#[derive(Debug, Clone)]
pub struct SeededProject {
    pub id: i64,
    pub name: String,
    /// 牌组页数，含封面、目录与结尾。
    pub pages: usize,
    /// 内容页数（不含封面、目录与结尾）。
    pub content_pages: usize,
    /// 「生成记录演示」写下的那次运行；「长牌组」没有运行记录。
    pub run: Option<SeededRun>,
}

/// 「生成记录演示」写下的那次以 `error` 结束的运行。
#[derive(Debug, Clone)]
pub struct SeededRun {
    pub id: i64,
    pub logs: usize,
    /// 失败的内容页下标（0 基）。
    pub failed_page: usize,
}

// ---------------------------------------------------------------------------
// 生成记录演示
// ---------------------------------------------------------------------------

/// 演示文稿：三章，对应五页内容页。内容自己编的，只有结构照真实文稿的样子写。
const DEMO_MARKDOWN: &str = "\
# 园区能耗治理方案

本文演示一次以失败结束的生成：没有 LM Studio 也能查看第二步的看板、详情与重试。

## 总体架构

平台把采集、计算与展示三层解耦：采集层只负责取数，计算层只负责口径，展示层只负责呈现。

- 采集层按点位上报，不解释业务含义
- 计算层统一口径，输出可复算的指标
- 展示层只读指标，不参与计算
- 接口契约稳定后各层可以独立演进

## 问题与取舍

平台上线后发现三类问题：口径不一致、离线设备造成的数据缺口，以及自建与采购之间的取舍。

- 同一指标出现三个版本，考核数据对不上
- 离线时段的补数策略一直没定下来
- 自建与采购的长期维护成本差别很大
- 数据缺口要能一眼看出是哪一段

## 落地路线

从试点到推广分四步走：对齐口径、补齐采集、灰度推广、常态运营。

- 先把指标定义固化下来
- 离线设备补装网关
- 先在两栋楼试运行
- 把日报交给业务方
";

/// 失败那一页的模型报错：形如 `generator::utils::parse_json_with_extraction` 的解析失败
/// （真实运行里这一条会被 `format!("{e:#}")` 写进 `page_error` 的 summary 与 `detail.error`）。
const FAILED_PAGE_ERROR: &str = "model JSON parse failed (no valid JSON object found):\n\
                                 ~~~ raw preview ~~~\n\
                                 {\"slide\": {\"kind\": \"step_flow\", \"title\": \"从发现到闭环的三个阶段\"\n\
                                 ~~~ sanitized ~~~\n\
                                 {\"slide\": {\"kind\": \"step_flow\", \"title\": \"从发现到闭环的三个阶段\"\n\
                                 ~~~ serde error ~~~\n\
                                 EOF while parsing an object at line 1 column 65";

/// 单页重试失败的原因：逐字用流水线里「生成结果与所选布局不一致」时的那句
/// （`generation::retry_generation_page`）。
const RETRY_PAGE_ERROR: &str = "所选布局生成未成功，请选择其他布局后重试";

/// 内容生成的系统提示词。真实流水线把它写进调试文件（`{prefix}.system.txt`），界面的
/// 「查看当前页调试信息」从日志 `detail` 的同名键读；这里是给演示写的一版短提示词。
const DEMO_SYSTEM_PROMPT: &str = "You generate a single Slidev slide blueprint as strict JSON. Follow the chosen component kind exactly. Keep text compact and presentation-ready. Use the same language as the source page.";

/// 播种「生成记录演示」：一个项目、一次以 `error` 结束的运行、一副失败页用兜底蓝图的牌组。
pub fn seed_generation_demo(state: &SharedState) -> Result<SeededProject> {
    let pages = demo_page_plans()?;
    let layouts = demo_layout_plans(&pages)?;
    let deck = generation_demo_deck(&pages, &layouts)?;

    // 这次运行之所以以 error 结束，就是因为兜底页过不了最终校验。校验一旦通过，说明布局
    // 规则或兜底规则变了，这条演示数据要重做——直接失败，别悄悄播一个对不上的故事。
    // 图标索引用真实的那一份（内嵌的 carbon 图标），这样修好的页不会因为索引为空而报错。
    let issues = validate_blueprints(&deck, &icon_index(state)?, &HashSet::new());
    if !invalid_content_pages(&issues, deck.len()).contains_key(&FAILED_PAGE) {
        return Err(format!(
            "{GENERATION_DEMO_PROJECT}：兜底页没有出现在校验问题里，演示数据不再成立"
        ));
    }

    let project_id = projects::create_project(
        state,
        GENERATION_DEMO_PROJECT.to_string(),
        DEMO_MARKDOWN.to_string(),
    )?;
    write_deck(state, project_id, &deck)?;
    let run = write_generation_run(state, project_id, &pages, &layouts, &deck, &issues)?;

    Ok(SeededProject {
        id: project_id,
        name: GENERATION_DEMO_PROJECT.to_string(),
        pages: deck.len(),
        content_pages: pages.len(),
        run: Some(run),
    })
}

/// 五页内容页的规划：字段与取值照 `planning::derive_page_plans` 的产出写。
///
/// 第 3 页（`FAILED_PAGE`）的信号刻意是「时间线」：它决定了这一页的兜底布局是 `timeline`，
/// 而 `make_fallback_slide` 对 `timeline` 只填标题、不填事件，于是最终校验必然不过。
fn demo_page_plans() -> Result<Vec<PagePlan>> {
    let plans = json!([
        {
            "page_id": "1",
            "section_title": "总体架构",
            "page_title": "三层解耦的总体架构",
            "objective": "阐述「三层解耦的总体架构」的核心内容",
            "key_points": [
                "采集层只负责取数",
                "计算层统一指标口径",
                "展示层只读指标",
                "接口契约稳定后各层可以独立演进"
            ],
            "content_shape": "summary",
            "visual_need": "text_only",
            "object_count": "multi",
            "argument_mode": "parallel",
            "density": "medium",
            "source_excerpt": "平台把采集、计算与展示三层解耦：采集层只负责取数，计算层只负责口径，展示层只负责呈现。",
            "page_role": "content",
            "section_number": "01"
        },
        {
            "page_id": "2",
            "section_title": "总体架构",
            "page_title": "分层职责与接口边界",
            "objective": "阐述「分层职责与接口边界」的核心内容",
            "key_points": ["谁负责取数", "谁负责口径", "谁负责呈现", "谁负责回滚"],
            "content_shape": "workflow",
            "visual_need": "text_only",
            "object_count": "multi",
            "argument_mode": "sequential",
            "density": "medium",
            "source_excerpt": "接口契约稳定后各层可以独立演进，回滚也走同一条链路。",
            "page_role": "content",
            "section_number": "01"
        },
        {
            "page_id": "3",
            "section_title": "问题与取舍",
            "page_title": "从发现到闭环的三个阶段",
            "objective": "阐述「从发现到闭环的三个阶段」的核心内容",
            "key_points": [
                "发现问题：对不上账的指标先记下来",
                "定位原因：区分口径问题与数据缺口",
                "闭环处理：补数、改口径或改采集",
                "回归验证：同一指标在三个视图里一致"
            ],
            "content_shape": "timeline",
            "visual_need": "text_only",
            "object_count": "multi",
            "argument_mode": "sequential",
            "density": "medium",
            "source_excerpt": "平台上线后发现三类问题：口径不一致、离线设备造成的数据缺口，以及自建与采购之间的取舍。",
            "page_role": "content",
            "section_number": "02"
        },
        {
            "page_id": "4",
            "section_title": "问题与取舍",
            "page_title": "自建与采购的取舍",
            "objective": "阐述「自建与采购的取舍」的核心内容",
            "key_points": ["自建：口径可控，但要自己养人", "采购：上线快，改造受厂商节奏"],
            "content_shape": "comparison",
            "visual_need": "text_only",
            "object_count": "pair",
            "argument_mode": "parallel",
            "density": "medium",
            "source_excerpt": "自建与采购的长期维护成本差别很大，先算清三到五年的账再决定。",
            "page_role": "content",
            "section_number": "02"
        },
        {
            "page_id": "5",
            "section_title": "落地路线",
            "page_title": "从试点到推广的四个节点",
            "objective": "阐述「从试点到推广的四个节点」的核心内容",
            "key_points": ["对齐口径", "补齐采集", "灰度推广", "常态运营"],
            "content_shape": "timeline",
            "visual_need": "text_only",
            "object_count": "multi",
            "argument_mode": "sequential",
            "density": "medium",
            "source_excerpt": "从试点到推广分四步走：对齐口径、补齐采集、灰度推广、常态运营。",
            "page_role": "content",
            "section_number": "03"
        }
    ]);
    serde_json::from_value(plans).map_err(|error| format!("演示页面规划不合法：{error}"))
}

/// 五页的布局方案。真实运行里这一步是「模型提候选 + 整副统一分配」，这里直接写死结果，
/// 但逐页对着 `layout_rules` 的资格表验一遍——不满足就直接播种失败。
fn demo_layout_plans(pages: &[PagePlan]) -> Result<Vec<LayoutPlan>> {
    let kinds = [
        SlideKind::FeatureGrid,
        SlideKind::SectionList,
        // 这一页的布局是模型选的步骤流；内容生成失败后牌组换成了兜底的时间轴
        // （见 `generation_demo_deck`），详情里的布局因此会显示「· 已调整布局」。
        SlideKind::StepFlow,
        SlideKind::Compare,
        SlideKind::Timeline,
    ];
    let reasons = [
        "三层职责等权并列，卡片式便于逐层对照",
        "有明确先后顺序，编号列表最清楚",
        "三个阶段按先后推进，适合步骤流",
        "两种路线左右对照",
        "按时间顺序排列的四个节点",
    ];
    let mut plans = Vec::with_capacity(pages.len());
    for (index, page) in pages.iter().enumerate() {
        let kind = kinds[index].clone();
        if let Some(reason) = ineligibility(&kind, page, DEMO_ASPECT) {
            return Err(format!(
                "演示布局不成立：第 {} 页不能用 {}：{reason}",
                index + 1,
                slide_kind_name(&kind)
            ));
        }
        plans.push(LayoutPlan {
            page_id: page.page_id.clone(),
            kind,
            title: page.page_title.clone(),
            subtitle: None,
            section_label: page.section_number.clone(),
            reason: reasons[index].to_string(),
        });
    }
    Ok(plans)
}

/// 组装的牌组：封面 + 目录 + 五页内容 + 结尾，与 `planning::assemble_slides` 一样的顺序。
fn generation_demo_deck(pages: &[PagePlan], layouts: &[LayoutPlan]) -> Result<Vec<SlideBlueprint>> {
    let mut deck = Vec::with_capacity(pages.len() + 3);
    deck.push(frame_slide(demo_cover())?);
    deck.push(frame_slide(demo_overview())?);
    for (index, page) in pages.iter().enumerate() {
        let mut slide = if index == FAILED_PAGE {
            // 照流水线 `make_fallback_slide` 造这一页：布局方案不要了（就是它失败的），
            // 只按页面规划拼一版简化内容。
            make_fallback_slide(page, Some(&layouts[index]), DEMO_ASPECT)
        } else {
            frame_slide(demo_content_slide(index)?)?
        };
        // 流水线组装前会对每一页再做一遍默认值修复（`generation.rs` 里 content_slides 那段），
        // 兜底页尤其需要。
        apply_component_defaults(&mut slide);
        normalize_lengths(&mut slide);
        normalize_tones(&mut slide);
        deck.push(slide);
    }
    deck.push(frame_slide(demo_closing())?);
    Ok(deck)
}

/// 失败页以外四页的蓝图。内容自己编的；不带图标与图片，因此校验只需要一个空的图标索引。
fn demo_content_slide(index: usize) -> Result<Value> {
    let slide = match index {
        0 => json!({
            "kind": "feature_grid",
            "aspect_ratio": DEMO_ASPECT,
            "section": "01",
            "title": "三层解耦的总体架构",
            "subtitle": "采集、计算、展示各管一段",
            "note": "分层的价值在于每一层都能单独替换。",
            "cards": [
                {
                    "title": "采集层",
                    "tone": "blue",
                    "body": "按点位上报原始读数，不解释业务含义。",
                    "items": ["点位心跳", "原始读数", "离线标记"]
                },
                {
                    "title": "计算层",
                    "tone": "green",
                    "body": "统一指标口径，输出可复算的结果。",
                    "items": ["口径唯一", "结果可复算", "重算幂等"]
                },
                {
                    "title": "展示层",
                    "tone": "amber",
                    "body": "只读指标，不参与任何计算。",
                    "items": ["只读查询", "不改口径", "按需缓存"]
                },
                {
                    "title": "接口契约",
                    "tone": "blue",
                    "body": "契约稳定后各层可以独立演进。",
                    "items": ["版本化接口", "字段不可删", "变更先评审"]
                }
            ]
        }),
        1 => json!({
            "kind": "section_list",
            "aspect_ratio": DEMO_ASPECT,
            "section": "01",
            "title": "分层职责与接口边界",
            "subtitle": "四个边界问题先定下来",
            "list_items": [
                {
                    "step": "01",
                    "title": "谁负责取数",
                    "body": "采集层只回答「读到什么」，不回答「意味着什么」。"
                },
                {
                    "step": "02",
                    "title": "谁负责口径",
                    "body": "计算层持有指标定义，展示层不得就地改口径。"
                },
                {
                    "step": "03",
                    "title": "谁负责呈现",
                    "body": "展示层只按定稿的指标做图形化，不改数据。"
                },
                {
                    "step": "04",
                    "title": "谁负责回滚",
                    "body": "口径变更由计算层统一发版，回滚走同一条链路。"
                }
            ]
        }),
        3 => json!({
            "kind": "compare",
            "aspect_ratio": DEMO_ASPECT,
            "section": "02",
            "title": "自建与采购的取舍",
            "subtitle": "先算清三到五年的维护账",
            "compare_data": {
                "mode": "pros-cons",
                "left": {
                    "title": "自建",
                    "tone": "blue",
                    "items": [
                        {"label": "口径可控", "desc": "指标定义与补数策略都在自己手里", "highlight": true},
                        {"label": "响应快", "desc": "临时口径当天就能上线"}
                    ],
                    "conclusion": "代价是要自己养一支能改采集与计算的队伍。"
                },
                "right": {
                    "title": "采购",
                    "tone": "amber",
                    "items": [
                        {"label": "上线快", "desc": "标准功能开箱即用，两栋楼试点够用", "highlight": true},
                        {"label": "改造慢", "desc": "口径调整要跟厂商排期"}
                    ],
                    "conclusion": "长期成本随改造次数增长。"
                }
            }
        }),
        4 => json!({
            "kind": "timeline",
            "aspect_ratio": DEMO_ASPECT,
            "section": "03",
            "title": "从试点到推广的四个节点",
            "subtitle": "四步走，每步都有验收口径",
            "timeline_events": [
                {
                    "date": "第一步",
                    "title": "对齐口径",
                    "body": "把指标定义固化下来，先冻结三个核心指标。",
                    "tone": "blue"
                },
                {
                    "date": "第二步",
                    "title": "补齐采集",
                    "body": "离线设备补装网关，缺口不再靠人工估算。",
                    "tone": "green"
                },
                {
                    "date": "第三步",
                    "title": "灰度推广",
                    "body": "先在两栋楼试运行，日报同步给业务方。",
                    "tone": "amber"
                },
                {
                    "date": "第四步",
                    "title": "常态运营",
                    "body": "日报交给业务方，平台只看异常与告警。",
                    "tone": "blue"
                }
            ]
        }),
        other => {
            return Err(format!(
                "演示蓝图少了第 {} 页（失败页 {} 由兜底蓝图顶替）",
                other + 1,
                FAILED_PAGE + 1
            ))
        }
    };
    Ok(slide)
}

fn demo_cover() -> Value {
    json!({
        "kind": "cover",
        "aspect_ratio": DEMO_ASPECT,
        "title": "园区能耗治理方案",
        "subtitle": "一次以失败结束的生成，用来验证第二步的看板与单页重试",
        "badges": ["演示数据", "error 运行"]
    })
}

fn demo_overview() -> Value {
    json!({
        "kind": "overview",
        "aspect_ratio": DEMO_ASPECT,
        "section": "00",
        "title": "提纲总览",
        "note": "先由模型拆成逐页内容，再由模型选择组件并生成最终幻灯片。",
        "overview_items": [
            {"number": "01", "title": "总体架构", "desc": "三层职责与接口边界"},
            {"number": "02", "title": "问题与取舍", "desc": "闭环处理与自建采购"},
            {"number": "03", "title": "落地路线", "desc": "四步走与验收口径"}
        ]
    })
}

fn demo_closing() -> Value {
    json!({
        "kind": "closing",
        "aspect_ratio": DEMO_ASPECT,
        "title": "感谢聆听",
        "subtitle": "敬请批评指正",
        "note": "欢迎交流讨论，也可继续提出修改建议。",
        "badges": ["Q&A"]
    })
}

/// 把一份 `Value` 蓝图读成 `SlideBlueprint`。
fn frame_slide(value: Value) -> Result<SlideBlueprint> {
    serde_json::from_value(value).map_err(|error| format!("演示蓝图不合法：{error}"))
}

// ---------------------------------------------------------------------------
// 长牌组
// ---------------------------------------------------------------------------

/// 「长牌组」的四个章节：每复制一轮内容页换一次章节号，标题与章节号因此逐页不同。
const LONG_DECK_CHAPTERS: &[(&str, &str)] = &[
    ("版式基线", "逐页换章节号与页码"),
    ("密度与边界", "卡片数随面板宽度变化"),
    ("图表与指标", "记录缩略图请求与内存"),
    ("交付与核验", "卡片、详情与对话框都要能打开"),
];

/// 长牌组的文稿：第一步会看到它，章节数与牌组一致。
const LONG_DECK_MARKDOWN: &str = "\
# 长牌组：看板虚拟化压测

把「全部版式」的内容页复制到 60 页，用来量第二步看板的可见区要图与内存占用。

## 版式基线

- 每页换章节号与页码，标题不重复
- 缩略图只渲染进入可见区的卡片

## 密度与边界

- 一屏内的卡片数随面板宽度变化
- 滚到底再滚回顶，观察缓存淘汰

## 图表与指标

- 记录实际请求的缩略图页数
- 记录本进程的工作集内存

## 交付与核验

- 卡片、详情与重试对话框都要能正常打开
";

/// 播种「长牌组」：把 `base_deck`（「全部版式」的牌组）的内容页复制到 60 页。
pub fn seed_long_deck(state: &SharedState, base_deck: &[Value]) -> Result<SeededProject> {
    let pages = long_deck_blueprints(base_deck)?;
    let icons = icon_index(state)?;
    // 复制出来的牌组要像一副真实生成完成的牌组：过不了最终校验就不写库。
    let deck = repaired_deck(pages, &icons)?;
    let project_id = projects::create_project(
        state,
        LONG_DECK_PROJECT.to_string(),
        LONG_DECK_MARKDOWN.to_string(),
    )?;
    write_deck(state, project_id, &deck)?;
    Ok(SeededProject {
        id: project_id,
        name: LONG_DECK_PROJECT.to_string(),
        pages: deck.len(),
        // 封面 + 目录 + 内容页 + 结尾。
        content_pages: deck.len() - 3,
        run: None,
    })
}

/// 长牌组的牌组：封面、目录、复制出来的内容页、结尾。
///
/// 内容页按 `LONG_DECK_CONTENT_PAGES` 循环复制 `base_deck` 的内容页，每一份都换章节号、
/// 标题与正文里的页码，因此 60 页里没有两页长得一样。
pub fn long_deck_blueprints(base_deck: &[Value]) -> Result<Vec<Value>> {
    let base_content = content_pages(base_deck);
    if base_content.is_empty() {
        return Err("「全部版式」牌组里没有内容页，无法复制成长牌组".to_string());
    }
    let rounds = LONG_DECK_CONTENT_PAGES.div_ceil(base_content.len());
    let mut deck = Vec::with_capacity(rounds * base_content.len() + 3);
    deck.push(long_deck_cover());
    deck.push(long_deck_overview(rounds));
    let mut page_no = 0usize;
    for round in 0..rounds {
        let chapter_no = chapter_number(round);
        for (slot, base) in base_content.iter().enumerate() {
            page_no += 1;
            deck.push(long_deck_page(base, &chapter_no, slot + 1, page_no));
        }
    }
    deck.push(long_deck_closing());
    Ok(deck)
}

/// 复制出来的一页：换章节号与标题，正文里的每段文本再带上页码。
fn long_deck_page(base: &Value, chapter_no: &str, slot: usize, page_no: usize) -> Value {
    let mut page = base.clone();
    let title = page
        .get("title")
        .and_then(Value::as_str)
        .unwrap_or("未命名页面")
        .to_string();
    page["title"] = json!(format!("{chapter_no}.{slot:02} {title}"));
    page["section"] = json!(chapter_no);
    page["aspect_ratio"] = json!(DEMO_ASPECT);
    restamp_text(&mut page, &format!("（第 {page_no} 页）"));
    page
}

/// 蓝图里承载正文的字段（不含 `kind` / `tone` / `icon` / `image` 这些受校验或会变丑的字段）。
const SLIDE_TEXT_KEYS: &[&str] = &[
    "subtitle",
    "note",
    "footer",
    "example_title",
    "example_body",
    "body",
    "desc",
    "conclusion",
];
/// 里面的字符串也算正文的数组字段（卡片要点、聚光灯面板的步骤…）。
const SLIDE_TEXT_LIST_KEYS: &[&str] = &["items", "steps"];

/// 给每一段正文加上页码后缀，让复制出来的每一页文字都不一样。
fn restamp_text(value: &mut Value, marker: &str) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if field.is_string() && SLIDE_TEXT_KEYS.contains(&key.as_str()) {
                    append_marker(field, marker);
                } else if field.is_array() && SLIDE_TEXT_LIST_KEYS.contains(&key.as_str()) {
                    if let Some(entries) = field.as_array_mut() {
                        for entry in entries {
                            if entry.is_string() {
                                append_marker(entry, marker);
                            } else {
                                restamp_text(entry, marker);
                            }
                        }
                    }
                } else {
                    restamp_text(field, marker);
                }
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                restamp_text(entry, marker);
            }
        }
        _ => {}
    }
}

fn append_marker(value: &mut Value, marker: &str) {
    if let Some(text) = value.as_str() {
        *value = json!(format!("{text}{marker}"));
    }
}

fn long_deck_cover() -> Value {
    json!({
        "kind": "cover",
        "aspect_ratio": DEMO_ASPECT,
        "title": "长牌组：60 页内容页的看板压测",
        "subtitle": "封面、目录、内容页与结尾都在，用来看可见区要图与内存",
        "badges": ["长牌组", "虚拟化"]
    })
}

fn long_deck_overview(rounds: usize) -> Value {
    let items: Vec<Value> = (0..rounds)
        .map(|round| {
            let (title, desc) = LONG_DECK_CHAPTERS[round % LONG_DECK_CHAPTERS.len()];
            json!({
                "number": chapter_number(round),
                "title": title,
                "desc": desc,
            })
        })
        .collect();
    json!({
        "kind": "overview",
        "aspect_ratio": DEMO_ASPECT,
        "section": "00",
        "title": "提纲总览",
        "note": "每一轮复制都换章节号与页码，标题不重复。",
        "overview_items": items
    })
}

fn long_deck_closing() -> Value {
    json!({
        "kind": "closing",
        "aspect_ratio": DEMO_ASPECT,
        "title": "感谢聆听",
        "subtitle": "敬请批评指正",
        "note": "滚到底再滚回顶，观察缩略图的请求与淘汰。",
        "badges": ["Q&A"]
    })
}

/// 牌组里的内容页：首张是封面时切掉封面、目录与结尾，与看板的切法一致
/// （`generation_progress::slide_items`）。
fn content_pages(deck: &[Value]) -> &[Value] {
    let starts_with_cover = deck
        .first()
        .and_then(|page| page.get("kind"))
        .and_then(Value::as_str)
        == Some("cover");
    if starts_with_cover && deck.len() > 3 {
        &deck[2..deck.len() - 1]
    } else {
        deck
    }
}

// ---------------------------------------------------------------------------
// 运行记录与日志
// ---------------------------------------------------------------------------

/// 写一次以 `error` 结束的运行：运行记录 + 日志 + 失败页的一次单页重试。
fn write_generation_run(
    state: &SharedState,
    project_id: i64,
    pages: &[PagePlan],
    layouts: &[LayoutPlan],
    deck: &[SlideBlueprint],
    issues: &[ValidationIssue],
) -> Result<SeededRun> {
    let models = model_names(state);
    // 真实运行的 debug_dir 在项目目录下（`GenerationConfig::debug_dir`）；演示数据把它放在
    // 沙箱的数据目录里，这样真机验证点「重试这一页」时写的调试文件不会落到仓库里。
    let debug_dir = PathBuf::from(&state.lock().unwrap().app_settings.data_dir)
        .join("debug")
        .join("seed-generation-demo")
        .display()
        .to_string();
    let document = parse_markdown(DEMO_MARKDOWN).map_err(|error| error.to_string())?;
    let section_count = document.sections.len();
    let page_plans = serde_json::to_value(pages).map_err(|error| error.to_string())?;
    let layout_plans = serde_json::to_value(layouts).map_err(|error| error.to_string())?;
    // 布局分配的报表与摘要用真实函数算：候选就是每一页被采纳的那个布局（模型首选）。
    let candidates: Vec<Vec<LayoutCandidate>> = layouts
        .iter()
        .map(|plan| {
            vec![LayoutCandidate {
                kind: plan.kind.clone(),
                score: 8,
                reason: plan.reason.clone(),
            }]
        })
        .collect();
    let assignment = layout_assignment_report(&candidates, layouts);
    let assignment_summary = layout_assignment_summary(&candidates, layouts);
    let issue_messages: Vec<String> = issues.iter().map(|issue| issue.message.clone()).collect();
    let failed_pages = invalid_content_pages(issues, deck.len());
    let failed_keys: Vec<usize> = failed_pages.keys().copied().collect();
    let error_message = format!(
        "最终校验未通过：{}。{}",
        describe_deck_issues(&failed_keys, issues, deck.len()),
        issue_messages.join("; ")
    );

    let state_lock = state.lock().unwrap();
    let conn = state_lock.db.lock().unwrap();
    let run_id = db::create_generation_run(
        &conn,
        Some(project_id),
        DEMO_MARKDOWN,
        Some("园区能耗治理方案"),
        &debug_dir,
    )
    .map_err(|error| error.to_string())?;
    let log = |slide_index: Option<i32>,
               stage: &str,
               kind: &str,
               title: &str,
               summary: &str,
               detail: Value,
               important: bool|
     -> Result<()> {
        append_log(
            &conn,
            run_id,
            slide_index,
            stage,
            kind,
            title,
            summary,
            detail,
            important,
        )
    };

    // `emit_progress`（stage / kind "status" / title 就是 stage / detail 带 progress）。
    log(
        None,
        "start",
        "status",
        "start",
        "开始生成流程",
        json!({ "progress": 0.0 }),
        false,
    )?;
    // `generate_slides` 的输入日志（kind "input"，带 aspect_ratio：`get_retry_layouts`
    // 也会从这条日志里取比例）。
    log(
        None,
        "start",
        "input",
        "本次生成输入",
        "记录原始文稿、标题和模型配置",
        json!({
            "source_markdown": DEMO_MARKDOWN,
            "frontmatter_title": "园区能耗治理方案",
            "llm_model": models.llm,
            "embedding_model": models.embedding,
            "multimodal_model": models.multimodal,
            "debug_dir": debug_dir,
            "aspect_ratio": DEMO_ASPECT,
        }),
        true,
    )?;
    // `run_pipeline` 的 init 段。
    log(
        None,
        "init",
        "status",
        "init",
        "验证大模型连接...",
        json!({ "progress": 0.05 }),
        false,
    )?;
    log(
        None,
        "init",
        "config",
        "模型连接检查",
        "模型与向量模型已通过连通性验证",
        json!({
            "llm_model": models.llm,
            "llm_base_url": models.llm_base_url,
            "embedding_model": models.embedding,
            "embedding_base_url": models.embedding_base_url,
        }),
        true,
    )?;
    // `run_pipeline` 的文档结构日志（stage "page_plan" / kind "parsed"）。
    log(
        None,
        "page_plan",
        "parsed",
        "文档结构",
        &format!("文稿解析为 {section_count} 个章节"),
        json!({
            "title": document.title,
            "section_count": section_count,
            "document": document,
        }),
        false,
    )?;
    // `run_pipeline` 的页面规划日志（detail 带 `page_plans`）。
    log(
        None,
        "page_plan",
        "status",
        "页面规划",
        &format!("确定性推导 {} 个页面", pages.len()),
        json!({ "page_plans": page_plans }),
        true,
    )?;
    // `run_pipeline` 的布局分配日志：detail 带 `page_plans` + `layout_plans`，
    // 单页重试与 `get_retry_layouts` 都从这一条取该页的规划与布局。
    log(
        None,
        "layout_plan",
        "status",
        "布局分配",
        &assignment_summary,
        json!({
            "assignment": assignment,
            "page_plans": page_plans,
            "layout_plans": layout_plans,
        }),
        true,
    )?;

    // 逐页的内容日志：成功页是 `content` / `slide`，失败页是 `content` / `page_error`。
    for (index, page) in pages.iter().enumerate() {
        let layout = &layouts[index];
        if index == FAILED_PAGE {
            log(
                Some(index as i32),
                "content",
                "page_error",
                "页面生成失败",
                FAILED_PAGE_ERROR,
                json!({
                    "page_plan": page,
                    "layout_plan": layout,
                    "error": FAILED_PAGE_ERROR,
                }),
                true,
            )?;
            continue;
        }
        let blueprint = &deck[index + 2];
        // 成功页的模型原文就是这份 JSON（流水线把 `.raw.txt` 写进调试目录，解析结果写进
        // `.parsed.json`；界面的「查看当前页调试信息」读的是日志里的同名键）。
        let raw_output = serde_json::to_string_pretty(&json!({ "slide": blueprint }))
            .map_err(|error| error.to_string())?;
        log(
            Some(index as i32),
            "content",
            "slide",
            &format!("幻灯片 {}/{}", index + 1, pages.len()),
            &format!("{} · {:?}", page.page_title, layout.kind),
            json!({
                "page_plan": page,
                "layout_plan": layout,
                "blueprint": blueprint,
                "system_prompt": DEMO_SYSTEM_PROMPT,
                "user_prompt": demo_user_prompt(page, layout),
                "raw_output": raw_output,
                "parsed_output": blueprint,
            }),
            true,
        )?;
    }

    // 有页失败时 `run_pipeline` 会写一条 warning。
    log(
        None,
        "generating",
        "warning",
        "部分页面失败",
        &format!(
            "有 1/{} 张幻灯片生成失败，将使用简化兜底页替代",
            pages.len()
        ),
        json!({ "failed": 1, "total": pages.len() }),
        true,
    )?;
    // 最终校验失败（detail 带 issues）：这次运行以 error 结束就是因为这一条。
    log(
        None,
        "validate",
        "validation",
        "最终校验失败",
        &format!("发现 {} 个校验问题", issues.len()),
        json!({ "issues": issue_messages }),
        true,
    )?;
    // `generate_slides` 失败分支的收尾日志。
    log(
        None,
        "error",
        "error",
        "生成失败",
        &error_message,
        json!({ "error": error_message }),
        true,
    )?;
    db::finish_generation_run(&conn, run_id, "error", Some("error"))
        .map_err(|error| error.to_string())?;

    // 运行结束之后，用户针对失败页做了一次单页重试，还是没成功。
    let retry_layout = LayoutPlan {
        page_id: pages[FAILED_PAGE].page_id.clone(),
        kind: SlideKind::SectionList,
        title: pages[FAILED_PAGE].page_title.clone(),
        subtitle: None,
        section_label: pages[FAILED_PAGE].section_number.clone(),
        reason: "用户手动选择布局进行重试".to_string(),
    };
    // `retry_generation_page` 收尾时写的那条日志：detail 的键与取值照抄。
    log(
        Some(FAILED_PAGE as i32),
        "content",
        "retry",
        "单页重试",
        RETRY_PAGE_ERROR,
        json!({
            "page_plan": pages[FAILED_PAGE],
            "layout_plan": retry_layout,
            "error": RETRY_PAGE_ERROR,
            "retry_succeeded": false,
            "deck_ready": false,
            "remaining": "",
        }),
        true,
    )?;

    let logs = db::get_generation_logs(&conn, run_id)
        .map_err(|error| error.to_string())?
        .len();
    drop(conn);
    drop(state_lock);

    Ok(SeededRun {
        id: run_id,
        logs,
        failed_page: FAILED_PAGE,
    })
}

/// 写一条生成日志。参数顺序与流水线的 `record_log` 一样。
#[allow(clippy::too_many_arguments)]
fn append_log(
    conn: &Connection,
    run_id: i64,
    slide_index: Option<i32>,
    stage: &str,
    kind: &str,
    title: &str,
    summary: &str,
    detail: Value,
    important: bool,
) -> Result<()> {
    db::append_generation_log(
        conn,
        run_id,
        slide_index,
        stage,
        kind,
        title,
        summary,
        &detail,
        important,
    )
    .map(|_| ())
    .map_err(|error| error.to_string())
}

/// 给演示页配一条像样的用户提示词：真实运行里由页面规划、布局方案与组件 schema 拼成。
fn demo_user_prompt(page: &PagePlan, layout: &LayoutPlan) -> String {
    format!(
        "Document title: 园区能耗治理方案\n\nPage plan:\n{}\n\nLayout plan:\n{}\n\nSchema:\nReturn {{\"slide\": SlideBlueprint}}.",
        serde_json::to_string_pretty(page).unwrap_or_default(),
        serde_json::to_string_pretty(layout).unwrap_or_default(),
    )
}

/// 本机的三个模型名与两个地址：写进演示日志，让 `input` / `config` 两条日志带着本机配置。
struct ModelNames {
    llm: String,
    embedding: String,
    multimodal: String,
    llm_base_url: String,
    embedding_base_url: String,
}

fn model_names(state: &SharedState) -> ModelNames {
    let state = state.lock().unwrap();
    let pick = |value: &str| {
        if value.trim().is_empty() {
            "（未配置）".to_string()
        } else {
            value.to_string()
        }
    };
    ModelNames {
        llm: pick(&state.settings.llm.model),
        embedding: pick(&state.settings.embedding.model),
        multimodal: pick(&state.settings.multimodal.model),
        llm_base_url: pick(&state.settings.llm.base_url),
        embedding_base_url: pick(&state.settings.embedding.base_url),
    }
}

// ---------------------------------------------------------------------------
// 公共部分
// ---------------------------------------------------------------------------

/// 真实的那份图标索引（内嵌的 carbon 图标），与流水线校验用的是同一份。
fn icon_index(state: &SharedState) -> Result<IconIndex> {
    let project_dir = state.lock().unwrap().project_dir.clone();
    IconIndex::load(&project_dir).map_err(|error| format!("载入图标索引失败：{error}"))
}

/// 复制出来的内容页按流水线组装后的那套修复过一遍，并确认整副牌组过得了最终校验。
///
/// 「全部版式」的 fixture 是渲染测试用的：有几个图标名与几张卡片的必填字段不符合
/// `validate` 的规则。长牌组要给看板用一整副「生成完成」的牌组，所以这里把不在索引里的
/// 图标清掉（需要的图标由 `apply_component_defaults` 用内置默认值补上），再跑一遍默认值
/// 修复与校验；校验不过就直接播种失败，不写一副过不了校验的牌组进库。
fn repaired_deck(pages: Vec<Value>, icons: &IconIndex) -> Result<Vec<SlideBlueprint>> {
    let mut deck = Vec::with_capacity(pages.len());
    for mut page in pages {
        drop_unknown_icons(&mut page, icons);
        let mut slide = frame_slide(page)?;
        apply_component_defaults(&mut slide);
        normalize_lengths(&mut slide);
        normalize_tones(&mut slide);
        deck.push(slide);
    }
    let issues = validate_blueprints(&deck, icons, &HashSet::new());
    if !issues.is_empty() {
        return Err(format!(
            "{LONG_DECK_PROJECT}过不了最终校验：{}",
            issues
                .iter()
                .map(|issue| issue.message.as_str())
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    Ok(deck)
}

/// 把索引里没有的图标名清成 `null`（`iter_icons` 覆盖到的那些字段都叫 `icon`）。
fn drop_unknown_icons(value: &mut Value, icons: &IconIndex) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                if key == "icon" {
                    if field.as_str().is_some_and(|icon| !icons.contains(icon)) {
                        *field = Value::Null;
                    }
                } else {
                    drop_unknown_icons(field, icons);
                }
            }
        }
        Value::Array(entries) => {
            for entry in entries {
                drop_unknown_icons(entry, icons);
            }
        }
        _ => {}
    }
}

/// 把一个项目的牌组写进库：比例与蓝图的写法与 example 里的 `write_deck` 一致。
fn write_deck(state: &SharedState, project_id: i64, deck: &impl serde::Serialize) -> Result<()> {
    projects::update_project_aspect_ratio(state, project_id, DEMO_ASPECT)?;
    let json = serde_json::to_string_pretty(deck).map_err(|error| error.to_string())?;
    projects::update_project_blueprints(state, project_id, json)
}
