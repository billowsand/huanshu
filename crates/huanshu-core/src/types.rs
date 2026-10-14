use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AspectRatio {
    #[serde(rename = "ratio_16x9")]
    Ratio16x9,
    #[serde(rename = "ratio_32x9")]
    Ratio32x9,
    #[serde(rename = "ratio_48x9")]
    Ratio48x9,
}

impl AspectRatio {
    pub fn width(self) -> u32 {
        match self {
            AspectRatio::Ratio16x9 => 1280,
            AspectRatio::Ratio32x9 => 2560,
            AspectRatio::Ratio48x9 => 3840,
        }
    }

    pub fn height(self) -> u32 {
        720
    }

    pub fn label(self) -> &'static str {
        match self {
            AspectRatio::Ratio16x9 => "16:9 (1280×720)",
            AspectRatio::Ratio32x9 => "32:9 (2560×720)",
            AspectRatio::Ratio48x9 => "48:9 (3840×720)",
        }
    }

    pub fn cols_for_grid(self, base_cols: u32) -> u32 {
        match self {
            AspectRatio::Ratio16x9 => base_cols,
            AspectRatio::Ratio32x9 => base_cols * 2,
            AspectRatio::Ratio48x9 => base_cols * 3,
        }
    }
}

fn default_font() -> String {
    "Microsoft YaHei".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(default)]
pub struct FontSettings {
    pub interface: String,
    pub title: String,
    pub subtitle: String,
    pub body: String,
    pub caption: String,
    pub number: String,
    pub code: String,
}

impl Default for FontSettings {
    fn default() -> Self {
        Self {
            interface: default_font(),
            title: default_font(),
            subtitle: default_font(),
            body: default_font(),
            caption: default_font(),
            number: default_font(),
            code: default_font(),
        }
    }
}

impl FontSettings {
    pub fn normalize(&mut self) {
        for font in [
            &mut self.interface,
            &mut self.title,
            &mut self.subtitle,
            &mut self.body,
            &mut self.caption,
            &mut self.number,
            &mut self.code,
        ] {
            *font = if font.trim().is_empty() {
                default_font()
            } else {
                font.trim().to_string()
            };
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppGlobalSettings {
    pub data_dir: String,
    pub media_dir: String,
    pub llm_configured: bool,
    pub embeddings_ready: bool,
    #[serde(default)]
    pub initialized_embedding_model: String,
    #[serde(default)]
    pub fonts: FontSettings,
}

impl Default for AppGlobalSettings {
    fn default() -> Self {
        let default_data = dirs::data_local_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join("auto-slidev-studio");
        Self {
            data_dir: default_data.to_string_lossy().to_string(),
            media_dir: default_data.join("media").to_string_lossy().to_string(),
            llm_configured: false,
            embeddings_ready: false,
            initialized_embedding_model: String::new(),
            fonts: FontSettings::default(),
        }
    }
}

#[cfg(test)]
mod typography_tests {
    use super::*;

    #[test]
    fn legacy_settings_default_to_yahei() {
        let settings: AppGlobalSettings = serde_json::from_str(
            r#"{
            "data_dir": "data", "media_dir": "media",
            "llm_configured": false, "embeddings_ready": false
        }"#,
        )
        .unwrap();
        assert_eq!(settings.fonts, FontSettings::default());
    }

    #[test]
    fn partial_fonts_preserve_customization_and_roundtrip() {
        let mut fonts: FontSettings =
            serde_json::from_str(r#"{"title":"  KaiTi  ","body":"思源黑体","code":" "}"#).unwrap();
        fonts.normalize();
        assert_eq!(fonts.title, "KaiTi");
        assert_eq!(fonts.body, "思源黑体");
        assert_eq!(fonts.interface, "Microsoft YaHei");
        assert_eq!(fonts.code, "Microsoft YaHei");
        let restored: FontSettings =
            serde_json::from_str(&serde_json::to_string(&fonts).unwrap()).unwrap();
        assert_eq!(restored, fonts);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SlideKind {
    Cover,
    Closing,
    Overview,
    SectionIntro,
    FeatureGrid,
    Spotlight,
    SplitLayers,
    SectionList,
    FocusExample,
    OutcomeGrid,
    CenterGrid,
    Timeline,
    StepFlow,
    Process,
    Compare,
    IssueStack,
    Swot,
    ChartFocus,
}

impl SlideKind {
    /// Every kind, in declaration order. Used by the editor schema and the layout tables so
    /// none of them can silently miss a kind.
    pub const ALL: &'static [SlideKind] = &[
        SlideKind::Cover,
        SlideKind::Closing,
        SlideKind::Overview,
        SlideKind::SectionIntro,
        SlideKind::FeatureGrid,
        SlideKind::Spotlight,
        SlideKind::SplitLayers,
        SlideKind::SectionList,
        SlideKind::FocusExample,
        SlideKind::OutcomeGrid,
        SlideKind::CenterGrid,
        SlideKind::Timeline,
        SlideKind::StepFlow,
        SlideKind::Process,
        SlideKind::Compare,
        SlideKind::IssueStack,
        SlideKind::Swot,
        SlideKind::ChartFocus,
    ];

    /// The snake_case name this kind serializes to, matching the layout tables'
    /// `LAYOUT_RULES` keys and `SUPPORTED_KINDS`.
    pub fn as_str(&self) -> &'static str {
        match self {
            SlideKind::Cover => "cover",
            SlideKind::Closing => "closing",
            SlideKind::Overview => "overview",
            SlideKind::SectionIntro => "section_intro",
            SlideKind::FeatureGrid => "feature_grid",
            SlideKind::Spotlight => "spotlight",
            SlideKind::SplitLayers => "split_layers",
            SlideKind::SectionList => "section_list",
            SlideKind::FocusExample => "focus_example",
            SlideKind::OutcomeGrid => "outcome_grid",
            SlideKind::CenterGrid => "center_grid",
            SlideKind::Timeline => "timeline",
            SlideKind::StepFlow => "step_flow",
            SlideKind::Process => "process",
            SlideKind::Compare => "compare",
            SlideKind::IssueStack => "issue_stack",
            SlideKind::Swot => "swot",
            SlideKind::ChartFocus => "chart_focus",
        }
    }
}

/// 图表 DSL：markdown 中 ```chart 围栏块的内容，原样贯穿 生成 → blueprint → 双端渲染。
/// LLM 不得改写其中的数据（validate.rs 把关），normalize 只做默认值修复。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSpec {
    /// line | bar | donut | grouped_bar | stacked_area
    #[serde(rename = "type")]
    pub chart_type: String,
    pub title: String,
    /// 行数据数组；每行是一个 { 列名: 值 } 对象
    pub data: Vec<serde_json::Value>,
    pub encoding: ChartEncoding,
    /// 多系列图表（grouped_bar / stacked_area）的系列定义，按调色板顺序取色
    #[serde(default)]
    pub series: Vec<ChartSeries>,
    #[serde(default)]
    pub emphasis: Option<ChartEmphasis>,
    /// user_input | llm_generated | tool —— 决定来源角标文案
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub source_note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartEncoding {
    #[serde(default)]
    pub x: Option<String>,
    #[serde(default)]
    pub y: Option<String>,
    /// donut：分类列名 / 数值列名
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartSeries {
    pub name: String,
    /// data 行对象中取值用的键
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartEmphasis {
    /// 需要高亮的类目名（x 值或 donut 的 label 值）
    #[serde(default)]
    pub highlight: Vec<String>,
    #[serde(default)]
    pub trend: Option<String>,
}

pub const CHART_TYPES: &[&str] = &["line", "bar", "donut", "grouped_bar", "stacked_area"];

impl ChartSpec {
    /// Structural problems that make the chart unrenderable, in English. The data comes from the
    /// user's markdown, so these are reported at parse time (`input.rs`) where the user can fix
    /// them; `validate.rs` re-checks edited decks. The LLM repair loop cannot fix chart data and
    /// must never be asked to.
    ///
    /// The rules live in [`crate::chart_check`] so the editor's JSON check cannot drift from
    /// these; this only re-serializes and reuses them.
    pub fn problems(&self) -> Vec<String> {
        let value = serde_json::to_value(self).unwrap_or(Value::Null);
        crate::chart_check::chart_problems_json(&value)
            .iter()
            .map(crate::chart_check::ChartProblem::message_en)
            .collect()
    }
}

/// Tolerant kind deserialization for persisted decks: a kind that no longer exists (for example the
/// removed `infographic` layout) must not fail the whole project load. Falls back to `Cover`,
/// matching the unknown-kind fallback in the frontend `SlideRenderer.vue`.
fn de_slide_kind<'de, D>(deserializer: D) -> Result<SlideKind, D::Error>
where
    D: Deserializer<'de>,
{
    let raw = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(raw).unwrap_or(SlideKind::Cover))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideBlueprint {
    #[serde(deserialize_with = "de_slide_kind")]
    pub kind: SlideKind,
    pub aspect_ratio: Option<AspectRatio>,
    pub section: Option<String>,
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub badge: Option<String>,
    #[serde(default)]
    pub accent: Option<String>,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub label_tone: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub images: Vec<String>,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub side_width: Option<String>,
    #[serde(default)]
    pub badges: Vec<String>,
    #[serde(default)]
    pub overview_items: Vec<OverviewItem>,
    #[serde(default)]
    pub cards: Vec<GridCard>,
    #[serde(default)]
    pub panels: Vec<SpotlightPanel>,
    #[serde(default)]
    pub left_items: Vec<ListItem>,
    #[serde(default)]
    pub layers: Vec<LayerItem>,
    #[serde(default)]
    pub list_items: Vec<ListItem>,
    #[serde(default)]
    pub points: Vec<ListItem>,
    #[serde(default)]
    pub ranking: Vec<RankingItem>,
    #[serde(default)]
    pub center_items: Vec<CenterItem>,
    #[serde(default)]
    pub footer: Option<String>,
    #[serde(default)]
    pub example_title: Option<String>,
    #[serde(default)]
    pub example_body: Option<String>,
    #[serde(default)]
    pub timeline_events: Vec<TimelineEvent>,
    #[serde(default)]
    pub steps: Vec<StepItem>,
    #[serde(default)]
    pub phases: Vec<PhaseItem>,
    #[serde(default)]
    pub direction: Option<String>,
    #[serde(default)]
    pub compare_data: Option<CompareData>,
    #[serde(default)]
    pub swot_data: Option<SwotData>,
    /// chart_focus 布局的图表数据；来自 markdown ```chart 块，LLM 原样搬运
    #[serde(default)]
    pub chart: Option<ChartSpec>,
}

impl SlideBlueprint {
    pub fn iter_icons(&self) -> impl Iterator<Item = &str> {
        self.cards
            .iter()
            .filter_map(|x| x.icon.as_deref())
            .chain(self.panels.iter().filter_map(|x| x.icon.as_deref()))
            .chain(self.left_items.iter().filter_map(|x| x.icon.as_deref()))
            .chain(self.list_items.iter().filter_map(|x| x.icon.as_deref()))
            .chain(self.points.iter().filter_map(|x| x.icon.as_deref()))
            .chain(self.center_items.iter().filter_map(|x| x.icon.as_deref()))
            .chain(self.steps.iter().filter_map(|x| x.icon.as_deref()))
            .chain(self.phases.iter().filter_map(|x| x.icon.as_deref()))
            .chain(
                self.timeline_events
                    .iter()
                    .filter_map(|x| x.icon.as_deref()),
            )
            .chain(
                self.compare_data
                    .iter()
                    .flat_map(|data| [&data.left, &data.right])
                    .filter_map(|side| side.icon.as_deref()),
            )
            .chain(
                self.swot_data
                    .iter()
                    .flat_map(|data| data.quadrants.iter())
                    .filter_map(|x| x.icon.as_deref()),
            )
    }

    pub fn iter_images(&self) -> impl Iterator<Item = &str> {
        self.image
            .iter()
            .map(String::as_str)
            .chain(self.images.iter().map(String::as_str))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OverviewItem {
    pub number: String,
    pub title: String,
    pub desc: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GridCard {
    pub title: String,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub footer_tag: Option<String>,
    #[serde(default)]
    pub footer_tone: Option<String>,
    #[serde(default)]
    pub top_bar_class: Option<String>,
    #[serde(default)]
    pub risk: Option<String>,
    #[serde(default)]
    pub tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpotlightPanel {
    pub title: String,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub steps: Vec<String>,
    #[serde(default)]
    pub highlight: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItem {
    #[serde(default)]
    pub step: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub title: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayerItem {
    pub title: String,
    pub meta: String,
    #[serde(default)]
    pub tone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RankingItem {
    pub index: String,
    pub label: String,
    pub meta: String,
    #[serde(default)]
    pub muted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEvent {
    pub date: String,
    pub title: String,
    pub body: String,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CenterItem {
    pub title: String,
    pub desc: String,
    pub icon: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepItem {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseStep {
    pub label: String,
    #[serde(default)]
    pub desc: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseItem {
    pub phase: String,
    pub title: String,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub tone: Option<String>,
    pub steps: Vec<PhaseStep>,
    #[serde(default)]
    pub highlight: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareItem {
    pub label: String,
    #[serde(default)]
    pub desc: Option<String>,
    #[serde(default)]
    pub highlight: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareSide {
    pub title: String,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub items: Vec<CompareItem>,
    #[serde(default)]
    pub conclusion: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompareData {
    pub mode: Option<String>,
    pub left: CompareSide,
    pub right: CompareSide,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwotQuadrant {
    pub key: String,
    pub title: String,
    #[serde(default)]
    pub tone: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    pub items: Vec<String>,
    #[serde(default)]
    pub summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwotData {
    pub quadrants: Vec<SwotQuadrant>,
    #[serde(default)]
    pub strategy: Option<String>,
}

/// One ranked layout option for a page. The LLM response is parsed leniently by
/// `generator::candidate_parsing` (not by deserializing this type directly), then filtered
/// by `planning::sanitize_candidates` before deck-level assignment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCandidate {
    pub kind: SlideKind,
    #[serde(default)]
    pub score: u8,
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlueprintWrapper {
    pub slide: SlideBlueprint,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepairResponse {
    pub slides: Vec<SlideBlueprint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagePlan {
    #[serde(deserialize_with = "de_string")]
    pub page_id: String,
    #[serde(deserialize_with = "de_string")]
    pub section_title: String,
    #[serde(default)]
    pub subsection_title: Option<String>,
    #[serde(deserialize_with = "de_string")]
    pub page_title: String,
    #[serde(deserialize_with = "de_string")]
    pub objective: String,
    pub key_points: Vec<String>,
    #[serde(default)]
    pub takeaway: Option<String>,
    #[serde(default)]
    pub content_shape: Option<String>,
    #[serde(default)]
    pub layout_intent: Option<String>,
    #[serde(default)]
    pub visual_need: Option<String>,
    #[serde(default)]
    pub object_count: Option<String>,
    #[serde(default)]
    pub argument_mode: Option<String>,
    #[serde(default)]
    pub density: Option<String>,
    #[serde(default)]
    pub source_excerpt: Option<String>,
    #[serde(default)]
    pub preferred_assets: Vec<String>,
    /// "content" or "section_summary" — set by deterministic page derivation
    #[serde(default)]
    pub page_role: Option<String>,
    /// Chapter number shown on the slide ("01", "02", …): 1-based index of the page's H2
    /// section, matching the overview slide. Set by deterministic page derivation and
    /// stamped onto the generated slide by code; the LLM never decides it.
    #[serde(default)]
    pub section_number: Option<String>,
    /// 本页关联的图表（来自 markdown ```chart 块，按出现位置归属到小节）。
    /// 有图表的页才有资格选 chart_focus 布局。
    #[serde(default)]
    pub chart: Option<ChartSpec>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPlan {
    #[serde(deserialize_with = "de_string")]
    pub page_id: String,
    pub kind: SlideKind,
    #[serde(deserialize_with = "de_string")]
    pub title: String,
    #[serde(default)]
    pub subtitle: Option<String>,
    #[serde(default)]
    pub section_label: Option<String>,
    pub reason: String,
}

pub fn de_string<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    match Value::deserialize(deserializer)? {
        Value::String(s) => Ok(s),
        Value::Number(n) => Ok(n.to_string()),
        Value::Bool(b) => Ok(b.to_string()),
        other => Err(serde::de::Error::custom(format!(
            "expected string-like value, got {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_infographic_kind_degrades_without_losing_slides() {
        let legacy = r#"[{"kind":"infographic","title":"\u65e7\u4fe1\u606f\u56fe\u9875"},{"kind":"swot","title":"\u77e9\u9635"}]"#;
        let deck: Vec<SlideBlueprint> =
            serde_json::from_str(legacy).expect("legacy deck must load");
        assert_eq!(deck.len(), 2, "removing a kind must not wipe the deck");
        assert_eq!(deck[0].kind, SlideKind::Cover);
        assert_eq!(deck[1].kind, SlideKind::Swot);
    }
}
