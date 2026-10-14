//! 编辑器的模式表：每种版式的名称、图标、说明、字段与默认内容，以及表单字段的类型、
//! 标签、占位符、下拉选项和「新增一项」的模板。
//!
//! 这些规则原本只存在于 TypeScript（`useSlideEditor.ts` 与 `Step3Editor.vue`），界面通过
//! `get_editor_schema` 取到这份内容渲染。`tests/fixtures/editor-schema.json` 是搬家前从旧表
//! 导出的快照，单测逐字比对它，证明搬家无损。

use serde::Serialize;
use serde_json::{json, Value};

/// 一个输入框。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorField {
    pub key: String,
    pub label: String,
    /// `text` | `textarea` | `select` | `boolean`
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rows: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

/// 一组「字符串数组」字段（如卡片的要点列表）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StringListDef {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub placeholder: Option<String>,
}

/// 列表项里再嵌一层列表（如流程阶段的步骤）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NestedListDef {
    pub key: String,
    pub label: String,
    /// 「新增一项」时写进牌组的内容。
    #[serde(rename = "new_item")]
    pub new_item: Value,
    pub fields: Vec<EditorField>,
}

/// 一个可增删的列表区。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListSectionDef {
    pub key: String,
    pub label: String,
    pub empty_text: String,
    /// 「新增一项」时写进牌组的内容（TypeScript 里是 `createItem` 函数）。
    #[serde(rename = "new_item")]
    pub new_item: Value,
    pub fields: Vec<EditorField>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub string_lists: Vec<StringListDef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub nested_lists: Vec<NestedListDef>,
    /// 用这些字段的文本去查图标建议。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon_query_keys: Option<Vec<String>>,
}

/// 一个固定结构的区块（如对比的左右两侧、SWOT 的四个象限）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectSectionDef {
    pub key: String,
    pub label: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub fields: Vec<EditorField>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub string_lists: Vec<StringListDef>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub list_sections: Vec<ListSectionDef>,
}

/// 一种版式在编辑器里的全部信息。
#[derive(Debug, Clone, Serialize)]
pub struct KindEntry {
    /// `SlideKind` 的 snake_case 名，与牌组里的 `kind` 字段取值一致。
    pub kind: String,
    pub label: String,
    pub icon: String,
    pub desc: String,
    /// 该版式用到的 `SlideBlueprint` 字段名。
    pub fields: Vec<String>,
    /// 新建该版式时的默认内容（不含 `kind` 本身）。
    pub defaults: Value,
}

/// 编辑器需要的全部模式信息。界面启动后取一次并缓存。
#[derive(Debug, Clone, Serialize)]
pub struct EditorSchema {
    pub kinds: Vec<KindEntry>,
    pub simple_fields: Vec<EditorField>,
    pub list_sections: Vec<(String, Vec<ListSectionDef>)>,
    pub object_sections: Vec<(String, Vec<ObjectSectionDef>)>,
    pub tone_options: Vec<String>,
    pub direction_options: Vec<String>,
}

impl EditorSchema {
    pub fn kind(&self, kind: &str) -> Option<&KindEntry> {
        self.kinds.iter().find(|entry| entry.kind == kind)
    }

    pub fn list_sections(&self, kind: &str) -> &[ListSectionDef] {
        self.list_sections
            .iter()
            .find(|(name, _)| name == kind)
            .map(|(_, defs)| defs.as_slice())
            .unwrap_or_default()
    }

    pub fn object_sections(&self, kind: &str) -> &[ObjectSectionDef] {
        self.object_sections
            .iter()
            .find(|(name, _)| name == kind)
            .map(|(_, defs)| defs.as_slice())
            .unwrap_or_default()
    }
}

/// The whole editor schema, built on demand; the UI caches it after the first call.
pub fn editor_schema() -> EditorSchema {
    EditorSchema {
        kinds: vec![
            KindEntry {
                kind: "cover".to_string(),
                label: "封面".to_string(),
                icon: "i-carbon:home".to_string(),
                desc: "演示文稿首页，标题 + 副标题 + 标签".to_string(),
                fields: ["kind", "title", "subtitle", "badges"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "title": json!("AI 战略规划报告"),
                    "subtitle": json!("从业务机会到落地路径的完整说明"),
                    "badges": json!([
                        json!("2026 Q2"),
                        json!("内部汇报"),
                        json!("30 分钟"),
                    ]),
                }),
            },
            KindEntry {
                kind: "closing".to_string(),
                label: "结束页".to_string(),
                icon: "i-carbon:collapse-categories".to_string(),
                desc: "固定收尾页，感谢聆听 + 批评指正".to_string(),
                fields: ["kind", "title", "subtitle", "note", "badges"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "title": json!("感谢聆听"),
                    "subtitle": json!("欢迎继续交流方案细节与排期"),
                    "note": json!("如需会后材料，可补充联系人、二维码或下一步安排。"),
                    "badges": json!([
                        json!("Q&A"),
                        json!("Follow-up"),
                    ]),
                }),
            },
            KindEntry {
                kind: "overview".to_string(),
                label: "概览".to_string(),
                icon: "i-carbon:list-bulleted".to_string(),
                desc: "章节导航，数字编号列表".to_string(),
                fields: ["kind", "section", "title", "note", "overview_items"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "section": json!("01"),
                    "title": json!("内容概览"),
                    "note": json!("建议保持 3-5 个模块，便于听众快速建立全局认知。"),
                    "overview_items": json!([
                        json!({
                            "number": json!("01"),
                            "title": json!("现状判断"),
                            "desc": json!("当前业务痛点、市场变化与内部约束。"),
                        }),
                        json!({
                            "number": json!("02"),
                            "title": json!("方案设计"),
                            "desc": json!("核心策略、能力架构与关键动作。"),
                        }),
                        json!({
                            "number": json!("03"),
                            "title": json!("落地计划"),
                            "desc": json!("阶段目标、资源需求与里程碑安排。"),
                        }),
                    ]),
                }),
            },
            KindEntry {
                kind: "section_intro".to_string(),
                label: "章节导览".to_string(),
                icon: "i-carbon:roadmap".to_string(),
                desc: "章节开场说明 + 子主题预览卡片".to_string(),
                fields: [
                    "kind", "section", "badge", "title", "subtitle", "note", "cards",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "section": json!("02"),
                    "badge": json!("章节导览"),
                    "title": json!("解决方案总览"),
                    "subtitle": json!("先看结构，再展开每个关键模块。"),
                    "note": json!("卡片标题建议短，正文控制在一行到两行。"),
                    "cards": json!([
                        json!({
                            "title": json!("目标定义"),
                            "icon": json!("i-carbon:target"),
                            "tone": json!("blue"),
                            "body": json!("明确业务目标、成效口径与验收标准。"),
                            "tag": json!("Why"),
                        }),
                        json!({
                            "title": json!("能力建设"),
                            "icon": json!("i-carbon:skill-level-advanced"),
                            "tone": json!("emerald"),
                            "body": json!("搭建数据、模型、流程三层能力。"),
                            "tag": json!("What"),
                        }),
                        json!({
                            "title": json!("推进机制"),
                            "icon": json!("i-carbon:workflow-automation"),
                            "tone": json!("amber"),
                            "body": json!("建立跨团队协同与持续优化闭环。"),
                            "tag": json!("How"),
                        }),
                    ]),
                }),
            },
            KindEntry {
                kind: "feature_grid".to_string(),
                label: "特性网格".to_string(),
                icon: "i-carbon:grid".to_string(),
                desc: "多列卡片展示核心特性".to_string(),
                fields: ["kind", "section", "title", "subtitle", "note", "cards"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "section": json!("03"),
                    "title": json!("核心特性"),
                    "subtitle": json!("适合展示 3-6 个能力点，每个卡片一条主信息。"),
                    "note": json!("可补充 `items`、`conclusion`、`risk` 等字段增强信息层次。"),
                    "cards": json!([
                        json!({
                            "title": json!("统一知识入口"),
                            "subtitle": json!("降低信息查找成本"),
                            "icon": json!("i-carbon:data-base"),
                            "tone": json!("blue"),
                            "body": json!("把分散文档、案例、规范集中到一个检索入口。"),
                            "items": json!([
                                json!("支持语义搜索"),
                                json!("权限可控"),
                                json!("可接业务系统"),
                            ]),
                        }),
                        json!({
                            "title": json!("智能内容生成"),
                            "subtitle": json!("缩短方案产出时间"),
                            "icon": json!("i-carbon:machine-learning-model"),
                            "tone": json!("violet"),
                            "body": json!("基于模板与上下文快速生成初稿，再由人工精修。"),
                            "conclusion": json!("适合标准化程度高的交付场景"),
                        }),
                        json!({
                            "title": json!("过程可追踪"),
                            "subtitle": json!("方便复盘与审计"),
                            "icon": json!("i-carbon:task-complete"),
                            "tone": json!("emerald"),
                            "body": json!("保留版本、输入来源和关键决策记录。"),
                            "risk": json!("需提前定义记录粒度"),
                        }),
                    ]),
                }),
            },
            KindEntry {
                kind: "spotlight".to_string(),
                label: "聚焦".to_string(),
                icon: "i-carbon:star-filled".to_string(),
                desc: "左侧图片 + 右侧面板列表".to_string(),
                fields: [
                    "kind",
                    "label",
                    "label_tone",
                    "title",
                    "panels",
                    "images",
                    "image",
                    "placeholder",
                    "side_width",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "label": json!("重点展示"),
                    "label_tone": json!("blue"),
                    "title": json!("重点案例拆解"),
                    "image": json!(""),
                    "images": json!([]),
                    "placeholder": json!("可填写 media:12、文件路径或图片 URL"),
                    "side_width": json!("42%"),
                    "panels": json!([
                        json!({
                            "title": json!("背景"),
                            "kind": json!("text"),
                            "icon": json!("i-carbon:document"),
                            "tone": json!("blue"),
                            "body": json!("客户希望在 6 周内完成销售线索分级与自动跟进方案验证。"),
                        }),
                        json!({
                            "title": json!("关键动作"),
                            "kind": json!("flow"),
                            "icon": json!("i-carbon:flow-stream"),
                            "tone": json!("emerald"),
                            "items": json!([
                                json!("梳理字段"),
                                json!("搭建评分模型"),
                                json!("接入 CRM"),
                                json!("验证转化"),
                            ]),
                            "highlight": json!("从人工分配改为自动优先级推荐"),
                        }),
                        json!({
                            "title": json!("交付结果"),
                            "kind": json!("steps"),
                            "icon": json!("i-carbon:chart-line"),
                            "tone": json!("amber"),
                            "steps": json!([
                                json!("线索响应时效缩短 60%"),
                                json!("销售跟进更聚焦"),
                                json!("复盘数据可量化"),
                            ]),
                        }),
                    ]),
                }),
            },
            KindEntry {
                kind: "split_layers".to_string(),
                label: "分层".to_string(),
                icon: "i-carbon:split-screen".to_string(),
                desc: "左侧解释卡片 + 右侧分层图/信息图".to_string(),
                fields: [
                    "kind",
                    "section",
                    "title",
                    "subtitle",
                    "left_items",
                    "layers",
                    "footer",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "section": json!("04"),
                    "title": json!("分层结构"),
                    "subtitle": json!("适合表达\"左侧说明 + 右侧架构层次\"的页面。"),
                    "left_items": json!([
                        json!({
                            "step": json!("01"),
                            "title": json!("业务层"),
                            "body": json!("围绕用户场景定义服务目标。"),
                            "icon": json!("i-carbon:user-service"),
                        }),
                        json!({
                            "step": json!("02"),
                            "title": json!("能力层"),
                            "body": json!("沉淀通用流程、工具与规则。"),
                            "icon": json!("i-carbon:layers"),
                        }),
                        json!({
                            "step": json!("03"),
                            "title": json!("数据层"),
                            "body": json!("提供指标、知识和底座支撑。"),
                            "icon": json!("i-carbon:data-base"),
                        }),
                    ]),
                    "layers": json!([
                        json!({
                            "title": json!("前台应用"),
                            "meta": json!("面向销售、运营、客服的工作台"),
                            "tone": json!("blue"),
                        }),
                        json!({
                            "title": json!("流程编排"),
                            "meta": json!("任务流、审批流、智能触发器"),
                            "tone": json!("emerald"),
                        }),
                        json!({
                            "title": json!("模型与规则"),
                            "meta": json!("分类、评分、召回、推荐"),
                            "tone": json!("violet"),
                        }),
                        json!({
                            "title": json!("数据底座"),
                            "meta": json!("主数据、知识库、行为日志"),
                            "tone": json!("amber"),
                        }),
                    ]),
                    "footer": json!("建议层数控制在 3-5 层，避免右侧过密。"),
                }),
            },
            KindEntry {
                kind: "section_list".to_string(),
                label: "列表".to_string(),
                icon: "i-carbon:task-view".to_string(),
                desc: "步骤编号 + 内容列表".to_string(),
                fields: ["kind", "section", "title", "subtitle", "list_items"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "section": json!("05"),
                    "title": json!("执行清单"),
                    "subtitle": json!("适合罗列步骤、要点、任务项。"),
                    "list_items": json!([
                        json!({
                            "step": json!("01"),
                            "title": json!("明确目标"),
                            "body": json!("先确认这页要回答的问题，避免只堆信息。"),
                            "icon": json!("i-carbon:target"),
                        }),
                        json!({
                            "step": json!("02"),
                            "title": json!("列关键点"),
                            "body": json!("每条聚焦一个结论，正文尽量不超过两行。"),
                            "icon": json!("i-carbon:idea"),
                        }),
                        json!({
                            "step": json!("03"),
                            "title": json!("补充证据"),
                            "body": json!("必要时加入数据、案例或素材引用。"),
                            "icon": json!("i-carbon:chart-line-data"),
                        }),
                    ]),
                }),
            },
            KindEntry {
                kind: "focus_example".to_string(),
                label: "示例".to_string(),
                icon: "i-carbon:zoom-in".to_string(),
                desc: "要点列表 + 右侧类比示例".to_string(),
                fields: [
                    "kind",
                    "section",
                    "title",
                    "subtitle",
                    "points",
                    "example_title",
                    "example_body",
                    "ranking",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "section": json!("06"),
                    "title": json!("方法说明"),
                    "subtitle": json!("左边讲原则，右边给例子，便于理解抽象概念。"),
                    "points": json!([
                        json!({
                            "step": json!("01"),
                            "title": json!("先给判断标准"),
                            "body": json!("说明什么情况下该采用这套方法。"),
                            "icon": json!("i-carbon:rule"),
                        }),
                        json!({
                            "step": json!("02"),
                            "title": json!("再讲落地动作"),
                            "body": json!("把动作拆成可执行的步骤。"),
                            "icon": json!("i-carbon:play-outline-filled"),
                        }),
                        json!({
                            "step": json!("03"),
                            "title": json!("最后展示效果"),
                            "body": json!("给出产出、收益或变化。"),
                            "icon": json!("i-carbon:result"),
                        }),
                    ]),
                    "example_title": json!("类比例子"),
                    "example_body": json!("可以把这套机制理解成\"先搭导航，再接数据，再让流程自动跑起来\"。"),
                    "ranking": json!([
                        json!({
                            "index": json!("A"),
                            "label": json!("优先统一口径"),
                            "meta": json!("先对齐概念和指标，再谈自动化。"),
                        }),
                        json!({
                            "index": json!("B"),
                            "label": json!("优先打通流程"),
                            "meta": json!("确保数据和执行动作能串起来。"),
                        }),
                        json!({
                            "index": json!("C"),
                            "label": json!("优先做局部试点"),
                            "meta": json!("用小范围结果验证价值。"),
                            "muted": Value::Bool(true),
                        }),
                    ]),
                }),
            },
            KindEntry {
                kind: "outcome_grid".to_string(),
                label: "成果网格".to_string(),
                icon: "i-carbon:chart-bar".to_string(),
                desc: "成果 / 结论的卡片网格".to_string(),
                fields: ["kind", "section", "title", "subtitle", "note", "cards"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "section": json!("07"),
                    "title": json!("阶段成果"),
                    "subtitle": json!("适合总结结果、结论、价值输出。"),
                    "note": json!("可以把 `footer_tag`、`conclusion` 当作每个成果卡片的收束语。"),
                    "cards": json!([
                        json!({
                            "title": json!("效率提升"),
                            "icon": json!("i-carbon:flash"),
                            "tone": json!("amber"),
                            "body": json!("提案准备时间从 2 天缩短到 4 小时。"),
                            "conclusion": json!("团队可把精力转向深度判断"),
                        }),
                        json!({
                            "title": json!("质量更稳"),
                            "icon": json!("i-carbon:checkmark-outline"),
                            "tone": json!("emerald"),
                            "body": json!("输出结构更统一，漏项明显减少。"),
                            "footer_tag": json!("可复用"),
                        }),
                        json!({
                            "title": json!("协作顺畅"),
                            "icon": json!("i-carbon:group"),
                            "tone": json!("blue"),
                            "body": json!("销售、方案、交付之间共享同一套上下文。"),
                            "footer_tone": json!("blue"),
                        }),
                    ]),
                }),
            },
            KindEntry {
                kind: "center_grid".to_string(),
                label: "中心网格".to_string(),
                icon: "i-carbon:grid-dots".to_string(),
                desc: "居中大字 + 下方图标网格".to_string(),
                fields: ["kind", "badge", "title", "accent", "center_items", "footer"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "badge": json!("核心观点"),
                    "title": json!("战略落地不是单点优化，而是能力系统建设"),
                    "accent": json!("统一口径、流程串联、数据闭环"),
                    "center_items": json!([
                        json!({
                            "title": json!("目标"),
                            "desc": json!("先定义成效"),
                            "icon": json!("i-carbon:target"),
                            "tone": json!("blue"),
                        }),
                        json!({
                            "title": json!("机制"),
                            "desc": json!("再固化流程"),
                            "icon": json!("i-carbon:workflow-automation"),
                            "tone": json!("emerald"),
                        }),
                        json!({
                            "title": json!("数据"),
                            "desc": json!("持续反馈优化"),
                            "icon": json!("i-carbon:data-vis-4"),
                            "tone": json!("violet"),
                        }),
                        json!({
                            "title": json!("组织"),
                            "desc": json!("明确协作责任"),
                            "icon": json!("i-carbon:user-multiple"),
                            "tone": json!("amber"),
                        }),
                    ]),
                    "footer": json!("中间标题适合放一句最重要的结论。"),
                }),
            },
            KindEntry {
                kind: "timeline".to_string(),
                label: "时间线".to_string(),
                icon: "i-carbon:time".to_string(),
                desc: "横向时间轴事件序列".to_string(),
                fields: [
                    "kind",
                    "section",
                    "title",
                    "subtitle",
                    "timeline_events",
                    "footer",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "section": json!("08"),
                    "title": json!("推进时间线"),
                    "subtitle": json!("用时间轴表达阶段节奏与里程碑。"),
                    "timeline_events": json!([
                        json!({
                            "date": json!("第 1 周"),
                            "title": json!("需求对齐"),
                            "body": json!("完成目标确认、范围边界和数据盘点。"),
                            "tone": json!("blue"),
                            "icon": json!("i-carbon:flag"),
                        }),
                        json!({
                            "date": json!("第 2-3 周"),
                            "title": json!("方案搭建"),
                            "body": json!("输出流程设计、页面原型和字段映射。"),
                            "tone": json!("violet"),
                            "icon": json!("i-carbon:build-tool"),
                        }),
                        json!({
                            "date": json!("第 4-5 周"),
                            "title": json!("试点运行"),
                            "body": json!("接入真实业务，跟踪异常和效果。"),
                            "tone": json!("emerald"),
                            "icon": json!("i-carbon:play-filled"),
                        }),
                        json!({
                            "date": json!("第 6 周"),
                            "title": json!("复盘优化"),
                            "body": json!("沉淀 SOP、指标面板和后续迭代项。"),
                            "tone": json!("amber"),
                            "icon": json!("i-carbon:renew"),
                        }),
                    ]),
                    "footer": json!("时间点可写日期，也可写\"第 N 周 / 阶段名\"。"),
                }),
            },
            KindEntry {
                kind: "step_flow".to_string(),
                label: "步骤流".to_string(),
                icon: "i-carbon:flow".to_string(),
                desc: "横向 / 纵向步骤流程".to_string(),
                fields: [
                    "kind",
                    "section",
                    "title",
                    "subtitle",
                    "direction",
                    "steps",
                    "footer",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "section": json!("09"),
                    "title": json!("实施步骤"),
                    "subtitle": json!("横向适合展示流程，纵向适合展示说明更长的步骤。"),
                    "direction": json!("horizontal"),
                    "steps": json!([
                        json!({
                            "title": json!("识别问题"),
                            "body": json!("确认最值得优先解决的业务卡点。"),
                            "icon": json!("i-carbon:search"),
                            "tone": json!("blue"),
                        }),
                        json!({
                            "title": json!("设计机制"),
                            "body": json!("定义流程、规则和人机协作方式。"),
                            "icon": json!("i-carbon:flow"),
                            "tone": json!("violet"),
                        }),
                        json!({
                            "title": json!("接入系统"),
                            "body": json!("把能力挂到真实业务链路中。"),
                            "icon": json!("i-carbon:application-web"),
                            "tone": json!("emerald"),
                        }),
                        json!({
                            "title": json!("持续优化"),
                            "body": json!("基于数据反馈迭代策略。"),
                            "icon": json!("i-carbon:chart-line"),
                            "tone": json!("amber"),
                        }),
                    ]),
                    "footer": json!("如需竖排，可把 direction 改为 vertical。"),
                }),
            },
            KindEntry {
                kind: "process".to_string(),
                label: "流程".to_string(),
                icon: "i-carbon:process".to_string(),
                desc: "多阶段流程任务分解".to_string(),
                fields: ["kind", "section", "title", "subtitle", "phases", "footer"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "section": json!("10"),
                    "title": json!("执行流程"),
                    "subtitle": json!("适合多阶段、多步骤的任务分解。"),
                    "phases": json!([
                        json!({
                            "phase": json!("Phase 1"),
                            "title": json!("准备阶段"),
                            "icon": json!("i-carbon:document-preliminary"),
                            "tone": json!("blue"),
                            "steps": json!([
                                json!({
                                    "label": json!("明确负责人"),
                                    "desc": json!("确定项目 owner 与参与角色。"),
                                }),
                                json!({
                                    "label": json!("梳理输入物"),
                                    "desc": json!("准备文档、数据、历史案例。"),
                                }),
                            ]),
                            "highlight": json!("确保启动前信息齐备"),
                        }),
                        json!({
                            "phase": json!("Phase 2"),
                            "title": json!("实施阶段"),
                            "icon": json!("i-carbon:play-outline"),
                            "tone": json!("emerald"),
                            "steps": json!([
                                json!({
                                    "label": json!("搭建流程"),
                                    "desc": json!("把方案转成可运行链路。"),
                                }),
                                json!({
                                    "label": json!("小范围试点"),
                                    "desc": json!("先验证关键环节效果。"),
                                }),
                            ]),
                        }),
                        json!({
                            "phase": json!("Phase 3"),
                            "title": json!("优化阶段"),
                            "icon": json!("i-carbon:analytics"),
                            "tone": json!("amber"),
                            "steps": json!([
                                json!({
                                    "label": json!("复盘问题"),
                                    "desc": json!("识别异常与低效节点。"),
                                }),
                                json!({
                                    "label": json!("沉淀标准"),
                                    "desc": json!("固化 SOP 与复用模板。"),
                                }),
                            ]),
                        }),
                    ]),
                    "footer": json!("每个阶段建议 2-4 个步骤，避免页面过满。"),
                }),
            },
            KindEntry {
                kind: "compare".to_string(),
                label: "对比".to_string(),
                icon: "i-carbon:compare".to_string(),
                desc: "左右双栏对比分析".to_string(),
                fields: [
                    "kind",
                    "section",
                    "title",
                    "subtitle",
                    "compare_data",
                    "footer",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "section": json!("11"),
                    "title": json!("方案对比"),
                    "subtitle": json!("左右两栏适合比较不同路线、工具或策略。"),
                    "compare_data": json!({
                        "mode": json!("side-by-side"),
                        "left": json!({
                            "title": json!("方案 A：快速上线"),
                            "tone": json!("blue"),
                            "icon": json!("i-carbon:flash"),
                            "items": json!([
                                json!({
                                    "label": json!("优势"),
                                    "desc": json!("投入小、启动快、便于试错。"),
                                    "highlight": Value::Bool(true),
                                }),
                                json!({
                                    "label": json!("代价"),
                                    "desc": json!("后期扩展性一般，规则维护成本较高。"),
                                }),
                                json!({
                                    "label": json!("适用场景"),
                                    "desc": json!("需求还不稳定，先验证业务价值。"),
                                }),
                            ]),
                            "conclusion": json!("适合先跑通闭环"),
                        }),
                        "right": json!({
                            "title": json!("方案 B：体系化建设"),
                            "tone": json!("emerald"),
                            "icon": json!("i-carbon:enterprise"),
                            "items": json!([
                                json!({
                                    "label": json!("优势"),
                                    "desc": json!("长期复用性强，适合多团队推广。"),
                                    "highlight": Value::Bool(true),
                                }),
                                json!({
                                    "label": json!("代价"),
                                    "desc": json!("前期协调与建设成本更高。"),
                                }),
                                json!({
                                    "label": json!("适用场景"),
                                    "desc": json!("业务成熟，准备做规模化复制。"),
                                }),
                            ]),
                            "conclusion": json!("适合中长期投入"),
                        }),
                    }),
                    "footer": json!("需要上下对比时，可调整 mode 字段。"),
                }),
            },
            KindEntry {
                kind: "issue_stack".to_string(),
                label: "问题堆叠".to_string(),
                icon: "i-carbon:warning-alt".to_string(),
                desc: "纵向堆叠的问题/挑战说明卡片".to_string(),
                fields: ["kind", "section", "title", "subtitle", "cards", "footer"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "section": json!("12"),
                    "title": json!("为什么这件事难做"),
                    "subtitle": json!("核心障碍、根因与可能的突破口"),
                    "cards": json!([
                        json!({
                            "title": json!("黑盒中的黑盒"),
                            "icon": json!("i-carbon:ibm-watson-machine-learning"),
                            "tone": json!("blue"),
                            "body": json!("底层能力强依赖框架与通信栈本身，定位链路深、调试窗口窄，管理成本高。"),
                            "items": json!([
                                json!("难调试"),
                                json!("难追踪"),
                                json!("难治理"),
                                json!("难控制"),
                            ]),
                        }),
                        json!({
                            "title": json!("恢复链路不完整"),
                            "icon": json!("i-carbon:warning-other"),
                            "tone": json!("green"),
                            "body": json!("即使上层已有编排系统，真正的自愈、自恢复和自动缓解仍然难以稳定落地。"),
                            "items": json!([
                                json!("难自愈"),
                                json!("难恢复"),
                                json!("难缓解"),
                            ]),
                        }),
                        json!({
                            "title": json!("可观测性缺块"),
                            "icon": json!("i-carbon:network-4"),
                            "tone": json!("amber"),
                            "body": json!("驱动、硬件、GPU 与网络故障往往跨层出现，根因信息分散，日志与事件不完整。"),
                            "items": json!([
                                json!("根因难判定"),
                                json!("事件难采集"),
                                json!("观测不足"),
                            ]),
                        }),
                    ]),
                    "footer": json!("适合 2-4 个纵向问题块，每块一句主判断 + 若干补充标签。"),
                }),
            },
            KindEntry {
                kind: "swot".to_string(),
                label: "SWOT".to_string(),
                icon: "i-carbon:analytics".to_string(),
                desc: "四象限战略分析矩阵".to_string(),
                fields: ["kind", "section", "title", "subtitle", "swot_data"]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                defaults: json!({
                    "section": json!("12"),
                    "title": json!("SWOT 分析"),
                    "subtitle": json!("用于总结内外部条件并给出战略建议。"),
                    "swot_data": json!({
                        "quadrants": json!([
                            json!({
                                "key": json!("S"),
                                "title": json!("优势 Strengths"),
                                "tone": json!("emerald"),
                                "icon": json!("i-carbon:thumbs-up"),
                                "items": json!([
                                    json!("已有行业案例积累"),
                                    json!("团队对业务流程熟悉"),
                                ]),
                                "summary": json!("内部基础较好"),
                            }),
                            json!({
                                "key": json!("W"),
                                "title": json!("劣势 Weaknesses"),
                                "tone": json!("amber"),
                                "icon": json!("i-carbon:warning"),
                                "items": json!([
                                    json!("数据口径不统一"),
                                    json!("系统集成链路较长"),
                                ]),
                                "summary": json!("落地复杂度偏高"),
                            }),
                            json!({
                                "key": json!("O"),
                                "title": json!("机会 Opportunities"),
                                "tone": json!("blue"),
                                "icon": json!("i-carbon:growth"),
                                "items": json!([
                                    json!("客户对自动化需求提升"),
                                    json!("管理层支持试点预算"),
                                ]),
                                "summary": json!("外部窗口期明确"),
                            }),
                            json!({
                                "key": json!("T"),
                                "title": json!("威胁 Threats"),
                                "tone": json!("rose"),
                                "icon": json!("i-carbon:storm-tracker"),
                                "items": json!([
                                    json!("竞品推进速度快"),
                                    json!("合规要求可能收紧"),
                                ]),
                                "summary": json!("需要控制推进节奏"),
                            }),
                        ]),
                        "strategy": json!("建议采用\"试点先行 + 能力沉淀\"策略，先快速证明价值，再逐步扩展。"),
                    }),
                }),
            },
            KindEntry {
                kind: "chart_focus".to_string(),
                label: "图表页".to_string(),
                icon: "i-carbon:chart-line-smooth".to_string(),
                desc: "要点解读 + 数据图表（数据来自 ```chart 围栏块）".to_string(),
                fields: [
                    "kind", "section", "title", "subtitle", "points", "chart", "footer",
                ]
                .iter()
                .map(|s| s.to_string())
                .collect(),
                defaults: json!({
                    "section": json!("13"),
                    "title": json!("关键指标解读"),
                    "subtitle": json!("数据来自文档中的 ```chart 围栏块，正文负责解读而非复述。"),
                    "points": json!([
                        json!({
                            "step": json!("01"),
                            "title": json!("趋势判断"),
                            "body": json!("说明指标的整体走向与阶段特征。"),
                        }),
                        json!({
                            "step": json!("02"),
                            "title": json!("关键拐点"),
                            "body": json!("指出变化最明显的时间点或分类并解释原因。"),
                        }),
                        json!({
                            "step": json!("03"),
                            "title": json!("行动建议"),
                            "body": json!("基于图表给出可执行的下一步建议。"),
                        }),
                    ]),
                    "chart": json!({
                        "type": json!("bar"),
                        "title": json!("季度交付量（示例）"),
                        "data": json!([
                            json!({
                                "quarter": json!("Q1"),
                                "count": json!(12),
                            }),
                            json!({
                                "quarter": json!("Q2"),
                                "count": json!(19),
                            }),
                            json!({
                                "quarter": json!("Q3"),
                                "count": json!(27),
                            }),
                            json!({
                                "quarter": json!("Q4"),
                                "count": json!(34),
                            }),
                        ]),
                        "encoding": json!({
                            "x": json!("quarter"),
                            "y": json!("count"),
                        }),
                        "source": json!("llm_generated"),
                    }),
                    "footer": json!("示例数据，仅用于版式预览；真实数据请在文档中以 ```chart 围栏块提供。"),
                }),
            },
        ],
        simple_fields: simple_fields(),
        list_sections: vec![
            (
                "overview".to_string(),
                vec![ListSectionDef {
                    key: "overview_items".to_string(),
                    label: "概览条目".to_string(),
                    empty_text: "还没有概览条目".to_string(),
                    new_item: json!({
                        "number": json!(""),
                        "title": json!(""),
                        "desc": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "number".to_string(),
                            label: "编号".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("01".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("现状判断".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "desc".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("一句话解释本节内容".to_string()),
                            rows: Some(2),
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: None,
                }],
            ),
            (
                "section_intro".to_string(),
                vec![ListSectionDef {
                    key: "cards".to_string(),
                    label: "导览卡片".to_string(),
                    empty_text: "还没有导览卡片".to_string(),
                    new_item: json!({
                        "title": json!(""),
                        "body": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                        "tag": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("目标定义".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("卡片正文".to_string()),
                            rows: Some(3),
                            options: None,
                        },
                        EditorField {
                            key: "tag".to_string(),
                            label: "标签".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("Why".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "body", "tag"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                }],
            ),
            (
                "feature_grid".to_string(),
                vec![ListSectionDef {
                    key: "cards".to_string(),
                    label: "特性卡片".to_string(),
                    empty_text: "还没有特性卡片".to_string(),
                    new_item: json!({
                        "title": json!(""),
                        "subtitle": json!(""),
                        "body": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                        "items": json!([]),
                    }),
                    fields: vec![
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("统一知识入口".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "subtitle".to_string(),
                            label: "副标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("降低信息查找成本".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "正文".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("卡片正文".to_string()),
                            rows: Some(3),
                            options: None,
                        },
                        EditorField {
                            key: "conclusion".to_string(),
                            label: "结论".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("适合标准化程度高的交付场景".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "risk".to_string(),
                            label: "风险".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("需提前定义记录粒度".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "footer_tag".to_string(),
                            label: "底部标签".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("可复用".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "footer_tone".to_string(),
                            label: "标签色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "tag".to_string(),
                            label: "卡片标签".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("Why".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "主色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![StringListDef {
                        key: "items".to_string(),
                        label: "要点列表".to_string(),
                        placeholder: Some("支持语义搜索".to_string()),
                    }],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "subtitle", "body"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                }],
            ),
            (
                "spotlight".to_string(),
                vec![ListSectionDef {
                    key: "panels".to_string(),
                    label: "右侧信息面板".to_string(),
                    empty_text: "还没有信息面板".to_string(),
                    new_item: json!({
                        "title": json!(""),
                        "kind": json!("text"),
                        "body": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                        "items": json!([]),
                        "steps": json!([]),
                    }),
                    fields: vec![
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("关键动作".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "kind".to_string(),
                            label: "类型".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("flow / text / steps".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "正文".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("面板正文".to_string()),
                            rows: Some(3),
                            options: None,
                        },
                        EditorField {
                            key: "highlight".to_string(),
                            label: "高亮信息".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("一句重点结论".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![
                        StringListDef {
                            key: "items".to_string(),
                            label: "要点列表".to_string(),
                            placeholder: Some("梳理字段".to_string()),
                        },
                        StringListDef {
                            key: "steps".to_string(),
                            label: "结果列表".to_string(),
                            placeholder: Some("线索响应时效缩短 60%".to_string()),
                        },
                    ],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "body", "highlight"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                }],
            ),
            (
                "split_layers".to_string(),
                vec![
                    ListSectionDef {
                        key: "left_items".to_string(),
                        label: "左侧说明项".to_string(),
                        empty_text: "还没有左侧说明项".to_string(),
                        new_item: json!({
                            "step": json!(""),
                            "title": json!(""),
                            "body": json!(""),
                            "icon": json!(""),
                        }),
                        fields: vec![
                            EditorField {
                                key: "step".to_string(),
                                label: "步骤号".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("01".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "title".to_string(),
                                label: "标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("业务层".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "body".to_string(),
                                label: "说明".to_string(),
                                field_type: "textarea".to_string(),
                                placeholder: Some("一句说明".to_string()),
                                rows: Some(2),
                                options: None,
                            },
                            EditorField {
                                key: "icon".to_string(),
                                label: "图标".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                                rows: None,
                                options: None,
                            },
                        ],
                        string_lists: vec![],
                        nested_lists: vec![],
                        icon_query_keys: Some(
                            ["title", "body"].iter().map(|s| s.to_string()).collect(),
                        ),
                    },
                    ListSectionDef {
                        key: "layers".to_string(),
                        label: "右侧分层".to_string(),
                        empty_text: "还没有右侧分层".to_string(),
                        new_item: json!({
                            "title": json!(""),
                            "meta": json!(""),
                            "tone": json!("blue"),
                        }),
                        fields: vec![
                            EditorField {
                                key: "title".to_string(),
                                label: "层标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("流程编排".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "meta".to_string(),
                                label: "层说明".to_string(),
                                field_type: "textarea".to_string(),
                                placeholder: Some("这一层做什么".to_string()),
                                rows: Some(2),
                                options: None,
                            },
                            EditorField {
                                key: "tone".to_string(),
                                label: "色调".to_string(),
                                field_type: "select".to_string(),
                                placeholder: None,
                                rows: None,
                                options: Some(
                                    [
                                        "blue", "emerald", "green", "violet", "amber", "rose",
                                        "gray", "red",
                                    ]
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                                ),
                            },
                        ],
                        string_lists: vec![],
                        nested_lists: vec![],
                        icon_query_keys: None,
                    },
                ],
            ),
            (
                "section_list".to_string(),
                vec![ListSectionDef {
                    key: "list_items".to_string(),
                    label: "列表项".to_string(),
                    empty_text: "还没有列表项".to_string(),
                    new_item: json!({
                        "step": json!(""),
                        "title": json!(""),
                        "body": json!(""),
                        "icon": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "step".to_string(),
                            label: "编号".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("01".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("明确目标".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("一句说明".to_string()),
                            rows: Some(2),
                            options: None,
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "body"].iter().map(|s| s.to_string()).collect(),
                    ),
                }],
            ),
            (
                "focus_example".to_string(),
                vec![
                    ListSectionDef {
                        key: "points".to_string(),
                        label: "要点".to_string(),
                        empty_text: "还没有要点".to_string(),
                        new_item: json!({
                            "step": json!(""),
                            "title": json!(""),
                            "body": json!(""),
                            "icon": json!(""),
                        }),
                        fields: vec![
                            EditorField {
                                key: "step".to_string(),
                                label: "编号".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("01".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "title".to_string(),
                                label: "标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("先给判断标准".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "body".to_string(),
                                label: "说明".to_string(),
                                field_type: "textarea".to_string(),
                                placeholder: Some("一句说明".to_string()),
                                rows: Some(2),
                                options: None,
                            },
                            EditorField {
                                key: "icon".to_string(),
                                label: "图标".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                                rows: None,
                                options: None,
                            },
                        ],
                        string_lists: vec![],
                        nested_lists: vec![],
                        icon_query_keys: Some(
                            ["title", "body"].iter().map(|s| s.to_string()).collect(),
                        ),
                    },
                    ListSectionDef {
                        key: "ranking".to_string(),
                        label: "排序/优先级".to_string(),
                        empty_text: "还没有排序项".to_string(),
                        new_item: json!({
                            "index": json!(""),
                            "label": json!(""),
                            "meta": json!(""),
                            "muted": Value::Bool(false),
                        }),
                        fields: vec![
                            EditorField {
                                key: "index".to_string(),
                                label: "编号".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("A".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "label".to_string(),
                                label: "标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("优先统一口径".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "meta".to_string(),
                                label: "说明".to_string(),
                                field_type: "textarea".to_string(),
                                placeholder: Some("一句说明".to_string()),
                                rows: Some(2),
                                options: None,
                            },
                            EditorField {
                                key: "muted".to_string(),
                                label: "弱化显示".to_string(),
                                field_type: "boolean".to_string(),
                                placeholder: None,
                                rows: None,
                                options: None,
                            },
                        ],
                        string_lists: vec![],
                        nested_lists: vec![],
                        icon_query_keys: None,
                    },
                ],
            ),
            (
                "outcome_grid".to_string(),
                vec![ListSectionDef {
                    key: "cards".to_string(),
                    label: "成果卡片".to_string(),
                    empty_text: "还没有成果卡片".to_string(),
                    new_item: json!({
                        "title": json!(""),
                        "body": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("效率提升".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("成果说明".to_string()),
                            rows: Some(3),
                            options: None,
                        },
                        EditorField {
                            key: "conclusion".to_string(),
                            label: "结论".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("团队可把精力转向深度判断".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "footer_tag".to_string(),
                            label: "底部标签".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("可复用".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "footer_tone".to_string(),
                            label: "标签色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "主色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "body", "conclusion"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                }],
            ),
            (
                "center_grid".to_string(),
                vec![ListSectionDef {
                    key: "center_items".to_string(),
                    label: "中心能力点".to_string(),
                    empty_text: "还没有能力点".to_string(),
                    new_item: json!({
                        "title": json!(""),
                        "desc": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("目标".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "desc".to_string(),
                            label: "说明".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("先定义成效".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "desc"].iter().map(|s| s.to_string()).collect(),
                    ),
                }],
            ),
            (
                "timeline".to_string(),
                vec![ListSectionDef {
                    key: "timeline_events".to_string(),
                    label: "时间线事件".to_string(),
                    empty_text: "还没有时间线事件".to_string(),
                    new_item: json!({
                        "date": json!(""),
                        "title": json!(""),
                        "body": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "date".to_string(),
                            label: "时间".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("第 1 周".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("需求对齐".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("这一阶段做什么".to_string()),
                            rows: Some(2),
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["date", "title", "body"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                }],
            ),
            (
                "step_flow".to_string(),
                vec![ListSectionDef {
                    key: "steps".to_string(),
                    label: "流程步骤".to_string(),
                    empty_text: "还没有流程步骤".to_string(),
                    new_item: json!({
                        "title": json!(""),
                        "body": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("识别问题".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("步骤说明".to_string()),
                            rows: Some(2),
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "body"].iter().map(|s| s.to_string()).collect(),
                    ),
                }],
            ),
            (
                "process".to_string(),
                vec![ListSectionDef {
                    key: "phases".to_string(),
                    label: "流程阶段".to_string(),
                    empty_text: "还没有流程阶段".to_string(),
                    new_item: json!({
                        "phase": json!(""),
                        "title": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                        "highlight": json!(""),
                        "steps": json!([]),
                    }),
                    fields: vec![
                        EditorField {
                            key: "phase".to_string(),
                            label: "阶段名".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("Phase 1".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("准备阶段".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "highlight".to_string(),
                            label: "高亮提示".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("确保启动前信息齐备".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![NestedListDef {
                        key: "steps".to_string(),
                        label: "阶段步骤".to_string(),
                        new_item: json!({
                            "label": json!(""),
                            "desc": json!(""),
                        }),
                        fields: vec![
                            EditorField {
                                key: "label".to_string(),
                                label: "步骤标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("明确负责人".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "desc".to_string(),
                                label: "步骤说明".to_string(),
                                field_type: "textarea".to_string(),
                                placeholder: Some("说明这一步做什么".to_string()),
                                rows: Some(2),
                                options: None,
                            },
                        ],
                    }],
                    icon_query_keys: Some(
                        ["phase", "title", "highlight"]
                            .iter()
                            .map(|s| s.to_string())
                            .collect(),
                    ),
                }],
            ),
            (
                "issue_stack".to_string(),
                vec![ListSectionDef {
                    key: "cards".to_string(),
                    label: "问题卡片".to_string(),
                    empty_text: "还没有问题卡片".to_string(),
                    new_item: json!({
                        "title": json!(""),
                        "body": json!(""),
                        "tone": json!("blue"),
                        "icon": json!(""),
                        "items": json!([]),
                    }),
                    fields: vec![
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("黑盒中的黑盒".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("问题说明".to_string()),
                            rows: Some(3),
                            options: None,
                        },
                        EditorField {
                            key: "tone".to_string(),
                            label: "色调".to_string(),
                            field_type: "select".to_string(),
                            placeholder: None,
                            rows: None,
                            options: Some(
                                [
                                    "blue", "emerald", "green", "violet", "amber", "rose", "gray",
                                    "red",
                                ]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                            ),
                        },
                        EditorField {
                            key: "icon".to_string(),
                            label: "图标".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                            rows: None,
                            options: None,
                        },
                    ],
                    string_lists: vec![StringListDef {
                        key: "items".to_string(),
                        label: "补充标签".to_string(),
                        placeholder: Some("难调试".to_string()),
                    }],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "body"].iter().map(|s| s.to_string()).collect(),
                    ),
                }],
            ),
            (
                "chart_focus".to_string(),
                vec![ListSectionDef {
                    key: "points".to_string(),
                    label: "解读要点".to_string(),
                    empty_text: "还没有解读要点".to_string(),
                    new_item: json!({
                        "step": json!(""),
                        "title": json!(""),
                        "body": json!(""),
                    }),
                    fields: vec![
                        EditorField {
                            key: "step".to_string(),
                            label: "编号".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("01".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "title".to_string(),
                            label: "标题".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("趋势判断".to_string()),
                            rows: None,
                            options: None,
                        },
                        EditorField {
                            key: "body".to_string(),
                            label: "说明".to_string(),
                            field_type: "textarea".to_string(),
                            placeholder: Some("一句解读".to_string()),
                            rows: Some(2),
                            options: None,
                        },
                    ],
                    string_lists: vec![],
                    nested_lists: vec![],
                    icon_query_keys: Some(
                        ["title", "body"].iter().map(|s| s.to_string()).collect(),
                    ),
                }],
            ),
        ],
        object_sections: vec![
            (
                "compare".to_string(),
                vec![
                    ObjectSectionDef {
                        key: "compare_data".to_string(),
                        label: "对比设置".to_string(),
                        fields: vec![EditorField {
                            key: "mode".to_string(),
                            label: "布局模式".to_string(),
                            field_type: "text".to_string(),
                            placeholder: Some("side-by-side".to_string()),
                            rows: None,
                            options: None,
                        }],
                        string_lists: vec![],
                        list_sections: vec![],
                    },
                    ObjectSectionDef {
                        key: "compare_data.left".to_string(),
                        label: "左侧方案".to_string(),
                        fields: vec![
                            EditorField {
                                key: "title".to_string(),
                                label: "标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("方案 A：快速上线".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "tone".to_string(),
                                label: "色调".to_string(),
                                field_type: "select".to_string(),
                                placeholder: None,
                                rows: None,
                                options: Some(
                                    [
                                        "blue", "emerald", "green", "violet", "amber", "rose",
                                        "gray", "red",
                                    ]
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                                ),
                            },
                            EditorField {
                                key: "icon".to_string(),
                                label: "图标".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "conclusion".to_string(),
                                label: "结论".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("适合先跑通闭环".to_string()),
                                rows: None,
                                options: None,
                            },
                        ],
                        string_lists: vec![],
                        list_sections: vec![ListSectionDef {
                            key: "items".to_string(),
                            label: "条目".to_string(),
                            empty_text: "还没有左侧条目".to_string(),
                            new_item: json!({
                                "label": json!(""),
                                "desc": json!(""),
                                "highlight": Value::Bool(false),
                            }),
                            fields: vec![
                                EditorField {
                                    key: "label".to_string(),
                                    label: "标题".to_string(),
                                    field_type: "text".to_string(),
                                    placeholder: Some("优势".to_string()),
                                    rows: None,
                                    options: None,
                                },
                                EditorField {
                                    key: "desc".to_string(),
                                    label: "说明".to_string(),
                                    field_type: "textarea".to_string(),
                                    placeholder: Some("一句说明".to_string()),
                                    rows: Some(2),
                                    options: None,
                                },
                                EditorField {
                                    key: "highlight".to_string(),
                                    label: "高亮".to_string(),
                                    field_type: "boolean".to_string(),
                                    placeholder: None,
                                    rows: None,
                                    options: None,
                                },
                            ],
                            string_lists: vec![],
                            nested_lists: vec![],
                            icon_query_keys: None,
                        }],
                    },
                    ObjectSectionDef {
                        key: "compare_data.right".to_string(),
                        label: "右侧方案".to_string(),
                        fields: vec![
                            EditorField {
                                key: "title".to_string(),
                                label: "标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("方案 B：体系化建设".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "tone".to_string(),
                                label: "色调".to_string(),
                                field_type: "select".to_string(),
                                placeholder: None,
                                rows: None,
                                options: Some(
                                    [
                                        "blue", "emerald", "green", "violet", "amber", "rose",
                                        "gray", "red",
                                    ]
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                                ),
                            },
                            EditorField {
                                key: "icon".to_string(),
                                label: "图标".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "conclusion".to_string(),
                                label: "结论".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("适合中长期投入".to_string()),
                                rows: None,
                                options: None,
                            },
                        ],
                        string_lists: vec![],
                        list_sections: vec![ListSectionDef {
                            key: "items".to_string(),
                            label: "条目".to_string(),
                            empty_text: "还没有右侧条目".to_string(),
                            new_item: json!({
                                "label": json!(""),
                                "desc": json!(""),
                                "highlight": Value::Bool(false),
                            }),
                            fields: vec![
                                EditorField {
                                    key: "label".to_string(),
                                    label: "标题".to_string(),
                                    field_type: "text".to_string(),
                                    placeholder: Some("优势".to_string()),
                                    rows: None,
                                    options: None,
                                },
                                EditorField {
                                    key: "desc".to_string(),
                                    label: "说明".to_string(),
                                    field_type: "textarea".to_string(),
                                    placeholder: Some("一句说明".to_string()),
                                    rows: Some(2),
                                    options: None,
                                },
                                EditorField {
                                    key: "highlight".to_string(),
                                    label: "高亮".to_string(),
                                    field_type: "boolean".to_string(),
                                    placeholder: None,
                                    rows: None,
                                    options: None,
                                },
                            ],
                            string_lists: vec![],
                            nested_lists: vec![],
                            icon_query_keys: None,
                        }],
                    },
                ],
            ),
            (
                "swot".to_string(),
                vec![ObjectSectionDef {
                    key: "swot_data".to_string(),
                    label: "SWOT 设置".to_string(),
                    fields: vec![EditorField {
                        key: "strategy".to_string(),
                        label: "战略建议".to_string(),
                        field_type: "textarea".to_string(),
                        placeholder: Some("用于总结整体建议".to_string()),
                        rows: Some(3),
                        options: None,
                    }],
                    string_lists: vec![],
                    list_sections: vec![ListSectionDef {
                        key: "quadrants".to_string(),
                        label: "四象限".to_string(),
                        empty_text: "还没有 SWOT 象限".to_string(),
                        new_item: json!({
                            "key": json!(""),
                            "title": json!(""),
                            "tone": json!("blue"),
                            "icon": json!(""),
                            "items": json!([]),
                            "summary": json!(""),
                        }),
                        fields: vec![
                            EditorField {
                                key: "key".to_string(),
                                label: "象限标识".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("S".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "title".to_string(),
                                label: "标题".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("优势 Strengths".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "summary".to_string(),
                                label: "总结".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("内部基础较好".to_string()),
                                rows: None,
                                options: None,
                            },
                            EditorField {
                                key: "tone".to_string(),
                                label: "色调".to_string(),
                                field_type: "select".to_string(),
                                placeholder: None,
                                rows: None,
                                options: Some(
                                    [
                                        "blue", "emerald", "green", "violet", "amber", "rose",
                                        "gray", "red",
                                    ]
                                    .iter()
                                    .map(|s| s.to_string())
                                    .collect(),
                                ),
                            },
                            EditorField {
                                key: "icon".to_string(),
                                label: "图标".to_string(),
                                field_type: "text".to_string(),
                                placeholder: Some("由 AI 推荐，通常无需手填".to_string()),
                                rows: None,
                                options: None,
                            },
                        ],
                        string_lists: vec![StringListDef {
                            key: "items".to_string(),
                            label: "要点".to_string(),
                            placeholder: Some("已有行业案例积累".to_string()),
                        }],
                        nested_lists: vec![],
                        icon_query_keys: Some(
                            ["key", "title", "summary"]
                                .iter()
                                .map(|s| s.to_string())
                                .collect(),
                        ),
                    }],
                }],
            ),
            (
                "chart_focus".to_string(),
                vec![ObjectSectionDef {
                    key: "chart".to_string(),
                    label: "图表设置".to_string(),
                    fields: vec![EditorField {
                        key: "title".to_string(),
                        label: "图表标题".to_string(),
                        field_type: "text".to_string(),
                        placeholder: Some("季度交付量".to_string()),
                        rows: None,
                        options: None,
                    }],
                    string_lists: vec![],
                    list_sections: vec![],
                }],
            ),
        ],
        tone_options: [
            "blue", "emerald", "green", "violet", "amber", "rose", "gray", "red",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect(),
        direction_options: ["horizontal", "vertical"]
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

/// The plain inputs every kind can show, above its structured sections.
pub fn simple_fields() -> Vec<EditorField> {
    {
        vec![
            EditorField {
                key: "section".to_string(),
                label: "章节号".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("01".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "badge".to_string(),
                label: "角标".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("章节导览".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "label".to_string(),
                label: "标签".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("重点展示".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "label_tone".to_string(),
                label: "标签颜色".to_string(),
                field_type: "select".to_string(),
                placeholder: None,
                rows: None,
                options: Some(
                    [
                        "blue", "emerald", "green", "violet", "amber", "rose", "gray", "red",
                    ]
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
                ),
            },
            EditorField {
                key: "title".to_string(),
                label: "标题".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("输入标题".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "subtitle".to_string(),
                label: "副标题".to_string(),
                field_type: "textarea".to_string(),
                placeholder: Some("输入副标题".to_string()),
                rows: Some(2),
                options: None,
            },
            EditorField {
                key: "accent".to_string(),
                label: "强调语".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("一句强调信息".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "note".to_string(),
                label: "说明".to_string(),
                field_type: "textarea".to_string(),
                placeholder: Some("补充说明、背景或提示".to_string()),
                rows: Some(3),
                options: None,
            },
            EditorField {
                key: "footer".to_string(),
                label: "页脚说明".to_string(),
                field_type: "textarea".to_string(),
                placeholder: Some("补充一句收束语".to_string()),
                rows: Some(2),
                options: None,
            },
            EditorField {
                key: "placeholder".to_string(),
                label: "图片占位提示".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("可填写 media:12 或图片 URL".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "side_width".to_string(),
                label: "侧栏宽度".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("42%".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "direction".to_string(),
                label: "布局方向".to_string(),
                field_type: "select".to_string(),
                placeholder: None,
                rows: None,
                options: Some(
                    ["horizontal", "vertical"]
                        .iter()
                        .map(|s| s.to_string())
                        .collect(),
                ),
            },
            EditorField {
                key: "example_title".to_string(),
                label: "示例标题".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("类比例子".to_string()),
                rows: None,
                options: None,
            },
            EditorField {
                key: "example_body".to_string(),
                label: "示例正文".to_string(),
                field_type: "textarea".to_string(),
                placeholder: Some("输入示例说明".to_string()),
                rows: Some(3),
                options: None,
            },
            EditorField {
                key: "image".to_string(),
                label: "主图路径".to_string(),
                field_type: "text".to_string(),
                placeholder: Some("media:12 或 https://...".to_string()),
                rows: None,
                options: None,
            },
        ]
    }
}

/// The dropdown options a tone or direction field may offer.
pub const TONE_OPTIONS: &[&str] = &[
    "blue", "emerald", "green", "violet", "amber", "rose", "gray", "red",
];
pub const DIRECTION_OPTIONS: &[&str] = &["horizontal", "vertical"];
