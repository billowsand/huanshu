//! 「模型服务表单」独立组件（原版 `SettingsView.vue` 362–425 行的表单面板）。
//!
//! 一个 [`ServiceForm`] 实体对应一类服务（llm / embedding / multimodal）的草稿：
//! 服务地址、API Key、模型（下拉或手动输入）、SOCKS5 代理。「获取列表」与「测试连接」
//! 都是 `services::settings::list_models`（async → `tasks::run_async`），结果带发起时的
//! 草稿签名（`serde_json`，等价原版的 `JSON.stringify`），回来时签名不符即视为过时。
//!
//! 首次向导（`pages/wizard.rs`）复用本组件：`ServiceForm::new` + `set_draft` / `draft`。

use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::select::{Select, SelectEvent, SelectState};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::{Disableable as _, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, Context, Entity, Render, Subscription, Window};
use huanshu_core::config::ModelServiceSettings;
use huanshu_core::services::settings;

use crate::app::AppShared;
use crate::icons::carbon;
use crate::tasks::run_async;
use crate::theme::StudioExt as _;
use crate::widgets;

/// 三类模型服务（原版 `ModelTarget`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelTarget {
    Llm,
    Embedding,
    Multimodal,
}

impl ModelTarget {
    pub const ALL: [ModelTarget; 3] = [
        ModelTarget::Llm,
        ModelTarget::Embedding,
        ModelTarget::Multimodal,
    ];

    /// core `list_models` 的 target 参数。
    pub fn key(self) -> &'static str {
        match self {
            ModelTarget::Llm => "llm",
            ModelTarget::Embedding => "embedding",
            ModelTarget::Multimodal => "multimodal",
        }
    }

    pub fn index(self) -> usize {
        match self {
            ModelTarget::Llm => 0,
            ModelTarget::Embedding => 1,
            ModelTarget::Multimodal => 2,
        }
    }

    /// 原版 `services[]` 数组里的静态文案（逐字）。
    pub fn info(self) -> ServiceInfo {
        match self {
            ModelTarget::Llm => ServiceInfo {
                title: "大语言模型",
                subtitle: "文本生成",
                icon: "chat",
                uses: ["Markdown 内容优化", "页面规划与布局", "幻灯片正文生成"],
                hint: "选择支持文本生成的指令模型。",
            },
            ModelTarget::Embedding => ServiceInfo {
                title: "向量模型",
                subtitle: "图标检索",
                icon: "data-base",
                uses: ["图标语义匹配", "内容向量化", "向量缓存初始化"],
                hint: "需要支持 /v1/embeddings 接口。",
            },
            ModelTarget::Multimodal => ServiceInfo {
                title: "多模态模型",
                subtitle: "图片理解",
                icon: "image",
                uses: ["图片内容分析", "图片自动描述", "图文内容理解"],
                hint: "选择支持图像输入的视觉模型。",
            },
        }
    }
}

/// 服务卡片的静态信息（原版 `services[]`，文案逐字）。
pub struct ServiceInfo {
    pub title: &'static str,
    pub subtitle: &'static str,
    pub icon: &'static str,
    pub uses: [&'static str; 3],
    pub hint: &'static str,
}

/// 连接状态机（原版 `ServiceState`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceState {
    Testing,
    Ok,
    Failed,
    Untested,
    Unset,
}

impl ServiceState {
    /// 原版 `serviceStateLabels`（逐字）。
    pub fn label(self) -> &'static str {
        match self {
            ServiceState::Testing => "测试中",
            ServiceState::Ok => "已连接",
            ServiceState::Failed => "连接失败",
            ServiceState::Untested => "待测试",
            ServiceState::Unset => "未配置",
        }
    }

    /// 徽章色调（原版 `.prefs-status.is-*`）。
    pub fn tone(self) -> widgets::Tone2 {
        match self {
            ServiceState::Ok => widgets::Tone2::Success,
            ServiceState::Failed => widgets::Tone2::Error,
            ServiceState::Testing => widgets::Tone2::Primary,
            ServiceState::Untested => widgets::Tone2::Warning,
            ServiceState::Unset => widgets::Tone2::Muted,
        }
    }
}

/// 一次连接测试的结果（原版 `results[target]`）。
#[derive(Debug, Clone)]
pub struct TestResult {
    /// 发起测试那一刻草稿的 `serde_json` 序列化（原版 `JSON.stringify(service)`）。
    pub signature: String,
    pub ok: bool,
    pub elapsed: u64,
    pub error: String,
}

/// 草稿签名（`JSON.stringify` 的等价物）。
pub fn service_signature(service: &ModelServiceSettings) -> String {
    serde_json::to_string(service).unwrap_or_default()
}

/// 原版 `serviceState()`：testing 优先；结果签名匹配当前草稿 → ok/failed；
/// 否则 base_url 与 model 都非空 → untested，缺一个 → unset。
pub fn service_state(
    testing: bool,
    result: Option<&TestResult>,
    draft: &ModelServiceSettings,
) -> ServiceState {
    if testing {
        return ServiceState::Testing;
    }
    if let Some(result) = result {
        if result.signature == service_signature(draft) {
            return if result.ok {
                ServiceState::Ok
            } else {
                ServiceState::Failed
            };
        }
    }
    if !draft.base_url.trim().is_empty() && !draft.model.trim().is_empty() {
        ServiceState::Untested
    } else {
        ServiceState::Unset
    }
}

/// 从服务地址里取 hostname（照原版 `new URL(...).hostname` 的判定；解析失败返回 `None`）。
fn host_of(base_url: &str) -> Option<String> {
    let (_, rest) = base_url.trim().split_once("://")?;
    let authority = rest.split(['/', '?', '#']).next()?;
    let host_port = authority.rsplit('@').next()?;
    if host_port.is_empty() {
        return None;
    }
    if let Some(inner) = host_port.strip_prefix('[') {
        let end = inner.find(']')?;
        let host = inner[..end].to_string();
        return (!host.is_empty()).then(|| format!("[{host}]"));
    }
    let host = host_port.split(':').next()?;
    (!host.is_empty()).then(|| host.to_string())
}

/// 原版 `serviceLocation`：本地 / 远程 / 地址未填写（逐字）。
pub fn service_location(base_url: &str) -> &'static str {
    match host_of(base_url) {
        None => "地址未填写",
        Some(host) if matches!(host.as_str(), "localhost" | "127.0.0.1" | "[::1]") => "本地服务",
        Some(_) => "远程服务",
    }
}

/// 一个服务表单组件：草稿 + 输入框 + 模型下拉 + 测试连接。设置页与首次向导共用。
pub struct ServiceForm {
    target: ModelTarget,
    shared: AppShared,
    /// 向导紧凑模式：只渲染「API 地址 / API Key / 模型」三个字段（照原版
    /// `FirstRunWizard.vue` 的 model-config-panel），无代理、无下拉、无获取列表。
    compact: bool,
    /// 当前草稿（原版 `draft[activeService]`）。
    pub draft: ModelServiceSettings,
    /// 「获取列表」拉到的模型（原版 `modelLists[target]`）。
    pub model_list: Vec<String>,
    pub testing: bool,
    pub result: Option<TestResult>,
    /// 「手动输入模型 ID」开关（原版 `manualModel[target]`）。
    pub manual: bool,
    pub show_key: bool,
    base_url: Entity<InputState>,
    api_key: Entity<InputState>,
    /// 手动输入模型 ID 的输入框。
    model_manual: Entity<InputState>,
    model_select: Entity<SelectState<Vec<String>>>,
    proxy_url: Entity<InputState>,
    proxy_user: Entity<InputState>,
    proxy_pass: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl ServiceForm {
    pub fn new(
        target: ModelTarget,
        draft: ModelServiceSettings,
        shared: AppShared,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(target, draft, shared, false, window, cx)
    }

    /// 向导第二步用的紧凑表单（原版 `FirstRunWizard.vue` 的三个 model-config-panel）。
    pub fn compact(
        target: ModelTarget,
        draft: ModelServiceSettings,
        shared: AppShared,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self::build(target, draft, shared, true, window, cx)
    }

    fn build(
        target: ModelTarget,
        draft: ModelServiceSettings,
        shared: AppShared,
        compact: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let base_url = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("http://127.0.0.1:1234")
                .default_value(draft.base_url.clone())
        });
        let api_key = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(if compact {
                    "sk-..."
                } else {
                    "本地服务可留空"
                })
                .masked(true)
                .default_value(draft.api_key.clone())
        });
        let model_manual = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(if compact {
                    draft.model.clone()
                } else {
                    "输入模型 ID".to_string()
                })
                .default_value(draft.model.clone())
        });
        let model_select = cx.new(|cx| SelectState::new(Vec::<String>::new(), None, window, cx));
        let proxy_url = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("socks5://127.0.0.1:1080")
                .default_value(draft.proxy.url.clone())
        });
        let proxy_user = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("可选")
                .default_value(draft.proxy.username.clone())
        });
        let proxy_pass = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("可选")
                .masked(true)
                .default_value(draft.proxy.password.clone())
        });

        let subscriptions = vec![
            cx.subscribe(&base_url, |this: &mut Self, state, _: &InputEvent, cx| {
                this.draft.base_url = state.read(cx).value().to_string();
                cx.notify();
            }),
            cx.subscribe(&api_key, |this: &mut Self, state, _: &InputEvent, cx| {
                this.draft.api_key = state.read(cx).value().to_string();
                cx.notify();
            }),
            cx.subscribe(
                &model_manual,
                |this: &mut Self, state, _: &InputEvent, cx| {
                    this.draft.model = state.read(cx).value().to_string();
                    cx.notify();
                },
            ),
            cx.subscribe(&proxy_url, |this: &mut Self, state, _: &InputEvent, cx| {
                this.draft.proxy.url = state.read(cx).value().to_string();
                cx.notify();
            }),
            cx.subscribe(&proxy_user, |this: &mut Self, state, _: &InputEvent, cx| {
                this.draft.proxy.username = state.read(cx).value().to_string();
                cx.notify();
            }),
            cx.subscribe(&proxy_pass, |this: &mut Self, state, _: &InputEvent, cx| {
                this.draft.proxy.password = state.read(cx).value().to_string();
                cx.notify();
            }),
            cx.subscribe(
                &model_select,
                |this: &mut Self, _state, event: &SelectEvent<Vec<String>>, cx| {
                    if let SelectEvent::Confirm(Some(value)) = event {
                        this.draft.model = value.clone();
                        cx.notify();
                    }
                },
            ),
        ];

        let mut this = Self {
            target,
            shared,
            compact,
            draft,
            model_list: Vec::new(),
            testing: false,
            result: None,
            manual: false,
            show_key: false,
            base_url,
            api_key,
            model_manual,
            model_select,
            proxy_url,
            proxy_user,
            proxy_pass,
            _subscriptions: subscriptions,
        };
        this.sync_model_select(window, cx);
        this
    }

    /// 当前状态（原版 `serviceState(activeService)`）。
    pub fn state(&self) -> ServiceState {
        service_state(self.testing, self.result.as_ref(), &self.draft)
    }

    /// 服务类型（向导按目标收集草稿时用）。
    pub fn target(&self) -> ModelTarget {
        self.target
    }

    /// 当前草稿生效的测试结果（签名必须匹配，原版 `activeResult`）。
    pub fn active_result(&self) -> Option<&TestResult> {
        let result = self.result.as_ref()?;
        (result.signature == service_signature(&self.draft)).then_some(result)
    }

    /// 下拉选项 = 当前 model + 已拉到的列表去重（原版 `modelOptions`）。
    fn model_options(&self) -> Vec<String> {
        let mut options: Vec<String> = Vec::new();
        for model in std::iter::once(&self.draft.model).chain(self.model_list.iter()) {
            let model = model.trim();
            if !model.is_empty() && !options.iter().any(|seen| seen == model) {
                options.push(model.to_string());
            }
        }
        options
    }

    /// 下拉选项/选中值与草稿同步。
    fn sync_model_select(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let options = self.model_options();
        let model = self.draft.model.clone();
        let selected = (!model.trim().is_empty())
            .then_some(model)
            .filter(|model| options.contains(model));
        self.model_select.update(cx, |state, cx| {
            state.set_items(options, window, cx);
            match selected {
                Some(model) => state.set_selected_value(&model, window, cx),
                None => state.set_selected_index(None, window, cx),
            }
        });
    }

    /// 原版 `testConnection()`：拉模型列表即一次连接测试。
    pub fn test_connection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.testing || self.draft.base_url.trim().is_empty() {
            return;
        }
        self.testing = true;
        cx.notify();
        let service = self.draft.clone();
        let signature = service_signature(&service);
        let target = self.target.key().to_string();
        let shared = self.shared.clone();
        let started = std::time::Instant::now();
        run_async(
            window,
            cx,
            async move {
                settings::list_models(&shared, Some(target), Some(service))
                    .await
                    .map(|models| (models, started.elapsed().as_millis() as u64))
            },
            |this: &mut Self, result, window, cx| {
                this.testing = false;
                match result {
                    Ok((models, elapsed)) => {
                        for model in models {
                            if !this.model_list.contains(&model) {
                                this.model_list.push(model);
                            }
                        }
                        this.result = Some(TestResult {
                            signature,
                            ok: true,
                            elapsed,
                            error: String::new(),
                        });
                    }
                    Err(error) => {
                        this.result = Some(TestResult {
                            signature,
                            ok: false,
                            elapsed: 0,
                            error,
                        });
                    }
                }
                this.sync_model_select(window, cx);
                cx.notify();
            },
        );
    }

    /// 整份替换草稿（撤销、向导回填用）。
    pub fn set_draft(
        &mut self,
        draft: ModelServiceSettings,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.draft = draft.clone();
        self.base_url.update(cx, |state, cx| {
            state.set_value(draft.base_url.clone(), window, cx)
        });
        self.api_key.update(cx, |state, cx| {
            state.set_value(draft.api_key.clone(), window, cx)
        });
        self.model_manual.update(cx, |state, cx| {
            state.set_value(draft.model.clone(), window, cx)
        });
        self.proxy_url.update(cx, |state, cx| {
            state.set_value(draft.proxy.url.clone(), window, cx)
        });
        self.proxy_user.update(cx, |state, cx| {
            state.set_value(draft.proxy.username.clone(), window, cx)
        });
        self.proxy_pass.update(cx, |state, cx| {
            state.set_value(draft.proxy.password.clone(), window, cx)
        });
        self.sync_model_select(window, cx);
        cx.notify();
    }
}

impl Render for ServiceForm {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let c = cx.studio().colors;
        let info = self.target.info();

        // 向导紧凑模式：三个字段竖排（原版 FirstRunWizard.vue 的 model-config-panel）。
        if self.compact {
            let field = |label: &'static str, note: Option<&'static str>, control: AnyElement| {
                div()
                    .flex()
                    .flex_col()
                    .gap(px(3.0))
                    .child(
                        div()
                            .flex()
                            .flex_row()
                            .items_center()
                            .gap(px(4.0))
                            .child(div().text_size(px(12.0)).text_color(c.muted).child(label))
                            .when_some(note, |label_row, note| {
                                label_row.child(
                                    div()
                                        .text_size(px(12.0))
                                        .text_color(c.muted)
                                        .opacity(0.5)
                                        .child(note),
                                )
                            }),
                    )
                    .child(control)
                    .into_any_element()
            };
            let eye = widgets::icon_button(
                "toggle-key",
                if self.show_key { "view-off" } else { "view" },
                if self.show_key {
                    "隐藏 API Key"
                } else {
                    "显示 API Key"
                },
            )
            .on_click(cx.listener(|this, _event, window, cx| {
                this.show_key = !this.show_key;
                this.api_key
                    .update(cx, |state, cx| state.set_masked(!this.show_key, window, cx));
                cx.notify();
            }));
            return div()
                .flex()
                .flex_col()
                .gap(px(10.0))
                .child(field(
                    "API 地址",
                    None,
                    div().child(Input::new(&self.base_url)).into_any_element(),
                ))
                .child(field(
                    "API Key",
                    Some("（本地留空）"),
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(6.0))
                        .child(Input::new(&self.api_key).flex_1())
                        .child(eye)
                        .into_any_element(),
                ))
                .child(field(
                    "模型",
                    None,
                    div()
                        .child(Input::new(&self.model_manual))
                        .into_any_element(),
                ));
        }

        let can_fetch = !self.testing && !self.draft.base_url.trim().is_empty();

        let row = |label: &'static str, control: AnyElement, help: Option<&'static str>| {
            div()
                .flex()
                .flex_row()
                .gap(px(16.0))
                .child(
                    div()
                        .w(px(96.0))
                        .flex_none()
                        .pt(px(7.0))
                        .text_size(px(13.0))
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(4.0))
                        .child(control)
                        .when_some(help, |col, help| {
                            col.child(div().text_size(px(12.0)).text_color(c.muted).child(help))
                        }),
                )
                .into_any_element()
        };

        let url_row = row(
            "服务地址",
            div().child(Input::new(&self.base_url)).into_any_element(),
            Some("兼容 OpenAI 接口，支持本地或远程服务。"),
        );

        let eye = widgets::icon_button(
            "toggle-key",
            if self.show_key { "view-off" } else { "view" },
            if self.show_key {
                "隐藏 API Key"
            } else {
                "显示 API Key"
            },
        )
        .on_click(cx.listener(|this, _event, window, cx| {
            this.show_key = !this.show_key;
            this.api_key
                .update(cx, |state, cx| state.set_masked(!this.show_key, window, cx));
            cx.notify();
        }));
        let key_row = row(
            "API Key",
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(6.0))
                .child(Input::new(&self.api_key).flex_1())
                .child(eye)
                .into_any_element(),
            Some("配置保存在本机。"),
        );

        // 模型行：下拉或手动输入 + 「获取列表」。
        let fetch = widgets::ghost_button("fetch-models", "获取列表")
            .icon(carbon("renew"))
            .disabled(!can_fetch)
            .on_click(cx.listener(|this, _event, window, cx| this.test_connection(window, cx)));
        let model_control: AnyElement = if self.manual {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(div().flex_1().child(Input::new(&self.model_manual)))
                .child(fetch)
                .into_any_element()
        } else {
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap(px(8.0))
                .child(
                    div().flex_1().child(
                        Select::new(&self.model_select)
                            .w_full()
                            .placeholder("请选择模型"),
                    ),
                )
                .child(fetch)
                .into_any_element()
        };
        let manual_link_label = if self.manual {
            "从列表选择"
        } else {
            "手动输入模型 ID"
        };
        let model_row = row(
            "模型",
            div()
                .flex()
                .flex_col()
                .gap(px(4.0))
                .child(model_control)
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap(px(4.0))
                        .child(
                            div()
                                .text_size(px(12.0))
                                .text_color(c.muted)
                                .child(info.hint),
                        )
                        .child(
                            Button::new("toggle-manual")
                                .ghost()
                                .label(manual_link_label)
                                .small()
                                .on_click(cx.listener(|this, _event, window, cx| {
                                    this.manual = !this.manual;
                                    if !this.manual {
                                        this.sync_model_select(window, cx);
                                    }
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element(),
            None,
        );

        let proxy_switch = div()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(8.0))
            .child(
                Switch::new("proxy-enabled")
                    .checked(self.draft.proxy.enabled)
                    .on_change(cx.listener(|this, checked: &bool, _window, cx| {
                        this.draft.proxy.enabled = *checked;
                        cx.notify();
                    })),
            )
            .child(
                div()
                    .text_size(px(13.0))
                    .child(if self.draft.proxy.enabled {
                        "通过 SOCKS5 代理连接"
                    } else {
                        "直接连接服务"
                    }),
            );

        let mut fields = div()
            .flex()
            .flex_col()
            .gap(px(14.0))
            .child(url_row)
            .child(key_row)
            .child(model_row)
            .child(row("网络代理", proxy_switch.into_any_element(), None));
        if self.draft.proxy.enabled {
            fields = fields
                .child(row(
                    "代理地址",
                    div().child(Input::new(&self.proxy_url)).into_any_element(),
                    Some("支持 socks5:// 和 socks5h://，需填写端口。"),
                ))
                .child(row(
                    "用户名",
                    div().child(Input::new(&self.proxy_user)).into_any_element(),
                    None,
                ))
                .child(row(
                    "密码",
                    div().child(Input::new(&self.proxy_pass)).into_any_element(),
                    None,
                ));
        }
        fields
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use huanshu_core::network::Socks5Settings;

    fn service(base_url: &str, model: &str) -> ModelServiceSettings {
        ModelServiceSettings {
            base_url: base_url.into(),
            api_key: String::new(),
            model: model.into(),
            proxy: Socks5Settings::default(),
        }
    }

    fn ok_result(service: &ModelServiceSettings) -> TestResult {
        TestResult {
            signature: service_signature(service),
            ok: true,
            elapsed: 12,
            error: String::new(),
        }
    }

    #[test]
    fn testing_wins_over_everything() {
        let draft = service("http://127.0.0.1:1234", "m");
        let result = ok_result(&draft);
        assert_eq!(
            service_state(true, Some(&result), &draft),
            ServiceState::Testing
        );
    }

    #[test]
    fn matching_signature_reports_ok_or_failed() {
        let draft = service("http://127.0.0.1:1234", "m");
        assert_eq!(
            service_state(false, Some(&ok_result(&draft)), &draft),
            ServiceState::Ok
        );
        let mut failed = ok_result(&draft);
        failed.ok = false;
        failed.error = "boom".into();
        assert_eq!(
            service_state(false, Some(&failed), &draft),
            ServiceState::Failed
        );
    }

    #[test]
    fn stale_signature_falls_back_to_untested_or_unset() {
        let draft = service("http://127.0.0.1:1234", "m");
        let result = ok_result(&draft);
        // 结果回来时草稿已改：签名不符 → 按当前草稿判断。
        let mut edited = draft.clone();
        edited.model = "other".into();
        assert_eq!(
            service_state(false, Some(&result), &edited),
            ServiceState::Untested
        );
        let mut empty = draft.clone();
        empty.model.clear();
        assert_eq!(
            service_state(false, Some(&result), &empty),
            ServiceState::Unset
        );
    }

    #[test]
    fn no_result_yet_uses_field_presence() {
        let draft = service("http://127.0.0.1:1234", "m");
        assert_eq!(service_state(false, None, &draft), ServiceState::Untested);
        let mut no_url = draft.clone();
        no_url.base_url.clear();
        assert_eq!(service_state(false, None, &no_url), ServiceState::Unset);
        let mut no_model = draft.clone();
        no_model.model = "  ".into();
        assert_eq!(service_state(false, None, &no_model), ServiceState::Unset);
    }

    #[test]
    fn service_location_classifies_hosts() {
        assert_eq!(service_location("http://127.0.0.1:1234"), "本地服务");
        assert_eq!(service_location("http://localhost:8080/v1"), "本地服务");
        assert_eq!(service_location("http://[::1]:9000"), "本地服务");
        assert_eq!(service_location("https://api.example.com/v1"), "远程服务");
        assert_eq!(service_location("https://user@api.example.com"), "远程服务");
        assert_eq!(service_location(""), "地址未填写");
        assert_eq!(service_location("not-a-url"), "地址未填写");
        assert_eq!(service_location("http://"), "地址未填写");
    }
}
