# 原生界面功能对照表（Tauri + Vue → GPUI）

原则：**整个界面复刻原版**，可以按 GPUI 的特点调整细节（控件形态、动画），但**关键功能一项都不能丢**。
本表是迁移的唯一清单：每做完一项把状态改掉，并在「依据」里写验证方式（截图名、单测名）。

- 原版截图：`docs/native/reference/`（Tauri 版，沙箱数据，暗·星图主题）
- 原生截图：`docs/native/shell/`（`apps/huanshu-native/tools/shots.ps1` 生成，同一份沙箱数据、同一主题）
- 状态：✅ 已实现　🟡 部分 / 骨架　⬜ 未开始　➖ 有意不做（写明原因）

## 全局

| 功能 | 原版 | 状态 | 说明 / 依据 |
|---|---|---|---|
| 自绘标题栏（品牌、最小化/最大化/关闭、拖动、贴靠） | `TitleBar.vue` | ✅ | `widgets::title_bar`；贴靠与多屏需人工验收 |
| 六个主题、外壳配色 | `style.css` `--studio-*` | ✅ | `theme/tokens.rs`（从运行中的旧应用读出的计算值） |
| GPUI Kit 组件跟随主题（按钮、输入框、对话框） | — | ✅ | `theme::apply` 映射 `colors` 并重派生 `tokens` |
| 主题切换并保存 | `ThemeToggle.vue` | ✅ | `widgets::theme_dots` → `ShellHandle::set_theme`（`app_settings.app_theme`） |
| 界面字体（`--font-interface`） | `typography.ts` | ✅ | `theme::apply`；设置页改「界面」字体即时生效（`pages/settings/appearance.rs`） |
| 首次启动进向导 | `App.vue` `is_first_run` | ✅ | `Shell::new` |
| Carbon 图标 | UnoCSS `i-carbon:*` | ✅ | `icons::carbon`，同一套图标 |
| 品牌图标 | `BrandMark.vue` | ✅ | `assets/brand-mark.svg` |
| 轻提示 / 对话框 | 各处 | ✅ | GPUI Kit `Notification` / `Dialog` / `AlertDialog` |
| 沙箱数据目录 | `HUANSHU_HOME` | ✅ | 真机验证一律用 `target/sandbox-home` |

## 首页 `HomeView.vue` → `pages/home.rs`（样板页）

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 操作栏：项目、主题浮层、导入、设置、新建演示 | ✅ | `shell/home-0.png` |
| 标题区、项目数 | ✅ | |
| 加载骨架、空状态（含新建按钮） | ✅ | |
| 项目卡片：名称、比例徽章（非 16:9）、页数、更新时间 | ✅ | |
| 点卡片进项目 | ✅ | |
| 删除确认（文案一致）并删除 | ✅ | `AlertDialog` → `services::projects::delete_project` |
| 导出对话框：普通 `.keynn` | ✅ | `services::storage::export_project` + 保存对话框 |
| 导出对话框：PDF 文档 + 结果对话框 | ✅ | `export::show_report`（对应 `PdfExportDialog.vue`） |
| 导出对话框：加密导出（密码 + 强度提示） | ⬜ | `PasswordDialog.vue`（encrypt 模式）、`export_project(encrypted, password)` |
| 导入未加密 `.keynn` | ✅ | `services::storage::import_project` |
| 导入加密 `.keynn`（输入密码、错误重试） | ⬜ | `PasswordDialog.vue`（decrypt 模式）、`is_encrypted_file` |
| 卡片入场动画 | ➖ | 视觉细节，GPUI 下不做 |

## 设置 `SettingsView.vue` → `pages/settings.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 左侧五个分区、返回工作台、版本号 | ✅ | |
| 分区标题与说明 | ✅ | |
| 界面主题卡片：点击预览、保存后生效、撤销 | ✅ | `shell/settings-appearance-0.png` |
| 字体：七个角色、可搜索系统字体下拉、统一所有字体、恢复默认 | ✅ | `pages/settings/appearance.rs`（FontPicker 弹层照 `SystemFontPicker.vue`；`fonts::enumerate_system_fonts` 走 `run_bg`）；单测 `set_font_under_unified_fills_every_role` 等 |
| 字体：演示预览 / 界面预览 | ✅ | `appearance.rs::font_preview_card`；界面字体随改随 `theme::apply` |
| 模型服务：三类服务卡片与状态 | ✅ | `service_form.rs::service_state`（签名比对照原版）；单测 `testing_wins_over_everything` 等 |
| 模型服务：地址、Key、模型下拉 / 手动输入 | ✅ | `service_form.rs::ServiceForm`（GPUI Kit Select；`services::settings::{get_settings, save_settings}`） |
| 模型服务：SOCKS5 代理 | ✅ | `ServiceForm` 代理开关 + 地址 / 用户名 / 密码（保存时 core `Socks5Settings::validate` 把关） |
| 模型服务：测试连接（耗时、错误） | ✅ | `list_models`（async → `tasks::run_async`，发起时签名比对，迟到结果作废） |
| 生成设置：修复轮数、并发数 | ✅ | `generation.rs`（0–10 / 1–8，保存前校验文案照原版）；单测 `generation_ranges_match_the_original_rules` |
| 存储：数据目录、媒体目录展示 | ✅ | |
| 存储：图标向量缓存初始化 | ✅ | `storage.rs::init_embeddings` → `services::generation::ensure_icon_embeddings`（async）；状态逻辑照原版 `embeddingReady` |
| 关于与帮助 | ✅ | |
| 底部保存条：状态、撤销、保存 | ✅ | 统一草稿（主题 + 字体 + 模型 + 生成参数）；保存后界面字体立即生效；单测 `config_dirty_tracks_any_component_change` |
| 离开时「有未保存的更改」确认（保存并离开 / 放弃 / 继续编辑） | ✅ | `mod.rs::leave`（Dialog 三按钮，文案逐字；放弃 = 恢复上次保存的主题与字体） |
| `Ctrl+S` 保存 | ✅ | `track_focus` + `on_key_down`（页面根，`mod.rs`） |

## 首次向导 `FirstRunWizard.vue` → `pages/wizard.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 品牌头、四步指示、上一步 / 下一步 / 跳过 | ✅ | |
| 存储目录（默认值来自 core，尊重沙箱） | 🟡 | 只展示；「浏览...」与保存未做（`browseDataDir`） |
| 配置大模型（三类服务）并校验 | ⬜ | `verifyModels`；与设置页共用表单组件 |
| 初始化向量库 | ⬜ | `startEmbeddingInit` |
| 完成页摘要、进入 Studio | 🟡 | `complete_first_run` 已接；目录改动后的保存与「需要重启」未做 |

## 工作流 `WorkflowView.vue` → `pages/workflow/mod.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 加载中 / 加载失败（返回首页） | ✅ | |
| 侧栏：返回、项目名、三步（状态、说明、连线）、主题、设置 | ✅ | |
| 初始步骤（生成中→2，有蓝图→3，否则→1）与可点击规则 | ✅ | `clickable` |
| 项目改名（点名字编辑，回车/失焦保存，Esc 取消） | ⬜ | `saveName` |
| 新建项目的草稿（名称、文稿、粒度、比例）保存与恢复 | ⬜ | `saveStep1Draft` / `persistStep1Meta` |
| 生成完成后自动跳到第 3 步 | ⬜ | 原版 800ms 后切换 |

## 第一步 `Step1Prepare.vue` + `MarkdownWorkbench.vue` → `workflow/step1.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 名称输入、字数 | ✅ | |
| Markdown 编辑器（高亮、行号） | ✅ | GPUI Kit `Editor`（tree-sitter markdown） |
| Markdown 预览 | 🟡 | `TextView::markdown` 已显示；按页切成 P1/P2… 卡片、滚动同步未做（`splitSlides`、`syncPreviewToEditor`） |
| 上传文件、拖入 .md | ⬜ | `openFile` / `onDrop` |
| AI 优化标题 | ⬜ | `services::generation::optimize_markdown_headings` |
| 格式工具（自动标题、清除加粗、删除序号、三种编号） | ⬜ | `autoDetectHeadings` / `clearHeadingBold` / `addHeadingNumbers` / `removeHeadingNumbers` |
| 分页粒度（自动推荐 / 二级 / 三级，各自页数） | ⬜ | `estimateSlides`、`detect_granularity` |
| 幻灯片比例三选一（写回项目） | ✅ | `update_project_aspect_ratio` |
| 素材库（添加、拖入、列表、删除、图注） | ⬜ | `useMediaLibrary.ts`、`services::media` |
| 开始 AI 生成（建项目、保存文稿、切到第 2 步） | ⬜ | `onStep1StartGenerate` |
| 图表围栏的校验提示 | ⬜ | `input.rs` 的解析报错 |

## 第二步 `Step2Generate.vue` + `stores/generation.ts` → `workflow/step2.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 顶部状态与统计（幻灯片数 / 关键事件 / 生成记录） | 🟡 | 只有幻灯片数 |
| 生成流水线与事件 | ⬜ | `services::generation::generate_slides` + `tasks::listen_events`（`CoreEvent`） |
| 整体进度、五个阶段 n/N、查看详情 | ⬜ | `utils/generationProgress.ts` |
| 看板卡片：序号、标题、状态、版式、缩略图 | 🟡 | 状态固定「已完成」；阶段标签未做 |
| 看板只渲染可见卡片 | ⬜ | 现在整副请求；改成虚拟列表 + `wanted` |
| 键盘 ←/→/↑/↓ 选页 | ⬜ | `handleStepKeydown` |
| 详情：大图、标题、状态、版式 | ✅ | |
| 详情：规划页面、生成日志、调试信息 | ⬜ | `get_generation_logs`、`extractPagePlanFromLog` 等 |
| 单页重试、换版式重试 | ⬜ | `retry_generation_page`、`get_retry_layouts`、`RetryLayoutDialog.vue` |
| 生成出错时重试整次生成 | ⬜ | |
| 前往编辑 | ✅ | |

## 第三步 `Step3Editor.vue` + `useSlideEditor.ts` → `workflow/step3.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 工具栏：幻灯片数、页码、导出 PDF、演示 | ✅ | |
| 左侧列表：序号、标题、版式；点击 / ↑↓ 换页 | ✅ | |
| 中间实时预览（Typst 位图、加载 / 失败 / 重试） | ✅ | `slides::SlideImages` |
| 裁切提示 | ✅ | 文案与原版一致 |
| 只发当前页编译（`serial` + `progressive`） | ⬜ | 编辑接上后改成单页牌组 |
| 版式卡片（图标、名称、kind） | ✅ | 来自 `editor_schema` |
| 更换版式（KindPicker，change 模式） | ⬜ | `onKindPicked` |
| 新建 / 删除 / 上下移动幻灯片 | ⬜ | `deleteEditorSlide` / `moveEditorSlide` |
| 字段表单：简单字段（文本 / 多行 / 下拉 / 开关） | 🟡 | 现为只读展示 |
| 列表区、嵌套列表、对象区 | ⬜ | `list_sections` / `object_sections` / `new_item` |
| 图标推荐 | ⬜ | `services::generation::recommend_icons_for_query` |
| 素材插图（主图、图集） | ⬜ | `setPrimaryImage` / `appendGalleryImage` |
| 图表 JSON 框（防抖校验、失焦立即校验、只写回通过的） | ⬜ | `chart_check::chart_problems_json` |
| 高级 JSON 模式（格式化、语义错误、插入素材引用） | ⬜ | `formatEditorJson` / `insertMediaUrl` |
| 应用更改（`Ctrl+S`）与未保存标记 | ⬜ | `services::projects::update_project_blueprints` |

## 演示 `PresentationOverlay.vue` → `present.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 覆盖层（不是新窗口）、打开即全屏 | ✅ | |
| 翻页：←/→/↑/↓/空格/点舞台/两侧按钮、Home/End | ✅ | 按键走焦点，不串页面 |
| Esc：先关全览，再退出 | ✅ | 实测（`clicktest`） |
| 拉伸 / 等比（S）、全屏（F） | ✅ | |
| 全览（O），点缩略图进入 | ✅ | |
| 胶片条、页码、进度条、主题圆点 | ✅ | |
| 当前页先出图，再预取 -1…+3，`max_cached` 10 | ✅ | |
| 快捷键提示（3 秒） | ✅ | |
| 切页动画 | ➖ | 原版有淡入/滑动；GPUI 下先不做 |
| 触摸滑动翻页 | ⬜ | `handleTouchStart` / `handleTouchEnd` |
| 全屏时控制条浮动 | ⬜ | `pres-controls--floating` |
