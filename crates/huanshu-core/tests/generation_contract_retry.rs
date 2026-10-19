//! Reproduce live-model structural failures without a model or nondeterministic output.
use huanshu_core::{
    assets::AssetCatalog,
    config::{GenerationConfig, LlmSettings, ModelServiceSettings},
    generator::{
        slides::{generate_one_slide, repair_single_slide},
        utils::blueprint_schema_hint,
    },
    lmstudio::LmStudioClient,
    types::{AspectRatio, LayoutPlan, PagePlan, SlideBlueprint, SlideKind},
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{Arc, Mutex},
    thread::JoinHandle,
    time::Duration,
};

const BAD: &str = r#"{"slide":{"kind":"feature_grid","title":"数据能力","cards":[{"title":"采集","body":"批量接入"},"串入的文字",{"title":"治理","body":"质量核验"}]}}"#;
const GOOD: &str = r#"{"slide":{"kind":"feature_grid","title":"数据能力","cards":[{"title":"采集","body":"批量接入"},{"title":"治理","body":"质量核验"}]}}"#;

fn server(replies: Vec<(u16, &'static str)>) -> (String, Arc<Mutex<Vec<Value>>>, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = requests.clone();
    let task = std::thread::spawn(move || {
        for (status, content) in replies {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(10)))
                .unwrap();
            let mut header = Vec::new();
            while !header.ends_with(b"\r\n\r\n") {
                let mut b = [0];
                stream.read_exact(&mut b).unwrap();
                header.push(b[0]);
                assert!(header.len() < 16384);
            }
            let header = String::from_utf8(header).unwrap().to_lowercase();
            assert!(header.starts_with("post /v1/chat/completions "));
            let length: usize = header
                .lines()
                .find_map(|line| line.strip_prefix("content-length: "))
                .unwrap()
                .parse()
                .unwrap();
            let mut body = vec![0; length];
            stream.read_exact(&mut body).unwrap();
            captured
                .lock()
                .unwrap()
                .push(serde_json::from_slice(&body).unwrap());
            let body = if status == 200 {
                json!({"choices":[{"message":{"content":content}}]}).to_string()
            } else {
                json!({"error":content}).to_string()
            };
            write!(stream,"HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).unwrap();
        }
    });
    (url, requests, task)
}
fn out() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/contract-retry-tests")
        .join(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir
}
async fn generate(url: &str, dir: PathBuf) -> anyhow::Result<SlideBlueprint> {
    let service = ModelServiceSettings::new(url, "", "test-model");
    let client = LmStudioClient::from_service(&service)?;
    let page: PagePlan = serde_json::from_value(
        json!({"page_id":"1","section_title":"数据平台","page_title":"数据能力","objective":"说明采集和治理两项能力","key_points":["采集支持批量接入","治理进行质量核验"]}),
    )?;
    let layout: LayoutPlan = serde_json::from_value(
        json!({"page_id":"1","kind":"feature_grid","title":"数据能力","reason":"两项并列能力"}),
    )?;
    let assets = AssetCatalog::collect(&dir, &[]);
    generate_one_slide(
        client,
        "test-model".into(),
        "数据平台".into(),
        page,
        layout,
        &assets,
        vec![],
        blueprint_schema_hint(&SlideKind::FeatureGrid),
        dir,
        "case".into(),
        0,
    )
    .await
}

#[tokio::test]
async fn malformed_object_array_is_corrected_without_deleting_content_or_switching_kind() {
    let (url, requests, task) = server(vec![(200, BAD), (200, GOOD)]);
    let dir = out();
    let slide = generate(&url, dir.clone()).await.unwrap();
    task.join().unwrap();
    assert_eq!(slide.kind, SlideKind::FeatureGrid);
    assert_eq!(
        slide
            .cards
            .iter()
            .map(|c| c.body.as_deref().unwrap())
            .collect::<Vec<_>>(),
        ["批量接入", "质量核验"]
    );
    let requests = requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    let correction = requests[1]["messages"][1]["content"].as_str().unwrap();
    assert!(correction.contains("cards[1]"));
    assert!(correction.contains("Keep slide.kind='feature_grid'"));
    assert_eq!(
        std::fs::read_to_string(dir.join("case.raw.txt")).unwrap(),
        BAD
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("case-contract-retry-1.raw.txt")).unwrap(),
        GOOD
    );
    assert!(dir.join("case.error.txt").is_file());
}
#[tokio::test]
async fn persistent_contract_errors_stop_after_three_attempts_without_returning_a_fallback() {
    let (url, requests, task) = server(vec![(200, BAD); 3]);
    let error = generate(&url, out()).await.unwrap_err();
    task.join().unwrap();
    assert_eq!(requests.lock().unwrap().len(), 3);
    assert!(format!("{error:#}").contains("still invalid after 3 attempts"));
}
#[tokio::test]
async fn transport_errors_are_not_misreported_as_contract_repairs() {
    let (url, requests, task) = server(vec![(503, "model unavailable")]);
    let error = generate(&url, out()).await.unwrap_err();
    task.join().unwrap();
    assert_eq!(requests.lock().unwrap().len(), 1);
    assert!(format!("{error:#}").contains("503"));
}
#[tokio::test]
async fn user_feedback_repairs_use_the_same_contract_retry_path() {
    let (url, requests, task) = server(vec![(200, BAD), (200, GOOD)]);
    let mut settings = LlmSettings::default();
    settings.llm = ModelServiceSettings::new(url, "", "test-model");
    let config = GenerationConfig::from_settings(&settings, out(), AspectRatio::Ratio16x9);
    let original: SlideBlueprint = serde_json::from_value(
        json!({"kind":"feature_grid","title":"旧标题","cards":[{"title":"采集","body":"接入"}]}),
    )
    .unwrap();
    let repaired = repair_single_slide(&config, &original, "改进说明", None, None)
        .await
        .unwrap();
    task.join().unwrap();
    assert_eq!(repaired.kind, original.kind);
    assert_eq!(repaired.cards.len(), 2);
    assert_eq!(requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn chart_repairs_preserve_source_data_and_application_metadata() {
    let replacement = r#"{"slide":{"kind":"chart_focus","title":"更新解读","points":[{"title":"稳定增长","body":"交付量持续上升"}],"chart":{"type":"bar","title":"模型修改的数据","data":[{"q":"一季度","v":999}],"encoding":{"x":"q","y":"v"}}}}"#;
    let (url, requests, task) = server(vec![(200, replacement)]);
    let mut settings = LlmSettings::default();
    settings.llm = ModelServiceSettings::new(url, "", "test-model");
    let config = GenerationConfig::from_settings(&settings, out(), AspectRatio::Ratio32x9);
    let original:SlideBlueprint=serde_json::from_value(json!({"kind":"chart_focus","title":"季度数据","section":"03","aspect_ratio":"ratio_32x9",
        "points":[{"title":"解读","body":"说明"}],"chart":{"type":"bar","title":"用户原始数据","data":[{"q":"一季度","v":12}],"encoding":{"x":"q","y":"v"}}})).unwrap();
    let repaired = repair_single_slide(&config, &original, "修改文字", None, None)
        .await
        .unwrap();
    task.join().unwrap();
    assert_eq!(
        serde_json::to_value(&repaired.chart).unwrap(),
        serde_json::to_value(&original.chart).unwrap()
    );
    assert_eq!(repaired.section, original.section);
    assert_eq!(repaired.aspect_ratio, original.aspect_ratio);
    assert_eq!(requests.lock().unwrap().len(), 1);
}
