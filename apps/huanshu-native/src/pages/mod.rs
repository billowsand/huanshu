//! 页面。每个页面一个模块、一个实体，结构对照原版 `src/views/`：
//!
//! | 原版                          | 这里                     | 状态（详见 `docs/native/parity.md`） |
//! |-------------------------------|--------------------------|---------------------------------------|
//! | `HomeView.vue`                | [`home`]                 | 样板：完整实现（加密导入导出待做）    |
//! | `SettingsView.vue`            | [`settings`]             | 完整实现（字体 / 模型服务 / 生成 / 存储 / 保存条） |
//! | `FirstRunWizard.vue`          | [`wizard`]               | 骨架：步骤条与翻页，表单待做          |
//! | `WorkflowView.vue`            | [`workflow`]             | 骨架：侧栏与三步切换可用              |
//! | `steps/Step1Prepare.vue`      | [`workflow::step1`]      | 骨架：分区占位                        |
//! | `steps/Step2Generate.vue`     | [`workflow::step2`]      | 骨架：看板与详情可看，生成流程待做    |
//! | `steps/Step3Editor.vue`       | [`workflow::step3`]      | 骨架：列表与预览可用，表单待做        |
//!
//! 写新页面的套路（照 `home.rs`）：
//! 1. 结构体里放 `ShellHandle` 与页面状态；数据经 `huanshu_core::services`，耗时的走 `tasks::run_bg`。
//! 2. `render` 里只用 `cx.studio()` 的颜色与 `widgets` 的部件，尺寸按原版 CSS 的逻辑像素抄。
//! 3. 用到幻灯片位图就用 `slides::SlideImages`，并在 `release` 里交还。
//! 4. 文案与原版逐字一致（截图对照 `docs/native/reference/`）。

pub mod home;
pub mod settings;
pub mod wizard;
pub mod workflow;
