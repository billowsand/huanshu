# 逐页审计：首次向导 + App 外壳 / 路由

本轮范围（原版 → 原生，全部逐字逐段读过）：

| 原版 | 原生 | 行数 |
|---|---|---|
| `src/views/FirstRunWizard.vue` | `apps/huanshu-native/src/pages/wizard.rs` | 976 → 901 |
| `src/App.vue` | `apps/huanshu-native/src/shell.rs`（外壳）+ `src/app.rs`（状态） | 48 → 251 / 140 |
| `src/main.ts` | `apps/huanshu-native/src/router.rs` + `src/main.rs` + `src/cli.rs` | 39 → 149 / 112 / 101 |

对照截图：`docs/native/reference/wizard-1..4.png`（原版）、`docs/native/shell/wizard-0.png`（原生，同沙箱、同主题）。

**做法**：每个原版文件从上到下列出用户能看到/能操作的东西（文案、按钮、输入、提示、禁用与显示条件、
快捷键、悬停/拖放、确认框、轻提示），再在 `<script>` 里逐个列出副作用（调了哪个命令、写了什么存储、
何时触发）。「原版没有」与 ➖ 的结论一律贴原版行号。

状态：✅ 一致 ／ 🔧 本轮已修 ／ ⚠️ 有差距未修 ／ ➖ 有意不做（附原因）。
主题相关的纯视觉细节（阴影、渐变、毛玻璃、动画）不逐条列，每个文件归一行「视觉细节」。
下表的「原生位置」行号是改动后的最终状态（`rustfmt` 已跑）。

> 原版截图说明：`reference/wizard-3.png` 与 `wizard-4.png` 都是第 4 步（第 3 步「初始化向量库」没有截图），
> 第 3 步只能按代码逐条对照。

---

## 1. `FirstRunWizard.vue` → `pages/wizard.rs`

### 1.1 可见项与可操作项

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 卡片（居中、宽 560；第 2 步 960） | FirstRunWizard.vue:218-222, 599-612 | wizard.rs:795-802 `Render` | ✅ | 宽度逐字（560 / 960） |
| 卡片内边距 2.5rem、圆角 16px | :599-609 | wizard.rs:795-799 `Render` | 🔧 | 原为 `widgets::panel` 的 32px / 10px；本轮改 40px / 16px |
| 品牌徽章（56×56、圆角 18、28px 品牌图） | :224-227, 628-644 | wizard.rs:815 `widgets::brand_chip(56,18,28)` | ✅ | 渐变底与阴影见「1.4 视觉细节」 |
| 「HUANSHU STUDIO」小标（主色） | :229, 652-657 | wizard.rs:816-828 | ✅ | 字距按视觉细节省略 |
| 「幻述」品牌名 | :230, 659-664 | wizard.rs:827 | ✅ | |
| 页标题「首次启动配置」 | :233, 666-670 | wizard.rs:830 | ✅ | |
| 副标题「完成以下设置后，即可开始生成和演示你的幻灯片」 | :234-236 | wizard.rs:831-835 | ✅ | 逐字 |
| 四步指示：圆点 1–4（32px、2px 边、数字） | :240-283, 686-699 | wizard.rs:432-487 `Render` | ✅ | 尺寸与编号一致 |
| 圆点 active 态 | :242-245, 701-705 | wizard.rs:457-461 | 🔧 | 原版 CSS 里 `.skipped`（:713）排在 `.active`（:701）/`.done`（:707）之后，同权重后者赢；原生原先让 active 压过 skipped。第 3 步点「跳过（可在设置中后续初始化）」→ 第 4 步 →「上一步」回到第 3 步即可复现（原版该圆点是灰的）。本轮改成 skipped 优先 |
| 圆点 done 态 | :244, 255, 268, 707-711 | wizard.rs:462-466 | ✅ | |
| 圆点 skipped 态（第 2 步跳过 / 第 3 步跳过） | :255, 269, 713-717 | wizard.rs:454-456 | ✅ | 条件 `skipLlm` / `skipEmbedding \|\| skipLlm` 逐条照抄 |
| 步骤连线（48×2）与 done 态 | :249, 260-263, 274-277, 719-728 | wizard.rs:473-485 | ✅ | 三段条件逐条对应：`step>1` / `step>2 \|\| skipLlm` / `step>3 \|\| skipLlm` |
| 第 1 步标题「选择存储目录」 | :288 | wizard.rs:710 | ✅ | 标题值取自 709-720 的匹配表 |
| 第 1 步说明「设置数据库和多媒体文件的存储位置」 | :289 | wizard.rs:710 | ✅ | 逐字 |
| 「数据目录」标签 | :293 | wizard.rs `dir_field`:248 | 🔧 | 原版 `.field label` 不设颜色（:760-763），继承 `--studio-text`（style.css:194-196 的 `[data-theme]`）；原生原为 muted，截图上比原版暗。已改为 `c.text` |
| 数据目录输入框（原版 `readonly`） | :295-299（:296 `readonly`） | wizard.rs `dir_field`:255 | ➖ | 有意：原版只能经「浏览...」修改；原生放开为可编辑（parity.md:68 记为「可编辑」）。多一种输入方式，联动规则不变 |
| 「浏览...」按钮（选目录） | :300-305 | wizard.rs `dir_field`:256-264 → `browse_data_dir`:207 | ✅ | rfd 系统目录对话框 |
| 数据目录提示「存放数据库和项目配置文件」 | :307, 780-784 | wizard.rs `dir_field`:266 | ✅ | muted，与原版一致 |
| 「媒体目录」标签 / 输入框 / 「浏览...」/ 提示 | :310-326 | wizard.rs `dir_field`:248-266 / `browse_media_dir`:222 | ✅ | 与数据目录同构（输入框同样由 readonly 放开为可编辑，见上） |
| 第 2 步标题「配置大模型」 | :332 | wizard.rs:711-714 | ✅ | |
| 第 2 步说明「设置 AI 生成所需的模型服务（可跳过，仅播放幻灯片时无需配置）」 | :333-335 | wizard.rs:711-714 | ✅ | 逐字 |
| 跳过提示条（information 图标 +「跳过此步骤仍可使用幻灯片播放功能，但无法生成新幻灯片」） | :337-340 | wizard.rs:532-552 | ✅ | 逐字 |
| 三个面板标题：大语言模型 / 向量模型 / 多模态模型 | :344, 376, 408 | wizard.rs:515-531（`ModelTarget::ALL`） | ✅ | 顺序一致 |
| 面板字段「API 地址」+ 占位 `http://127.0.0.1:1234` | :345-350, 377-382, 409-414 | `service_form.rs::ServiceForm::compact`（service_form.rs:534-538） | ✅ | 复用设置页表单，占位逐字 |
| 面板字段「API Key (本地留空)」+ 密码框 + 占位 `sk-...` | :352-365, 384-397, 416-429 | service_form.rs:539-550 | ➖ | 表现层，按 GPUI 原生做法（原生是全角「（本地留空）」、且多一个「显示/隐藏 API Key」眼睛按钮，都在 `service_form.rs`） |
| 面板字段「模型」+ 占位（各服务默认模型名） | :366-372, 398-404, 430-436 | service_form.rs:551-557 | ✅ | 占位 = 草稿模型名，清空输入后可见（与原版同） |
| 第 2 步错误提示块（`.error-msg`，`v-if="modelError"`） | :440, 911-919 | wizard.rs:855-868 | ✅ | 文案、配色、圆角一致 |
| 错误提示只在第 2 步渲染 | :440（`v-if` 在第 2 步的 `div` 内，第 3、4 步没有） | wizard.rs:855-868（卡片级） | ➖ | 有意：第 3 步的失败原版另有 `embeddingStatus`（:173）兜底；第 4 步 `finishWizard` 失败原版**完全没有可见反馈**（只有 :212 写 `modelError`）。原生在所有步骤都显示，避免「进入 Studio」点了没反应 |
| 第 3 步标题「初始化向量库」 | :445 | wizard.rs:715-718 | ✅ | |
| 第 3 步说明「正在为图标语义匹配生成向量索引（需要一些时间）」 | :446-448 | wizard.rs:715-718 | ✅ | 逐字 |
| 进度条（8px 高、fill 宽 = 进度%） | :450-456 | wizard.rs:557-572 | ✅ | 进度只有 0/100 两态，与原版相同（原版也只在 :170 置 100） |
| 进度状态文字 | :457 | wizard.rs:573-581 | ✅ | |
| 「开始初始化」按钮（初始化中禁用） | :460-467 | wizard.rs:583-596 | ✅ | 禁用条件 = `isInitializing`，一致 |
| 「开始初始化」按钮的图标：**仅初始化中**显示 `renew` | :465（`v-if="isInitializing"`） | wizard.rs:591 | 🔧 | 原生原先闲置时还画一个 `data-base` 图标（原版没有）→ 改为仅初始化中显示 `renew`（旋转动画见视觉细节） |
| 「跳过（可在设置中后续初始化）」整宽按钮 | :469-477 | wizard.rs:598-605 | ✅ | 文案逐字（含全角括号，:476）；行为 = `skipEmbedding=true; step=4` |
| 第 4 步完成徽章（88×88、圆角 28、52px 品牌图） | :482-486, 864-880 | wizard.rs:644 | ✅ | 渐变与阴影见视觉细节 |
| 「配置完成！」 | :487 | wizard.rs:719 | ✅ | |
| 摘要六行：数据目录 / 媒体目录 / 大模型 / 向量模型 / 多模态模型 / 向量库 | :489-522 | wizard.rs:609-662 | ✅ | 取值规则逐条一致：`skipLlm ? 未配置`、`skipEmbedding \|\| skipLlm ? 未初始化 : 已就绪` |
| 摘要值单行省略、最大宽 280px | :902-909 | wizard.rs:617 | 🔧 | 原生原为 340px → 改 280px |
| 重启警告（warning 图标 +「需要重启应用」+ 说明段），显隐 = `dataDirChanged` | :524-532, 65-69 | wizard.rs:664-702 | ✅ | 与 core 启动时的目录比较 |
| 底栏「上一步」+ `arrow-left`（第 2 步起显示） | :536-542 | wizard.rs:733-737 | ✅ | 图标在文字之前，与原版一致 |
| 底栏第 1 步「下一步 →」（`canProceedStep1` 为真才出现） | :544-550 | wizard.rs:741-746 | 🔧 | 原生箭头画在文字前（GPUI 的 `Button::icon` 固定在前，gpui-component `button.rs:766-777`）→ 改用 `.child()` 追加，箭头落在文字之后 |
| 底栏第 2 步「跳过」（永远显示） | :551-561 | wizard.rs:749-751 | ✅ | `skipLlm=skipEmbedding=true → step 4`（:79-81）一致 |
| 底栏第 2 步「下一步 →」（`canProceedStep2` 为真才出现） | :562-568 | wizard.rs:753-761 | 🔧 | 箭头位置同上；另原生在此按钮上加了「校验中禁用」，涉及校验本身 → 见第 6 节（有意偏离：提前发现模型配置错误） |
| 底栏第 3 步「下一步 →」（`!skipEmbedding && progress<100` 时禁用） | :569-576 | wizard.rs:764-772 | 🔧 | 箭头位置同上；禁用条件逐字 |
| 底栏第 4 步「进入 Studio →」（文案固定、pulse 动画） | :577-583 | wizard.rs:775-782 | 🔧 | 原生原先提交期间把文案改成「进入中…」→ 改回固定「进入 Studio」；提交中禁用（防重复提交）保留；pulse 动画见视觉细节 |
| 第 2 步三列网格；`<700px` 时单列 | :342, 799-803, 823-827 | wizard.rs:553（flex 三列） | ✅ | 原版媒体查询在桌面窗口下永不触发（主窗口最小宽 1080，main.rs:74） |
| 滚动：卡片限高 `calc(100vh - 4rem)`、步骤体内部滚动（页头/底栏保持可见） | :608, 730-734 | wizard.rs:786-793（整页滚动） | ➖ | 有意：窗口最小 680 高时各步内容都放得下，整页滚动更简单；要严格一致需给卡片加 `max_height` + 步骤体 `overflow_y_scroll` |
| 快捷键：原版没有（文件内无 `@keydown` / `@keyup`，grep 0 命中） | :217-587 | — | ✅ | 原生也没有页面快捷键 |
| 悬停 / 拖放 / 确认对话框 / 轻提示：原版没有区分（无 `on_drop`/`drag`/`Notification`/`confirm`，grep 0 命中） | :217-587 | — | ✅ | 文件里只有 `@keyframes pulse-glow`（:958）、`spin`（:971）两处动画，以及按钮/输入的 CSS 悬停（:315-323, 346-350） |

### 1.2 `<script>` 副作用

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 挂载时算默认目录 `localDataDir()/auto-slidev-studio`（+ `/media`），失败回落 `~/.local/share/auto-slidev-studio` | :36-50 | `WizardPage::new`:63-75（core `storage::get_app_settings`） | ➖ | 有意：原版不认 `HUANSHU_HOME`（CLAUDE.md:74），沙箱里会指回真实目录；原生取 core 的目录，两处默认值在未设环境变量时相同 |
| 初始草稿：三类服务默认地址 `http://127.0.0.1:1234`、模型 `qwen/qwen3.5-9b` / `text-embedding-bge-m3` / `qwen/qwen2.5-vl-7b-instruct` | :17-25 | `WizardPage::new`:75-104（core `settings::get_settings`） | ✅ | 默认值在 core 里，同一批；向导显示当前设置（原版显示硬编码默认值，首次运行时两者相同） |
| 页面状态：`currentStep` / `dataDir` / `mediaDir` / 三组服务 / 进度 / `isInitializing` / `skipLlm` / `skipEmbedding` / `modelError` | :14-34 | `WizardPage` 字段:39-60 | ✅ | 一一对应 |
| 派生 `canProceedStep1` = 两个目录都非空 | :52-54 | `can_proceed_step1`:142 | ✅ | |
| 派生 `canProceedStep2` = 跳过，或三个服务的**地址与模型**都非空（不看 API Key） | :55-64 | `can_proceed_step2`:147-156 | ✅ | 原生按 `draft.base_url` / `draft.model` 判空，同 |
| 派生 `dataDirChanged`（与默认目录比较） | :65-69 | `data_dir_changed`:158 | ✅ | 比较对象是 core 的初始目录（见上一行的有意差异） |
| 派生 `embeddingsReady` | :70-75 | `embeddings_ready`:164（单测 `embeddings_ready_requires_full_init_flow`:891） | ✅ | `!skipLlm && !skipEmbedding && 进度>=100` |
| `nextStep()`：第 1 步可进 →（`skipLlm` ? `skipEmbedding=true` 且直接第 4 步 : 第 2 步）；第 2 步可进 → 第 3 步；第 3 步 → 第 4 步 | :77-90 | `next_step`:168-180 | ✅ | 分支逐条一致 |
| `prevStep()`：第 2→1、第 3→2、第 4→（`skipLlm` ? 1 : 3） | :92-100 | `prev_step`:182-196 | ✅ | |
| `browseDataDir()`：目录对话框；选中后 `dataDir = 选中`；媒体目录不含 `/projects/` 时设为 `<选中>/media` | :102-110 | `browse_data_dir`:207 + `suggest_media_dir`:26（单测 `browsing_data_dir_refreshes_media_dir_unless_it_is_a_projects_dir`:879） | ✅ | `\` 与 `/` 都容忍 |
| `browseMediaDir()` | :112-117 | `browse_media_dir`:222 | ✅ | |
| `verifyModels()`：依次 `list_models(llm/embedding/multimodal)`，任一失败写 `modelError` | :119-149 | `verify_models`:271-306 | ➖ | **有意偏离：提前发现模型配置错误**——原版**没有任何调用点**（全仓 grep 只有 :119 的定义与函数体内的三处 `invoke`），第 2 步点「下一步」不校验、直接进第 3 步；原生把它接在「下一步」前，失败不翻页（并在校验期间禁用「下一步」）。这是有意保留的偏离，不是未完成的差距 |
| `startEmbeddingInit()`：置 `isInitializing`、进度 0、状态「正在初始化图标向量库...」→ 写三类服务到 config → `config.save()` → `invoke("ensure_icon_embeddings")` → 进度 100 /「初始化完成！」；失败 →「初始化失败: {e}」+ `modelError`；`finally` 复位 `isInitializing` | :151-178 | `start_embedding_init`:317-364 | ✅ | 原生：`get_settings` → 覆盖三类服务 → `save_settings` → `ensure_icon_embeddings`（`run_async`）；`initializing` 在成功/失败两条出口都复位（wizard.rs:346-358） |
| `finishWizard()`：`save_app_settings`（data_dir、media_dir、llm_configured、embeddings_ready、initialized_embedding_model）→ 未跳过时写三类服务并 `config.save()` → `complete_first_run(llmConfigured)` → `router.push("/")`；失败 → `modelError` | :180-214 | `finish_wizard`:366-425 | ✅ | 顺序一致（`run_bg` 里做三步：读-改-写 `app_settings` → `save_settings` → `complete_first_run`）；`finishing` 在两条出口复位，成功回调 `navigate(Home)` |
| `initialized_embedding_model` 的取值：`!skipLlm && !skipEmbedding ? 模型名 : ""` | :188-191 | `finish_wizard`:378-386 | ✅ | |
| 离开页面：原版无 `onUnmounted`（可删项：无） | :36-214（无） | `shell.rs::release_page`:166-171（Wizard 不持有位图） | ✅ | 向导没有需要释放的资源 |

### 1.3 原版有、原生有意不做（汇总，依据见上表）

- 目录输入框 `readonly`（:296, :314）：原生放开为可编辑。
- 错误提示只在第 2 步可见（:440）：原生所有步骤都可见（第 4 步失败才有反馈）。
- 卡片限高 + 步骤体内部滚动（:608, 730-734）：原生整页滚动。
- 目录默认值取自前端 `localDataDir()`（:36-50）：原生取 core 的目录（尊重沙箱）。

### 1.4 视觉细节

- **做了**：卡片底色/边框、圆点三态配色、进度条配色、警告与错误块的底/边、图标（Carbon 同款）。
- **按 GPUI 省略**：品牌徽章与完成徽章的径向渐变 + 大阴影（`brand_chip` 只画纯色底 + 细边，widgets.rs:26-38）；
  「进入 Studio」的 `pulse-glow` 呼吸动画（:954-956）；初始化中 `renew` 的旋转动画（:968-970）；
  进度条宽度过渡、圆点状态过渡、按钮过渡（:698, 723, 845, 309）；`letter-spacing`（:655, 662）。
- **未逐条对齐的度量**：字号整体比原版小 1–2px（原版 `0.875rem`=14px → 原生 13px、`1.25rem`=20 → 19、
  `1.5rem`=24 → 22），这是全工程统一的换算，本轮只改了卡片内边距与圆角、摘要值宽度这两个明显的；
  外层留白原版 2rem=32px、原生 `py(px(36.0))`。GPUI Kit 的 ghost 按钮文字色比原版 `.btn-ghost`（:343 muted）更亮，
  按钮圆角/内边距由组件决定（widgets.rs 范围）。

---

## 2. `App.vue` → `shell.rs`（外壳）+ `app.rs`（状态加载）

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| `<TitleBar />` 自绘标题栏（品牌、最小化/最大化/关闭、拖动、贴靠） | App.vue:35 | `shell.rs`:239 `widgets::title_bar`（widgets.rs:42-58） | ✅ | parity.md:14；`TitleBar.vue` 本身不在本轮三个文件范围内 |
| 页面容器 `.app-body`：顶部让出标题栏高度、`height:100vh`、`overflow:hidden` | :36-38, 42-47 | `shell.rs`:231-249（flex 列：标题栏 + `flex_1 min_h_0` 内容） | ✅ | |
| 界面字体随设置即时生效（`watch(fonts, applyFonts, deep/immediate/sync)`） | :9, 16 | `main.rs`:54-55（启动）+ `shell.rs::set_theme` / 设置页保存后 `theme::apply` | ✅ | parity.md:18 |
| 挂载时并行 `appSettings.load()` 与 `editorSchema.load()`（后者失败只 `console.error`） | :18-22 | `app::load`:19-31（同步读 core 状态）；版式表由用到它的页面按需从 core 取 | ✅ | 原生不做全局缓存：第三步与重试弹窗各自读 `editor_schema` |
| `is_first_run` 为真 → `router.push('/wizard')`；命令失败只 `console.error`（停在首页） | :23-30 | `Shell::new`:115-121（`storage::is_first_run(...).unwrap_or(false)`） | ✅ | 原版先渲染首页再跳（有一帧首页）；原生建页前决定路由，无闪烁 |
| 路由出口 `<RouterView />` | :37 | `shell.rs`:225-230（`Page` 枚举选当前页实体） | ✅ | 换页同时 `close_presentation` + `release_page`（:151-158） |
| 其余可见元素：没有（文件只有标题栏 + 容器） | :34-39（全文件 48 行） | — | ✅ | |

**副作用**：原版 `appSettings.load()` / `editorSchema.load()` / `is_first_run` 三个 invoke → 原生
`app::load()`（`AppState::load()` 读 core，单测 `state_load_follows_the_sandbox`:127）、`shell::interface_font`、
`Shell::new` 的 `storage::is_first_run`。原版 `editorSchema.load()` 失败会被 `catch` 吞掉继续启动（:21），
原生没有这一步（页面按需读，失败各自显示）。

**视觉细节**：`.app-body` 的 `padding-top: var(--titlebar-height)` 与原生 flex 布局等价；背景色、字体、
基础字号来自 `theme::apply` + `cx.studio()`（shell.rs:235-238）。

---

## 3. `main.ts` → `router.rs` + `main.rs` + `cli.rs`

| 项 | 原版位置（文件:行） | 原生位置（文件:函数） | 状态 | 备注 |
|---|---|---|---|---|
| 主题：读 `localStorage['app-theme']`，未知回落 `DEFAULT_THEME`，写到 `documentElement[data-theme]` | main.ts:19-21 | `main.rs`:41-55（core `settings::get_app_theme` + `themes::theme_or_default` → `theme::apply`） | ➖ | 有意：主题存在 core 的 `app_settings.app_theme` 键（CLAUDE.md:72、parity.md:17），与 Tauri 版（WebView localStorage）暂不同步；`--theme` 开关会写回设置 |
| 五条路由：`/`、`/new`、`/project/:id`、`/settings`、`/wizard` | :23-32 | `router.rs` `Route` 枚举 + `Route::parse`:82-118（`--route`）；`Shell::new` 选择首页 | ✅ | 一一对应；`/project/:id` → `Route::Project{ id: Some(..) }`，`/new` → `id: None`；设置页另有五个分区（`settings:<tab>`） |
| hash history（`createWebHashHistory`） | :24 | — | ➖ | 有意：原生窗口没有地址栏/URL；脚本化跳页由 `--route` 代替（README 已有开关表） |
| 全局 store（`createPinia`） | :34-37 | 应用级状态：`generation::GenerationStore`（`shell.rs`:122-126，经 `ShellHandle` 下发）+ 各页模型 | ✅ | parity.md:79 |
| 挂载 `#app` | :39 | `main.rs`:81-84 `open_window` → `Shell::new` | ✅ | |
| 主题样式表的引入（`style.css` + 四个主题 CSS） | :9-17 | `theme/tokens.rs` + `theme::apply` | ✅ | parity.md:15 |
| 命令行开关（`--route` / `--theme` / `--quit-after` / `--open --export-pdf`） | 原版没有（main.ts 无参数解析，全文件 39 行） | `cli.rs`:26-50 + `main.rs`:33-35, 63-71 | ➖ | 有意新增：脚本化截图/导出用（README 的开关表），不影响界面行为 |
| 窗口尺寸与最小尺寸（1440×900、最小 1080×680、居中） | 原版在 `src-tauri/tauri.conf.json`（不在本轮三个文件内） | `main.rs`:73-79 | ✅ | 本轮不审计 `tauri.conf.json` |

**副作用**：原版 `localStorage.getItem` + `documentElement.setAttribute`（main.ts:19-21）、`app.use/mount`
（:37-39）→ 原生没有 DOM，对应 `theme::apply` 与 `open_window`。原生多一个启动预热
`services::export::renderer()`（`main.rs`:57-61，后台扫描系统字体，为了第一张缩略图不等 0.5 秒）。

**视觉细节**：无（本文件不含界面元素）。

---

## 4. 统计

| 状态 | 条数 |
|---|---|
| ✅ 一致 | 63 |
| 🔧 本轮已修 | 9 |
| ⚠️ 有差距未修 | 0 |
| ➖ 有意不做 | 9 |
| 合计 | 81 |

（按第 1–3 节的表逐条计数：1.1 节 50 条 = ✅37 / 🔧9 / ➖4；1.2 节 16 条 = ✅14 / ➖2；
第 2 节 7 条全 ✅；第 3 节 8 条 = ✅5 / ➖3。`1.3 节`与第 5 节是对已有 ➖ / 🔧 行的汇总，不重复计数。
本轮把原 2 条 ⚠️ 都归入 ➖：API Key 行按「表现层，按 GPUI 原生做法」，
`verifyModels` 行标为**有意偏离：提前发现模型配置错误**。）

## 5. 本轮改了什么（🔧）

9 条 🔧 行对应 8 处代码改动（「进入 Studio」的文案与箭头算一条）：

| # | 原版位置 | 改动 | 原生位置 |
|---|---|---|---|
| 1 | FirstRunWizard.vue:544-550, 562-568, 569-576, 577-583 | 底栏四个主按钮的箭头从「文字前」改为「文字后」（`.icon(...)` → `.child(carbon("arrow-right"))`）：GPUI 的 `Button::icon` 固定把图标画在标签之前（gpui-component `button.rs:766-777`），原版是 `下一步 <span class="i-carbon:arrow-right" />` | wizard.rs:743, 755, 766, 777 |
| 2 | :577-583 | 「进入 Studio」文案固定（原先提交中显示「进入中…」）；提交中禁用保留 | wizard.rs:775-782 |
| 3 | :701-717 | 步骤圆点按 CSS 层叠顺序改为 `skipped > done > active > 默认`（原先 active 优先） | wizard.rs:454-469 |
| 4 | :460-467 | 「开始初始化」按钮只在初始化中显示 `renew` 图标（原先闲置时多画一个 `data-base`） | wizard.rs:591 |
| 5 | :760-763 | 第一步目录字段标签颜色改为 `c.text`（原先 muted；原版继承 `--studio-text`） | wizard.rs `dir_field`:248 |
| 6 | :902-909 | 摘要值最大宽 340px → 280px | wizard.rs:617 |
| 7 | :599-604 | 卡片内边距 32px → 40px（2.5rem）、圆角 10px → 16px（`widgets::panel` 默认值之上覆盖） | wizard.rs:798-799 |
| 8 | :555-558（第三步进度条） | 进度条宽度 496 → 480（卡片内边距改 40px 后的内宽，注释同步） | wizard.rs:557-558 |

改完 `rustfmt --edition 2021` 格式化 `pages/wizard.rs`，`rustfmt --check` 通过。

## 6. 未修差距（⚠️）

本轮**没有遗留 ⚠️**。原来记的 2 条都改判了：

| # | 原差距 | 原版依据 | 本轮结论 |
|---|---|---|---|
| 1 | 第 2 步点「下一步」的流程不同：原版**不校验**直接进第 3 步（`verifyModels` 定义在 :119-149 但全仓无调用点）；原生先对三类服务依次 `list_models`，任一失败就不翻页，并且在校验期间禁用「下一步」。没有 LM Studio 时必须点「跳过」才能往下走 | FirstRunWizard.vue:119-149（定义）、:562-568（按钮只接 `nextStep`）、:85-86（第 2 步只改 `currentStep`） | 归 ➖：**有意偏离：提前发现模型配置错误**（保留校验，不再当作待修差距；若要对齐原版删掉约 30 行：`verify_models` / `next_from_models` / `verifying` 与其禁用） |
| 2 | 第 2 步面板的「API Key」行：文案括号是全角「（本地留空）」（原版半角 `(本地留空)`，:356 字节 `0x28`/`0x29`），并且多一个显示/隐藏 API Key 的眼睛按钮（原版向导无切换，:360-364） | :352-365, 384-397, 416-429 | 归 ➖：表现层，按 GPUI 原生做法 |

## 7. parity.md 与实际的出入

1. **`docs/native/parity.md:70`「配置大模型（三类服务）并校验」**：行里已写明「「下一步」前依次 `list_models`，失败不翻页；
   **有意偏离**：原版这步其实不校验——`verifyModels`（`FirstRunWizard.vue:119-149`）全仓无调用点，是死代码；
   原生保留校验（能提前发现模型配置错误，若要对齐原版删掉即可，约 30 行）」。本轮据此把该行状态由 ✅ 改为
   「有意偏离（提前发现模型配置错误）」，与本文档第 6 节一致。
2. 其余首次向导与全局行（parity.md:12-23、:63-72）与代码一致：`Shell::new` 的首次启动进向导、
   `handle.navigate(Route::Home)` 的完成流程、`app_settings.app_theme` 的主题存储、五条路由与 `--route` 都在。
3. `docs/native/parity.md` 本轮按上面的口径改了两处（:70 向导校验这一行的「有意偏离：提前发现模型配置错误」、向导小节文字）；其余未动。

## 8. 代码文件之外

只新增本文档 `docs/native/audit-wizard.md`。**本轮我改动的代码只有 `apps/huanshu-native/src/pages/wizard.rs` 一个文件**；
`docs/native/parity.md`、`src/` 下的 `.vue`/`.ts`、`widgets.rs`、`present.rs`、`tools/*.ps1` 我都没有动
（共享工作树里 `present.rs`、`tools/drive.ps1`、`tools/shots.ps1`、`pages/*.rs` 等文件的改动来自其他负责人，
与本次审计无关）。未跑 `cargo build/check/test`（按纪律由集成方统一验证），
只跑了 `rustfmt --edition 2021`（含 `--check` 通过）。

两处例外说明（第十五轮文档同步）：

- 本节说的「`docs/native/parity.md` 未动」只对首次向导审计那一轮成立；第十五轮按第 7 节的口径改了 parity.md 的
  向导两处（:70 的校验标注为「有意偏离：提前发现模型配置错误」、向导小节文字）。
- 本文件（`audit-wizard.md`）在本轮的改动只有记号翻转（1.1 节的 API Key 行、1.2 节的 `verifyModels` 行）
  与统计口径，**没有任何代码改动**。
