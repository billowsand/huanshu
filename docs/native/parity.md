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
| 锤子便签材质统一 | 用户提供的锤子便签参考 | ✅ | 2026-10-16：浅木纹、横向金属回形针、元信息上置和页边线；生成看板与编辑字段使用纸卡；组件背景独立于木纹桌面，修复黄色输入框/弹窗；演示与全览铺木纹并添加纸张投影。设计见 `docs/design/notes-native-reference.png`；沙箱窗口核对首页、编辑器、演示、全览；74 项原生测试通过。 |
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
| 操作栏：项目、主题浮层、导入、设置、新建演示 | ✅ | `shell/home-0.png`；审计补齐：主题浮层补「主题」标签、浮层内点击不再误关（`stop_propagation`）、操作栏 44→48px、内容列 750→860/边距 4→24（照原版 CSS `HomeView.vue:381,512-519`） |
| 标题区、项目数 | ✅ | |
| 加载骨架、空状态（含新建按钮） | ✅ | |
| 项目卡片：名称、比例徽章（非 16:9）、页数、更新时间 | ✅ | |
| 点卡片进项目 | ✅ | |
| 删除确认（文案一致）并删除 | ✅ | `AlertDialog` → `services::projects::delete_project` |
| 导出对话框：普通 `.keynn` | ✅ | `services::storage::export_project` + 保存对话框 |
| 导出对话框：PDF 文档 + 结果对话框 | ✅ | `export::show_report` 现在是**三框**（进行中 / 成功 / 失败）：导出前开「正在生成 PDF…」+「首次导出需要扫描系统字体，会稍慢一些。」（首页与第三步都经 `begin_export`），成功后换成结果框（两态标题「PDF 已导出」/「…有几处需要留意」、摘要「文件名 · N 页 · x.x 秒」、占位页按版式中文名分组、裁切页解释、缺图一处一条列页码/引用/原因、主题回落用主题名、无回落时的常驻提示「PDF 版式跟随当前主题。」、按钮「在文件夹中显示」+「完成」），失败换成失败框（错误全文可滚动）；core 与原生共 8 个新单测；剩余差异：`导出中不可关` 与「在文件夹中显示」失败仍用 Kit 轻提示（后者记 ➖）；见 `audit-home.md` 第 4 节 |
| 导出对话框：完整 Typst 工程（zip） | ✅ | 第四项「Typst 工程」（`i-carbon:code`）：`services::export::export_typst_project` 在后台写 zip（模板 + deck.json + 素材 + 预着色图标 + README + 空 `fonts/`），`platform::TYPST_PROJECT` 提供 zip 过滤器。复用 `export.rs` 的三框：进行中「正在打包 Typst 工程…」、成功结果框（摘要「文件名 · N 页 · M 个文件 · x.x 秒」，未安装字体提示、占位页按版式中文名分组、裁切页、缺图一处一条、主题回落、常驻 `typst compile main.typ deck.pdf` 提示）、失败框错误全文；命令行 `--open <id> --export-typst <path>`。core 单测 `project_archive_contains_sources_readme_and_fonts`、slide-typst 集成测试 `tests/project.rs`（解包后用真实目录的 Typst World 编译） |
| 导出对话框：加密导出（密码 + 强度提示） | ✅ | `password_dialog.rs::PasswordDialog`（encrypt 模式，强度条/确认密码/门槛照原版）、`export_project(encrypted, password)`；单测 `encrypt_gate_agrees_with_core_password_validation` 等；审计补齐：面板补「密码至少 8 位，需包含大写字母、小写字母和数字。」提示、两个输入框回车提交、Kit 自带 ✕ 去重 |
| 导入未加密 `.keynn` | ✅ | `services::storage::import_project` |
| 导入加密 `.keynn`（输入密码、错误重试） | ✅ | `PasswordDialog`（decrypt 模式）：`is_encrypted_file` 为真弹密码框，密码错误不关闭、`set_external_error` 重试 |
| 卡片入场动画 | ➖ | 视觉细节，GPUI 下不做 |

## 设置 `SettingsView.vue` → `pages/settings.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 左侧五个分区、返回工作台、版本号 | ✅ | 「返回工作台」照原版 `goBack()` 回来源页（`SettingsView.vue:210-213`）：外壳记下进设置之前的路由（`shell.rs` 的 `Shell.settings_origin` + `ShellHandle::leave_settings`），从工作流侧栏进来就回**那个项目的那个步骤**（`WorkflowPage::show_step` 把步骤同步回路由），没有来源时回首页 |
| 读取设置失败：提示 + 重新加载，不许保存 | ✅ | 原版 252-255（`!ready` 时工作区整块禁用）：`mod.rs::read_settings` / `reload_settings` / `load_error`（失败态只画提示，保存按钮禁用 + `save()` 再挡一次）；core 记失败原因并支持重读（`AppState::{settings_load_error, settings_file_error, reload_settings}`、`services::settings::{read_error, reload_settings}`） |
| 分区标题与说明 | ✅ | |
| 界面主题卡片：点击预览、保存后生效、撤销 | ✅ | `shell/settings-appearance-0.png` |
| 字体：十个角色、可搜索系统字体下拉、统一所有字体、恢复默认 | ✅ | `pages/settings/appearance.rs`（FontPicker 弹层照 `SystemFontPicker.vue`；`fonts::enumerate_system_fonts` 走 `run_bg`）；单测 `set_font_under_unified_fills_every_role` 等；审计补齐：弹层 Enter 应用高亮项、打开自动聚焦搜索框、字体枚举失败/空列表有提示；**弹层的 ↑/↓/Home/End 候选导航 ➖**：表现层，按 GPUI 原生做法——不换 GPUI Kit 的列表组件，因为 `List`/`Select`/`Combobox`/`Command` 把点击与回车合并成同一个确认动作（`list.rs:503-514` 等）、列表上下文不绑 `home`/`end`（`list.rs:26-35`），换过去会破坏原版「点击只高亮、回车才应用」；现状：搜索 ✅、回车应用 ✅、Home/End ❌ |
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
| 离开时「有未保存的更改」确认（保存并离开 / 放弃 / 继续编辑） | ✅ | `mod.rs::leave`（Dialog 三按钮，文案逐字；放弃 = 恢复上次保存的主题与字体）；原版初始焦点在「保存并离开」（`SettingsView.vue:191-201`）归 ➖「表现层，按 GPUI 原生做法」 |
| `Ctrl+S` 保存 | ✅ | `track_focus` + `on_key_down`（页面根，`mod.rs`） |

## 首次向导 `FirstRunWizard.vue` → `pages/wizard.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 品牌头、四步指示（含 skipped / 连线 done 态）、上一步 / 下一步 / 跳过 | ✅ | 显隐逻辑照原版 `wizard-footer` |
| 存储目录（默认值来自 core，尊重沙箱） | ✅ | 可编辑 + `rfd::FileDialog::pick_folder`；联动规则照原版（媒体目录非 `/projects/` 时设为 `<选中>/media`，`\`/`/` 都容忍）；单测 `browsing_data_dir_refreshes_media_dir_unless_it_is_a_projects_dir` |
| 配置大模型（三类服务）并校验 | ✅ | 复用 `settings/service_form.rs::ServiceForm::compact`；「下一步」前依次 `list_models`，失败不翻页；**有意偏离：提前发现模型配置错误**——原版这步其实不校验：`verifyModels`（`FirstRunWizard.vue:119-149`）全仓无调用点，是死代码；原生保留校验（若要对齐原版删掉即可，约 30 行） |
| 初始化向量库 | ✅ | `startEmbeddingInit`：先 `save_settings` 再 `ensure_icon_embeddings`（`run_async`），进度/状态文案逐字 |
| 完成页摘要、进入 Studio | ✅ | 摘要六行 + `dataDirChanged` 重启警告；`finishWizard` 顺序 `save_app_settings` → `save_settings` → `complete_first_run` |

## 工作流 `WorkflowView.vue` → `pages/workflow/mod.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 加载中 / 加载失败（返回首页） | ✅ | |
| 侧栏：返回、项目名、三步（状态、说明、连线）、主题、设置 | ✅ | 有意偏离：顶部是「← 项目」（回上一级），项目名挪到标题栏正中（超过 28 字省略），与三步顶栏同高 64px（`HEADER_H`）。第 2 步说明按 `gen` 四态（出错 / 生成中 `stageLabel` / 已生成 N 张 / 等待开始），生成中圆点里是转圈（`Spinner`）；审计补齐：done 态圆点/连线改成功绿（照 `WorkflowView.vue:703-759`）、可点步骤悬停变亮、说明 truncate、项目名 tooltip |
| 生成状态是应用级 store（离开项目 / 回首页再进，进度接着走） | ✅ | `src/generation.rs::GenerationStore`：`Shell` 创建、`ShellHandle` 发给页面，事件接收常驻在它身上（`tasks::listen_events_in`）；页面来去不影响在跑的生成（原版 pinia store 同）；`ProjectModel` 不再存生成字段；**生成中开别的项目曾崩溃（实体租约重入）已修**：`open_project` 的「store 正为别的项目跑」早退分支改 `defer` + WeakEntity（`f-b1-*` 5 次不崩） |
| 静态状态（别的项目在生成时的本项目） | ✅ | store 只实时持有「发起生成的那个项目」，其他项目的第二步用库里的静态快照：`snapshot_for` 的静态分支用 `StaticCache`（打开项目时经 `run_bg` 读该项目的最近运行 + 日志一次，运行变了才失效，每帧只读缓存）恢复失败页（`progress::statuses_from_logs`）、关键事件 / 生成记录统计、最近 10 条日志、看板条目；失败段与「重试这一页」会出现但按全局规则禁用（`!live` 即禁用，因为静态分支下 store 的重试会排到别的项目的运行上；「重新生成全部」只在 `busy_elsewhere` 时禁用）；6 个新单测（`generation.rs::tests`）；原版对应 `loadLatestRun`+`loadFromJson`（`generation.ts:380-435`） |
| 第二步选中的页码跨步骤保留 | ✅ | 下标存在 `ProjectModel::selected_slide`（`Option<usize>`，照原版 `selectedSlideIndex` 的 null 语义）：`Step2::select` 写、渲染归位（越界/未选回落 `preferred_slide_index`）、整次重新生成前清空；4 个新单测（`resolve_selected`）；原版在 `WorkflowView.vue:500,505` |
| 初始步骤（生成中→2，有蓝图→3，否则→1）与可点击规则 | ✅ | `finish_open`（原版 `onMounted` 顺序在 `GenerationStore::open_project` 里）；第 3 步点击含「没有页在重试」 |
| 项目改名（点名字编辑，回车/失焦保存，Esc 取消） | ✅ | 点标题栏正中的名字改名（`WorkflowPage::title_element`，`occlude()` 挡住拖动区）；`mod.rs::start_rename` / `save_name`（空名不保存、保持编辑态，照原版） |
| 新建项目的草稿（名称、文稿、粒度、比例）保存与恢复 | ✅ | 800ms 防抖自动保存（`WorkflowPage::schedule_autosave` / `persist_draft`，新建落库后 `retarget_route` 对准路由）；meta 存 `app_settings` 的 `project_meta:<id>`（`services::projects::Step1ProjectMeta`，单测 `step1_meta_*`）；待截图对比 |
| 生成完成后自动跳到第 3 步 | ✅ | `sync_advance`：条件从**应用级 store** 读（`state_for` → `GenState`），与原版 watch 的 `[stage, retryingPages.size, 有失败页]` 一样「变了才判断」——`done` + 无重试 + 无失败页 + 用户正停在本项目的第 2 步时，800ms 后再查一遍再跳；计时走 `window.background_executor().timer`（不是 tokio）；**已真机验证**（LM Studio，全部成功的生成完成后未操作自动落在第三步，`target/drive/f-d1-gen125-0.png`） |
| 牌组的唯一写入路径 | ✅ | 第三步的每一处编辑（应用 / 新建 / 删除 / 移动 / 换版式）都只经 `WorkflowPage::commit_deck(deck, active, save, …)`：先写 `ProjectModel`（`deck` + `active_slide`）→ 再写应用级 store（`GenerationStore::set_deck`，只在 store 正持有本项目且不在生成中时写得进去）→ `save` 时整副 JSON 落库；`on_generation_changed` 的镜像比较后相等就不动，不会回环。落库状态由 `ProjectModel.deck_saving` / `deck_save_error` 单点表达（每条出口都复位） |
| 库里停在 `running` 的旧运行（上次进程崩了） | 🟡 | **有意偏离**：原版会一直转圈；这里按「已中断」显示（`running=false`、阶段照 `slideStatus` 的非运行分支）且允许重新生成，**不改库**（`GenerationStore::apply_run`） |

## 第一步 `Step1Prepare.vue` + `MarkdownWorkbench.vue` → `workflow/step1.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 名称输入（原版占位文案）、字数（UTF-16 码元数，与原版 `mdContent.length` 一致） | ✅ | 有意偏离：第一步不再单放名称框（与标题栏改名重复），格式工具提到顶栏原「准备素材」处；名称仍从 H1 自动提取。`ProjectModel::md_len` |
| 草稿保存状态（保存中… / 已保存 / 失败） | ✅ | `WorkflowPage` 800ms 防抖自动保存置位（`DraftSaveState`）；失败原因显示在格式工具行（原版 `draftSaveError`）；待截图对比 |
| Markdown 编辑器（高亮、行号） | ✅ | GPUI Kit `Editor`（tree-sitter markdown） |
| Markdown 预览按页卡片（P1/P2…，`TextView::markdown`） | ✅ | `markdown_tools::split_preview_pages`（auto 先 `detect_granularity`）；空态「预览区将实时显示 Markdown 渲染效果」；**数学公式已渲染**（`formulas.rs` 插件 + `slide_typst::render_formula` Typst 位图，行内/块级、坏公式红字、价格类 `$` 不被吞；外观与原版 KaTeX 不同属有意——与幻灯片/PDF 同一套排版，2026-10-16 真机验证 `target/drive/m2-notes2`、`t2-light2`）；渲染语义差异（原版 `breaks/linkify/typographer/html:false`，GPUI 是标准 CommonMark 且会解析内联 HTML）归 ➖「表现层，按 GPUI 原生做法」；待截图对比 |
| 编辑区 ↔ 预览区滚动同步（按比例） | 🟡 | 仅滚轮双向同步（预览 `ScrollHandle` 精确 ↔ 编辑器 `scroll_offset`/`set_scroll_offset`，均有回声守卫）。**已知偏差（2026-10 审计查证）**：① 编辑器真实最大滚动量无公开 API——`text_bounds()` 是视口矩形不是内容边界（gpui-base `state.rs:644`/`element.rs:3311`），真值在私有 `scroll_size`/`display_map`，只能按「行数 × 行高 − 视口」估计，soft-wrap 折行时中段比例有偏差；「越界探针」能读真值但会让编辑器可见地闪一帧，不采用；② 编辑器内建滚动条拖拽不触发同步（拖拽只调 `ScrollHandle::set_offset` 不通知实体，`state.rs:2616-2624` 只在偏移被夹时才 notify）；③ 预览侧框架根本不画滚动条（gpui-pre `div.rs` 无滚动条代码），无拖拽可挂 |
| 上传文件（rfd `.md`/`.txt`，H1 取名） | ✅ | `open_file` / `load_markdown`；待截图对比 |
| 拖入 .md / .txt（`on_drop` + `ExternalPaths`，释放以加载 Markdown 文件） | ✅ | 拖入加载可用；**已知偏差**：框架无文件拖入悬停钩子（只有 `on_file_drop_exit`/`on_drop`），拖动中的覆盖层高亮显示不出来；扩展名原生收紧为只认 `.md`/`.txt`（原版 `onDrop` 不校验，`Step1Prepare.vue:222-234`，记 ➖）；待截图对比 |
| AI 优化标题（`optimize_markdown_headings`，async） | ✅ | `run_async` + 行内错误（`prepare-inline-error`）；成功 `replace_all` 保留撤销并从 H1 更新名称；迟到结果按模型实体 + 项目 id 作废；待截图对比 |
| 格式工具（自动标题、清除加粗、删除序号、三种编号） | ✅ | `markdown_tools::{auto_detect_headings, clear_heading_bold, remove_heading_numbers, add_heading_numbers}`；`replace_all` 保留撤销历史；快照单测 `markdown_tools.rs` |
| 分页粒度（自动推荐 / 二级 / 三级，各自页数与标题数） | ✅ | `estimate_slides` / `detect_granularity` / `count_exact_headings`；待截图对比 |
| 幻灯片比例三选一（写回项目） | ✅ | `update_project_aspect_ratio` |
| 素材库（添加、拖入、列表、删除） | ✅ | `services::media::{import_media_asset, delete_media_asset, get_project_media}`；新建项目先 `create_project` + `open_project`，落库后 `retarget_route` 对准路由（原版 `router.replace`）；图注编辑原版不在第一步（第三步高级 JSON）；审计补齐：缩略图 68×51 横排、删除按钮只在该缩略图悬停时显示、拖放区虚线框常显、编辑/预览两栏宽比 1.15:0.85（照 `MarkdownWorkbench.vue:395`）；待截图对比 |
| 开始 AI 生成（建项目、保存文稿、切到第 2 步） | ✅ | `WorkflowPage::start_generation`：取消防抖 → 立即落库 → 切第 2 步 → `GenerationStore::generate`（granularity auto 传 None）；「开始 AI 生成」的可用条件即原版 `canGenerate`（**全局**没有生成 / 重试在跑：`store.busy()`，同一时刻全应用只许一次生成或重试——别的项目在跑时这一步也不可用）；第 2 步底栏的「重新生成全部」走的就是这一条路；**生成流程已真机验证**（含生成中回首页再进项目进度接着走、`target/drive/f-b1-reopen-*`） |
| 图表围栏的校验提示 | ✅ | **原版无单独提示**：第一步侧（`Step1Prepare.vue` / `MarkdownWorkbench.vue` / `WorkflowView.vue` / `stores/` / `composables/`）对 `chart` 围栏 0 命中；围栏错误在生成时由 core 抛出（`crates/huanshu-core/src/input.rs:103-239`：JSON 语法、ChartSpec::problems、不在小节内、未闭合等），随生成失败显示在第二步「生成遇到问题」+ `message`（原版 `stores/generation.ts:197-210` → `Step2Generate.vue:415-418`；原生同链路 `generation.rs::on_event` → `step2.rs`）；**已真机验证**（坏 ```chart 围栏开始生成 2 秒失败，第二步显示「```chart 块 JSON 解析失败: EOF while parsing an object…」，`target/drive/d6-error1/2-0.png`） |

## 第二步 `Step2Generate.vue` + `stores/generation.ts` → `workflow/step2.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 顶部状态与统计（幻灯片数 / 关键事件 / 生成记录） | ✅ | 标题四态（生成中「`{stageLabel}...`」/「生成完成 · N 张幻灯片」/「生成遇到问题」/「准备开始」）+ 第二行 `message`；三个统计取 core 的 `slideItems` 条数 / `important_log_count` / `prompt_log_count`（数字用主色）；`shell/project-step2-0.png`、`shell/project-step2-error-0.png` |
| 生成流水线与事件 | ✅ | `GenerationStore`（原版 `stores/generation.ts`，应用级、比页面活得久）+ `services::generation::generate_slides`；五类 `CoreEvent` 都在 store 里落地；单页重试另见下面两行；**生成流程已真机验证**（LM Studio，单轮 4 页规划/7 张牌组 43–51s：看板逐页流转、整体进度、实时点、落库，`target/drive/d1-*`、`f-d1-*`） |
| 整体进度、五个阶段 n/N、查看详情 | ✅ | `huanshu_core::generation_progress::summarize_progress`（快照单测 `crates/huanshu-core/tests/generation_progress.rs`）；展开后的两段说明与四列表格逐字照抄；实测展开：`共 5 页 · 已完成 4 · 进行中 0 · 等待 0 · 失败 1 · 已中断 0`，五阶段 5/5 5/5 4/5 4/5 4/5 |
| 看板卡片：序号、标题、状态、版式、缩略图 | ✅ | 状态与五个阶段标签都调 core（`slide_status` / `page_step_state`），版式名用 `step2_kind_label`（**不是** `editor_schema` 的中文名）；空状态与两种占位文案照原版；`project-step2-error-0.png`（第 3 张红色「生成失败」，阶段停在「布局」之后） |
| 看板只渲染可见卡片 | ✅ | `uniform_list`（一行 = 一行卡片，行高 = 卡片实测高度）+ `SlideImages::set_wanted` = 可见行 + 预取一行；长牌组（60 内容页）打开时请求 6 页（另有详情 1 页），滚到底累计请求 60 页，滚回顶部 0 次新请求 |
| 键盘 ↑/↓ 选页 | ✅ | `track_focus` + `on_key_down`：**只有 ↑/↓**，带修饰键不处理（照原版 `handleStepKeydown`；原表里的 ←/→ 是错的）；进入第 2 步时看板拿到焦点；选中的卡片滚进可见区；实测 ↑/↓ 换页 |
| 详情：大图、标题、状态、版式 | ✅ | 大图只请求选中这一页（详情 `SlideImages` 的 `max_cached = 4`）；审计补齐：详情标题兜底「待生成幻灯片」（卡片兜底仍是「第 N 张」，照原版 `Step2Generate.vue:555`）、详情头状态 chip 中性色（原版 `.sdp-chip` 无色调类） |
| 详情：规划页面、生成日志、调试信息 | ✅ | 「规划页面」（objective / key_points / 结论）/「布局排版」（含「· 已调整布局」与 reason）/「内容结果」三段 + 「查看当前页调试信息」（System Prompt / User Prompt / 模型原始输出 / 解析结果：等宽字体、深色底、可滚动）+「查看最近日志」（`recent_logs`，`{log_stage_label} · #{seq}`）；**已知偏差**：调试文本不可选中复制（gpui 没有可选中文本元素） |
| 单页重试、换版式重试 | ✅ | 失败段 → `RetryLayoutDialog.vue` 对应物（`get_retry_layouts` + `editor_schema` 的 label/desc、默认选中当前版式、关掉后迟到的结果按版本号作废、下拉自动聚焦）→ `GenerationStore::retry_page`（返回 `Result`，被拒时 Notification 提示不再静默）；**成功路径已真机验证**（LM Studio：重试中 → 换版式重试成功、写回牌组与日志、全部通过后底栏恢复「前往编辑」；重试途中离开项目结果仍写回原项目，sqlite 核对，`target/drive/f-b2-*`、`f-b2c-*`）；无 LM Studio 时该页回到「生成失败」并显示连接错误，界面不崩 |
| 生成出错时重试整次生成 | ✅ | 底栏「重新生成全部」走与第一步「开始 AI 生成」同一条路（`WorkflowPage::start_generation`）；实测：点后进入生成中（看板清空、底栏隐藏、面板头出现实时点），连接失败后回到「生成遇到问题」 |
| 前往编辑 | ✅ | 完成态底栏，有页在重试时禁用 |

## 第三步 `Step3Editor.vue` + `useSlideEditor.ts` → `workflow/step3.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 工具栏：幻灯片数、页码、导出 PDF、演示 | ✅ | 拆成**三个栏头**（原版 `.edit-col-head`，同高连成一条）：左 `layers` +「幻灯片」+ 数量徽章 + `reset` 图标（回第 1 步）；中 `view` +「实时预览」+「● 未保存」+「渲染中…」+ `← N / M →`（两端禁用）+「导出 PDF」+「演示」；右 `edit` +「编辑 第 N 张」+ 上移 / 下移 / 删除。截图 `shell/project-step3-0.png`（对照 `reference/project-step3.png`） |
| 左侧列表：序号、标题、版式；点击 / ↑↓ 换页 | ✅ | 行尾垃圾桶**只在悬停该行时显示**（`group` + `group_hover`，原版 `.edit-slide-row:hover .edit-row-del`）、只剩一页时不显示；点击行为实测（`3 / 18`、`4 / 19`）；↑↓ 只在第三步根节点自己有焦点时换页——焦点在输入框里时 ↑↓ 不换页（实测），KindPicker 开着时也不换页（实测） |
| 中间实时预览（Typst 位图、加载 / 失败 / 重试） | ✅ | `slides::SlideImages` + `slide_frame`；舞台在栏头之下取最大比例框、四周 16px，比例按当前页草稿的 `aspect_ratio`（缺省回落项目比例）；实测 32:9 项目舞台明显变宽 |
| 裁切提示 | ✅ | 「内容过多，已缩到最小字号仍放不下，超出部分被裁切」（`warning-alt` 图标），`SlideImages::overflow(0)` 为真时显示 |
| 只发当前页编译（`serial` + `progressive`） | ✅ | 牌组恒为 `[draft]`（下标 0）；草稿变化 **120ms 防抖**（`debounceMs: 120`）后 `set_deck`；换页走 `SlideImages::reset`（清图 + 逐张 `drop_image`，不是重建实体）；`serial` / `progressive` 取 `PreviewOptions::default()`（都是真）；「渲染中…」在 `pending()` 持续 **150ms** 后才显示 |
| 版式卡片（图标、名称、kind） | ✅ | 来自 `editor_schema`；`shell/project-step3-0.png` |
| 更换版式（KindPicker，change 模式） | ✅ | 实测 feature_grid 页 →「更换」→「列表」（section_list）：`章节号 01` / `标题` / `副标题` **保留**，新内容来自 schema 的默认值，当前 kind 高亮；未点「应用更改」前 sqlite 里那一页仍是 `feature_grid`。标题「更换幻灯片类型」/「选择新类型后将自动保留可迁移的公共字段」逐字；截图 `shell/project-step3-kind-picker-0.png`（对照 `reference/step3-kind-picker.png`） |
| 新建 / 删除 / 上下移动幻灯片 | ✅ | 「新建幻灯片」：徽章 18→19、`4 / 19`，**DB 仍 18 页**（新建只改内存，照原版）；新页章节号继承上一页——第 5 页（`section='02'`）之后新建得到 `'02'`，封面之后新建**没有** `section`（两者都用 sqlite 验过，测完还原）；删除 / 上移**立即落库**（sqlite：18→17 页且 `feature_grid` 消失；上移后页序交换） |
| 字段表单：简单字段（文本 / 多行 / 下拉 / 开关） | ✅ | 全按 `editor_schema` 生成：`simple_fields ∩ kind.fields`、两列网格（`textarea` 占整行、`rows` 缺省 3）、`select` 未选中时显示「请选择」（= 空串，可清除）、`boolean` 是自带标签的勾选框（`Checkbox`）；表单表由 `tests/editor_schema.rs` 对着快照锁定 |
| 列表区、嵌套列表、对象区 | ✅ | 18 个版式逐页走查（`tools/drive.ps1` 逐页截图 + 联系表；联系表是未提交的临时产物）+ compare / swot / process 全分辨率逐项核对：区标题 / `empty_text`（删空「列表项」→「还没有列表项」）/ 每项卡片「{label} {n}」+ 上移下移删除 / `string_lists` / `nested_lists`（process 的「阶段步骤」）/ `object_sections`（compare 的左右方案、swot 的四象限）/ `issue_stack` 的 cards 提示按张数二选一（逐字）；`icon_query_keys` 有值的卡片留「图标助手」占位 |
| 图标推荐 | ✅ | 查询词 = 卡片 `icon_query_keys` 各字段值 trim 后空格连接（空不发）；`recommend_icons_for_query(query, 6)`，第一个候选直接写进 `{卡片路径}.icon`，候选小片（预览 + 名字）点击可换；「推荐中…」按钮禁用，按页 + 表单代次 + seq 作废迟到结果；实测（project:2，`target/drive/icon01~icon06`）：候选出现、预览图标随草稿变、未应用不落库；无向量库时走 core 本地关键词兜底（与原版同一份兜底，不会失败卡住）；向量库已初始化时走语义检索（LM Studio embedding，6 候选，`target/drive/f-b3k-recommended-0.png`）；候选小片点击路径 bug（写成 `cards[0].icon.icon`）已修并实测可换（`f-b3k-chip-applied-0.png`）；「推荐中…」文案因请求 1–2s 内返回未截到图 |
| 素材插图（主图、图集） | ✅ | 显示条件照原版（kind 的 fields 含 `images` 或当前页 `image` 非空）；素材条 = 项目 `image` 素材小卡（缩略图同 step1 `media_thumb`、名字、**「设为主图」→ `image`**、**「加入图集」→ 追加 `images` 且去重**）；实测（project:1，`target/drive/g1~g5`）：重复加入不重复、设为主图预览更新、应用后 sqlite 只读核对 `image`/`images` 正确 |
| 图表 JSON 框（防抖校验、失焦立即校验、只写回通过的） | ✅ | 框 = 草稿 `chart` 两空格缩进 JSON，有焦点不被草稿覆盖、换页/换版式重置；语法错立即「JSON 语法错误：{原因}」，合法 150ms 防抖 `chart_problems_json` + `message_zh`（「；」连接多条），**只写回通过校验的 JSON**（未通过预览保持上一次合法图表）；失焦立即校验还在等待的输入；等待时「正在校验…」，否则「type / data / encoding / series，校验通过才会应用到预览」；实测 a–d 全过（project:5，`target/drive/j01~j05`，含粘贴后立即失焦仍生效） |
| 高级 JSON 模式（格式化、语义错误） | ✅ | 「高级 Blueprint JSON」段：`json_text` 随表单改动/换页/换版式/应用刷新（`filter_slide_json` 两空格缩进）；框内输入解析成功 → 草稿=解析结果**不过滤**、重建表单，失败 → 「第 N 行：{原因}」、草稿不动；**应用写回 `json_text` 解析出来的那一页**（手加字段应用后仍在，sqlite 实测）；有语法错应用禁用、Ctrl+S 无效；「格式化」能解析才规整；行号代码编辑器、Tab 插两个空格；语义错误（无语法错时显示，`warning` 图标）= core `slide_edit::semantic_errors`，图表语义错误 150ms 防抖只认最新；原版「插入素材引用」模板里没有触发入口，未做；实测 a–f 全过（project:2，`target/drive/adv02~adv12`）；原版出错行号标红（`.ej-line-num--error`）归 ➖「表现层，按 GPUI 原生做法」（GPUI Kit Editor 行号栏无接口，错误文案里的「第 N 行：」仍在） |
| 应用更改（`Ctrl+S`）与未保存标记 | ✅ | 三态逐字：`保存中…`（`renew`）/ `应用更改 (Ctrl+S)`（`save`）/ `已保存`（`checkmark`），保存失败时按钮下方是 `.ej-error` 样式的错误行；**Ctrl+S 在第三步任何位置都生效**（有意改进：原版只在高级 JSON 框里）；实测 sqlite 前后：`Auto Slidev Layout Blueprint Test Deck` → `… LOOP4`，杀进程重进项目仍是新值；`filter_slide_json` 的有意修改保住 `aspect_ratio`（32:9 项目改封面并应用后该页仍是 `ratio_32x9`，第 2 步看板卡片仍是 32:9）；吸底：原版按钮靠 `position:sticky`（`WorkflowView.vue:2061-2065`）吸在右栏底，GPUI 没有 sticky，改成**右栏拆两段**——可滚动的表单区 `#editor-panel` + 固定在栏底的操作区（`step3.rs::render_actions`，含按钮 / 生成中提示 / 保存失败行），长表单下按钮始终可见；**有意偏离**：短表单时操作区落在栏底（原版按钮紧跟内容之后） |
| 生成中只读 | ✅ | **有意偏离**：`WorkflowPage::editing_locked` 只在**本项目**正在生成/重试（store 正持有本项目且 `busy()`）时为真——应用/新建/删除/移动/换版式与全部输入框禁用、保存按钮旁显示「生成进行中，完成后可编辑」（原版没挡）；**别的项目**在生成时本项目第三步照常可编辑（`set_deck` 写不进时模型是唯一状态，`commit_deck` 落库写本项目自己的行，不碰生成中的项目）；core 侧生成已按运行所属项目解耦（`get_generation_deck`，见 CLAUDE.md），跨项目生成不失败、不写错牌组；**已真机验证**（LM Studio：A 生成中 A 第三步只读、B 第三步可编辑且改动落库不被覆盖，sqlite 核对，`target/drive/f-d3-*`、`d3-*`） |

## 演示 `PresentationOverlay.vue` → `present.rs`

| 功能 | 状态 | 说明 / 依据 |
|---|---|---|
| 覆盖层（不是新窗口）、打开即全屏 | ✅ | |
| 翻页：←/→/↑/↓/空格/点舞台/两侧按钮、Home/End | ✅ | 按键走焦点，不串页面 |
| Esc：先关全览；全屏时只退全屏、非全屏才退出 | ✅ | 实测（`target/drive/e1-e4`：全屏 Esc 退回窗口态演示仍在，再 Esc 退出回第三步）；语义照原版 `PresentationOverlay.vue:311-316` |
| 拉伸 / 等比（S）、全屏（F） | ✅ | |
| 全览（O），点缩略图进入 | ✅ | 缩略图独立投影，选中边框与纸张投影分层；六主题随配色切换。 |
| 胶片条、页码、进度条、主题圆点 | ✅ | |
| 当前页先出图，再预取 -1…+3，`max_cached` 10 | ✅ | |
| 快捷键提示（3 秒） | ✅ | |
| 切页动画 | ➖ | 原版有淡入/滑动；GPUI 下先不做 |
| 触摸滑动翻页 | ✅ | `on_swipe` + `Swipe` 状态机（`ScrollWheelEvent.touch_phase` 累计，\|dx\|>\|dy\| 且 \|dx\|>50 翻页，左滑下一页；`did_swipe` 吞掉随后的舞台点击）；鼠标滚轮不触发；5 个单测。**真机触摸未验证**（本机无触摸屏，方向符号与合成点击需人工验收）；`target/drive/w1-w3`（滚轮不翻页） |
| 全屏时控制条浮动 | ✅ | 全屏时控制条不再占位、叠在舞台底部：底色 72%、上边框 45% 不透明，默认透明下移 8px、悬停（只挂控制条自己）淡入上移 0.3s（`with_animation` 两端态，悬停标志拼进元素 id；`backdrop-filter` 模糊省略）；截图 `shell/present-full-0.png` / `present-full-hover-0.png`；非全屏照旧 flex 底部占位 |
| 全屏铺满 / 非全屏留白 | ✅ | 全屏时外壳不画标题栏，舞台可用区 = 整个屏幕；非全屏 = 标题栏下的覆盖层 − 96/88。2026-10-16：留白改为主题底纹，琥珀疏网格、星图星点坐标、终端·墨扫描线、终端·纸点阵、报刊分栏与钴蓝线、便签木纹；舞台和全览共用 `theme/presentation.rs`。 |

- 字体覆盖复查：新增编辑器、数学公式与兜底字体；便签主题遵守字体设置，代码/日志与富文本同步组件主题，窗口根层覆盖弹层回退，幻灯片/公式/PDF 共用设置。见 [字体覆盖表](fonts.md)。
