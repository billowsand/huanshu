use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct DeckSpec {
    pub title: String,
    pub export_filename: String,
    pub slides: Vec<SlideSpec>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct SlideSpec {
    pub layout: Option<String>,
    pub component: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppGlobalSettings {
    pub data_dir: String,
    pub media_dir: String,
    pub llm_configured: bool,
    pub embeddings_ready: bool,
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
        }
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
    Infographic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlideBlueprint {
    pub kind: SlideKind,
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
    #[serde(default)]
    pub infographic_syntax: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagePlanResponse {
    pub pages: Vec<PagePlan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutPlanResponse {
    pub pages: Vec<LayoutPlan>,
}

/// Top-K ranked layout candidates returned by the LLM for diversity-aware selection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutCandidateResponse {
    pub candidates: Vec<LayoutCandidate>,
}

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
