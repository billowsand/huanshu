# 代码结构：`huanshu-core` 与界面层

业务逻辑都在 `crates/huanshu-core`（**不依赖 Tauri**，`cargo tree -p huanshu-core | grep -i tauri` 必须为空），
界面层只做转发。界面正从 Tauri + Vue 逐页迁往原生 GPUI（`spikes/gpui-kit/`、`docs/spikes/gpui-kit.md`），两套界面共用 core。

- `huanshu_core::AppState`（`state.rs`）：设置、当前牌组、SQLite 连接与存储根；界面层用自己的锁包住它
  （Tauri：`commands::Shared = Arc<Mutex<AppState>>`），服务函数只认 `&Mutex<AppState>`。
- `services/`：`generation`（流水线、重试、图标）、`projects`、`storage`（`.keynn` 导入导出与加密）、`media`、
  `settings`、`export`（PDF 与缩略图，包装 `slide-typst`）。错误类型是 `String`，命令层直接透传。
- 事件：core 只认 `events::EventSink` / `CoreEvent`，`CoreEvent::wire()` 给出 `(事件名, 载荷)`；Tauri 的 `TauriEventSink`
  转发给 `AppHandle::emit`。线上格式由 `tests/fixtures/generation-events.json` 锁定（`src-tauri` 的单测比对）。
- 带 `async` 的服务函数要求调用方处于 tokio 运行时中；同步重活（PDF、缩略图、字体枚举）由命令层放进 `spawn_blocking`。
- `src-tauri/src/commands/*.rs` 的每个命令只做「取参数 → 调 core → 转错误」，**不要**再把 SQL、文件读写或业务分支写回命令层。
- 前端的「规则」也在 core：编辑器版式表 `editor_schema.rs`（`get_editor_schema`，前端缓存在 `stores/editorSchema.ts`）、
  图表校验 `chart_check.rs`（`chart_problems`）。
- 暂时允许的重复（Tauri 界面下线前，**改一边必须改另一边**）：缩略图调度 `preview.rs` ↔ `useTypstThumbnails.ts`、
  主题清单 `themes.rs` ↔ `src/themes/catalog.ts`、第二步进度规则 `generation_progress.rs` ↔
  `src/utils/generationProgress.ts` / `src/stores/generation.ts` / `Step2Generate.vue`（快照
  `tests/fixtures/generation-progress.json` 由 `target/` 下一次性 bun 脚本从原版代码生成，`tests/generation_progress.rs` 比对）。
  第三步编辑规则 `slide_edit.rs` ↔ `src/composables/useSlideEditor.ts` / `Step3Editor.vue`（快照 `tests/fixtures/slide-edit.json`，
  `tests/slide_edit.rs` 比对；`filterSlideJson` 两边都额外保留 `aspect_ratio`，否则应用一次更改就丢掉页面比例）。

## 隔离数据目录（真机验证必读）

**真机验证一律在隔离目录里做，绝不写用户的真实数据**（2026-10 曾有一轮验证把用户真实项目的封面改坏并保存）。
设置 `HUANSHU_HOME` 后，数据库、`settings.json`、素材与 `bootstrap.json` 全部落在它下面（`state.rs::storage_paths`）；
未设置时与从前逐字相同。

```powershell
$env:HUANSHU_HOME = 'E:\huanshu\target\sandbox-home'
cargo run -p huanshu-core --example seed_demo -- $env:HUANSHU_HOME   # 已有库时加 --force 重建
```

`seed_demo` 建 9 个演示项目（全部版式 16:9 / 32:9 / 48:9、图表、图表边界、压力、图片，以及第二步用的
「生成记录演示」——一次以 error 结束、第 3 页失败的运行——和 60 个内容页的「长牌组」），并标记首次向导已完成；
拒绝以真实数据目录或 bootstrap 所在目录为目标。没有 LM Studio 时，生成与重试的成功路径只能人工验收。Tauri 版用 `.claude/launch.json` 的 `tauri-dev-sandbox`。

## 原生界面（`apps/huanshu-native`，GPUI Kit）

逐页对照 Tauri + Vue 版，**目标是完整替代原版**：关键功能一项都不能丢，但**要贴合 GPUI、用好它的能力，不是把网页逐像素复制过来**。
功能性差距（缺信息、丢状态、数据安全、流程走不通）必须补；尺寸差几像素、悬停底色、控件形态、CSS 技巧（sticky、毛玻璃、两段式对话框）
这类表现层差异优先用 GPUI Kit 组件的原生做法，不算缺陷，不要为了像原版去模拟。
框架与写页面的规矩见 `apps/huanshu-native/README.md`；功能清单与进度是 `docs/native/parity.md`（做完一项改一项）；
原版截图 `docs/native/reference/`，原生截图 `docs/native/shell/`（`tools/shots.ps1`，同一份沙箱数据、同一主题）。
在根 workspace 里但**不在 `default-members`**：`bunx tauri dev`、裸 `cargo build` 不编 GPUI，`cargo test --workspace` 与 `-p huanshu-native` 会编。

- 外壳 `shell.rs`（标题栏 + 当前页 + 演示覆盖层），页面经 `ShellHandle` 换页、换主题、开演示；路由 `router.rs` 对应原版五条路由。
- 配色只用 `cx.studio().colors`：`theme/tokens.rs` 是从运行中的旧应用读出的 `--studio-*` 计算值（改 CSS 时同步）。
  `theme::apply` 同时写 GPUI Kit 的 `Theme.colors` 并重派生 `Theme.tokens`（组件读 tokens，不重派生主按钮就是白底）。
- 第一步的 Markdown 编辑器读 GPUI Kit 的 `Theme.highlight_theme`（底色、当前行、行号栏、语法高亮），
  每个主题一份在 `theme/editor.rs`，**不设就是白底蓝标题**；字体 / 行高在 `cx.studio().editor`。
  便签主题另有 `cx.studio().paper`（锤子便签式稿纸：32px 横线、页边、宋体、木纹 `notes/wood.svg`、纸张阴影），
  页面按 `paper.is_some()` 分支，不按主题 id 判断。
- 标题栏是自绘的（`widgets::title_bar`，不用 GPUI Kit 的 `TitleBar`：它的窗口按钮固定用 `Theme.foreground`，
  便签主题的深棕标题栏上看不见）。打开文稿时正中是文稿名（点它改名）；标题栏整条是拖动区（`HTCAPTION`），
  里面要接收点击的元素必须 `.occlude()`。工作流各栏顶栏同高 `workflow::HEADER_H`（64px）。
- 图标用原版同一套 Carbon（`icons::carbon("i-carbon:xxx")`，SVG 由 `slide_typst::render_icon_svg` 现生成）。
- 数据只经 `huanshu_core::services`；同步重活 `tasks::run_bg`，core 的 async 服务 `tasks::run_async`（进程内一个 tokio 运行时），
  生成事件 `tasks::listen_events`（`CoreEvent` 通道，替代 Tauri 的 `listen('gen:*')`）。
- 幻灯片位图只用 `slides::SlideImages`（包 `huanshu_core::preview::PreviewCache`），页面离开调 `release`。
  **每张不再显示的 `RenderImage` 都要 `drop_image`**，GPUI 不会回收图集。
- `PreviewCache::set_deck` / `set_wanted` 只在内容真变了时才重新打开失败闸：界面每帧都会调用它们，无条件打开会让编译不过的牌组每帧重试。
- 页面快捷键用 `track_focus` + `on_key_down`；**不要用 `observe_keystrokes`**（全应用的，会收到其他窗口和其他页面的按键）。
- 界面里的延时（防抖、自动跳页）用 GPUI 的 `background_executor().timer(..)`，**不要用 `tokio::time::sleep`**：它需要
  `tokio().enter()`，守卫跨 `.await` 留在 UI 线程上，两个计时同时在等就会乱序释放，tokio 直接 panic。tokio 只给 core 的 async 服务用（`tasks::run_async`）。
- 生成状态是应用级的 `generation::GenerationStore`（原版 pinia 的 `generation` store）：`Shell` 创建、经
  `ShellHandle::generation()` 下发，事件通道常驻在它身上，页面来去不影响在途生成。页面只读它
  （`state_for` / `snapshot_for`：store 正持有本项目用实时状态，否则用库里的静态状态）。全应用同一时刻
  只有一次生成或重试（`busy()`）。`ProjectModel.deck` 是 store 牌组的镜像：**第三步做编辑时必须把改动
  写回 store**（原版直接改 `gen.blueprints`），否则 store 下一次通知（如单页重试完成）会用旧牌组盖掉编辑。
  现在第三步的一切牌组改动只走 `WorkflowPage::commit_deck`（模型 → `GenerationStore::set_deck` → 落库）。
- **core 的生成与「当前打开的项目」已解耦**（2026-10，native-step3b）：一次生成属于它开始时的项目
  （`generation_runs.project_id`）——组装落库写那个项目的行并记一条 `assembling/assembled` 日志，
  `AppState.blueprints` 只在那个项目仍打开时镜像；单页重试的底稿与写回、
  `get_retry_layouts`、`get_generation_deck(project_id)`（运行到达过组装才返回牌组）都按运行所属项目，
  不看 `active_project_id`。原生版 `WorkflowPage::editing_locked` 也因此只在本项目生成/重试时只读；
  Tauri 侧 store 生成结束取牌组走 `get_generation_deck`（发起生成的项目；`/new` 未落库时保留
  `get_blueprints`）；用户已打开别的项目时（`activeProjectId` 变了）结果不写进 store，`loadFromJson` 会
  `stopListening`，`WorkflowView` 生成后不再自己存 `gen.blueprints`（core 已落库，再存会把别的项目的牌组写进来）。
- 演示是盖在当前页上的覆盖层（与原版一致），打开即全屏，全屏时外壳不画标题栏。Esc 照原版：先关全览；
  全屏时只退出全屏，非全屏时才退出演示。
- `apps/huanshu-native/build.rs` 把 Windows 主线程栈调到 8 MiB：默认 1 MiB，debug 构建打开第三步表单就栈溢出（release 不会）。
- 主题存在 `app_settings` 的 `app_theme` 键，与 Tauri 版（WebView localStorage）暂不同步。
- 量内存要量**本进程**：PowerShell 里的 `$PID` 是 PowerShell 自己。只读查看器阶段实测（release，5 分钟每 100ms 翻一页）工作集稳定在 385–412 MiB。
- **原版首次向导在沙箱里会指回真实目录**：`FirstRunWizard.vue` 用前端的 `localDataDir()` 拼默认目录，不认 `HUANSHU_HOME`。
  在 `tauri-dev-sandbox` 里不要点向导的「进入 Studio」（用 `complete_first_run` 命令跳过）；原生版的向导取 core 的目录，没有这个问题。

# Auto-Slidev 模板扩展说明

## 新增一个 Slide 模板的标准流程
当你要给 Auto-Slidev 增加一个新模板时，按下面顺序改（幻灯片只由 Typst 渲染，见「Typst 原生 PDF 导出」一节；
Rust 生成侧只产出 blueprint，前端没有幻灯片组件）：

1. `crates/slide-typst/typst/layouts/`
   新建 `<kind>.typ` 版式，并按「新增一个版式」登记（`main.typ`、`lib.rs` 的 `TEMPLATES` / `SUPPORTED_KINDS`、fixture）。
2. `crates/huanshu-core/src/types.rs`
   新增 `SlideKind` 枚举项，以及该模板对应的数据结构。
3. `crates/huanshu-core/src/generator/layout_rules.rs`
   在 `LAYOUT_RULES` 表里加一行（布局提示词由这张表生成，代码过滤也用它）：
   - `use_when`：什么时候应该选这个模板、它和相邻模板的边界是什么
   - `requirement` + `check`：什么时候**不能**选（硬条件，宜宽松，多个信号取 OR）
   - `blocked_aspects`：哪些比例下版式会崩（如超宽下的双栏/单图）
   另在 `crates/huanshu-core/src/generator/layout_scoring.rs` 的 `PRIORS` 加一行先验分，并在 `SCORE_RULES`
   写上哪些页面信号该给它加分（LLM 失败时的兜底排序靠这张表）。
4. `crates/huanshu-core/src/generator/utils.rs`
   在 `blueprint_schema_hint()` 里加入严格 JSON 示例。
5. `crates/huanshu-core/src/generator/slides.rs`
   在内容生成提示词里加入该模板的字段要求。
6. `crates/huanshu-core/src/generator/normalize.rs`
   增加默认值修复、长度收敛、tone/icon 修复。
7. `crates/huanshu-core/src/validate.rs`
   增加结构校验，避免生成结果进入非法状态。
8. `crates/huanshu-core/src/editor_schema.rs`
   编辑器的版式表（中文名、图标、说明、字段、默认内容）与表单定义（`list_sections` / `object_sections` 的字段、
   「新增一项」模板 `new_item`）。改完同步更新快照 `crates/huanshu-core/tests/fixtures/editor-schema.json`
   （单测逐字比对，并检查每个 kind 都有条目、字段名真实存在、默认值能反序列化）。前端经 `get_editor_schema` 读取，
   **不要在 TypeScript 里另写字段表**；`src/components/types.ts` 只补类型。
9. 文档
   更新 `CLAUDE.md`，把版式用途、字段和新增流程记录下来。


## 做新模板时最容易漏掉的点

- 只写了 Typst 版式，没有把它接到 `SlideKind`
- 只改了 layout 选择，没有补内容生成提示词
- schema hint 不完整，导致 LLM 输出字段漂移
- 没有 normalize/validate，偶发坏 JSON 会直接进渲染阶段
- 没有写"不要使用此模板"的规则，导致模板被误选
- 在 schema hint 里写 `"section": "NN"` 示例值：LLM 会照抄（历史上 process 页全是 09、section_intro 全是 01）。
  章节号（幻灯片上的大号 `.section-num`）由代码决定：`derive_page_plans` 按 H2 序号写入
  `PagePlan.section_number`（与提纲总览共用 `planning::chapter_number`），流水线末尾和兜底页用
  `normalize::stamp_section_number` 覆盖 LLM 输出；按反馈修复单页时保留原章节号


## 移除一个 Slide 模板（反流程）

2026-09 实例：移除 `Infographic` 模板与 `@antv/infographic` 依赖，模板数 18 → 17。
**不要重新加回 @antv/infographic**（用户明确弃用；`SplitLayers` 右侧也一并改回内置分层卡片渲染）。

反流程比加模板多两个易漏点：**LLM 提示词白名单**和**旧数据兼容**。

Typst 与前端：
1. `crates/slide-typst/typst/layouts/<kind>.typ` 删文件，去掉 `main.typ` 的 `layouts` 表、`lib.rs` 的 `TEMPLATES` /
   `SUPPORTED_KINDS` 中的项，以及 fixture 里该 kind 的页（2026-09 那次删的还是 Vue 模板；Vue 模板已于 2026-10 全部移除）
2. `src/components/types.ts`：`SlideKind` 联合类型、`SlideBlueprint` 中该模板专属字段
3. `crates/huanshu-core/src/editor_schema.rs`：该 kind 的条目与表单定义，并更新快照 `tests/fixtures/editor-schema.json`
4. （编辑器字段表已全部在 Rust，前端无需再改）

Rust（变体与 `SlideBlueprint` 字段必须同时删干净，否则各处 struct 字面量编译不过）：
6. `crates/huanshu-core/src/types.rs`：`SlideKind` 变体 + `SlideBlueprint` 字段
7. `crates/huanshu-core/src/generator/layout_rules.rs`：`LAYOUT_RULES` 中该模板的行（提示词白名单与规则文案由它生成）；`crates/huanshu-core/src/generator/layout_scoring.rs`：`PRIORS` 与 `SCORE_RULES` 中该模板的行；`crates/huanshu-core/src/generator/planning.rs`：各 `SlideBlueprint { .. }` 字面量
8. `crates/huanshu-core/src/generator/slides.rs`：内容生成提示词的字段要求
9. `crates/huanshu-core/src/generator/utils.rs`：`slide_kind_name`（`SlideKind → snake_case`）映射、`blueprint_schema_hint`
10. `crates/huanshu-core/src/generator/normalize.rs`：默认值修复分支 + 字面量
11. `crates/huanshu-core/src/validate.rs`：结构校验
12. 文档：README 模板数、CHANGELOG（新增 `[Unreleased]`，不要改已发布版本号条目）

必查两点：
- `grep -rn "<模板名>" src src-tauri/src` 应 0 命中（仅剩迁移说明/测试里的字符串）
- **旧数据兼容**：`SlideKind` 是 `#[serde(rename_all = "snake_case")]` 外部标签枚举，删变体后 DB / `.keynn` 里带旧 kind 的牌组会**整体反序列化失败**，而 `services/projects.rs` 用的是 `unwrap_or_default()` → 项目静默变空。所以 `SlideBlueprint.kind` 挂了
  `#[serde(deserialize_with = "de_slide_kind")]`（未知 kind → `Cover`），并有回归测试 `legacy_infographic_kind_degrades_without_losing_slides`。
  Typst 侧对没有版式的 kind 渲染「Typst 版式尚未实现」占位页，不会丢页。


## 布局选择：候选 + 全局分配

生成流水线（`crates/huanshu-core/src/services/generation.rs` 的 `run_pipeline`）分三段，布局**不再逐页即时决定**：

1. **2a 并发规划**（`slides.rs::plan_single_page`）：每页 enrich + `planning::layout_candidates_for_page`，
   LLM 只按「本页适配度」给 2-3 个候选打分，**提示词里不要再加多样性信息**。
   提示词只列出本页**可用**的 kind（并说明其余 kind 为何不可用），候选再经 `sanitize_candidates`
   按同一张适用条件表过滤（见下）；`section_summary` 页不调 LLM，直接 `section_intro`；
   LLM 失败时用 `fallback_candidates`（两个不同的可用 kind）。
   LLM 回复由 `generator/candidate_parsing.rs` **逐条宽松解析**（不要改回直接反序列化
   `Vec<LayoutCandidate>`：一条坏条目会让整份回复作废）：容忍 kind 拼写漂移（`FeatureGrid` /
   `feature-grid` / `KeynoteFeatureGridSlide`）、分数格式（`"8"`、`8.6`、`"7/10"`、百分制、小数）、
   外层结构漂移（顶层数组、`layouts` 键、单个对象、多包一层）；缺分数按排名给 8、7、6…。
   解析只管语法，能否使用仍由 `sanitize_candidates` 判定；丢弃的条目写进 `02-layout-NN.parsed.json` 的 `dropped`。
2. **2b 全局分配**（`planning::assign_layouts`，纯 CPU、确定性）：最大化
   `Σ score − 3·相邻同款 − 1·隔页同款 − 2·超出上限的次数`，单一 kind 上限为内容页数的 30%（至少 2）。
   先 Viterbi 精确求解相邻项，再做单页交换的局部搜索处理全局项。结果只取决于候选，与并发数无关；
   分配明细写入调试目录 `02-layout-assignment.json`。权重常量在 `planning.rs` 的 `ADJACENT_SAME_PENALTY` 等。
3. **2c 并发生成**（`slides.rs::generate_single_page_pipeline`）：content → normalize → validate/repair。
   repair 最后一轮仍可能用 `pick_different_kind` 换模板，此时不会回头重排其他页（少见，可接受）。

### 图表页（`chart_focus`）

数据来自 Markdown 的 ```` ```chart ```` 围栏块（`input.rs` 解析，`ChartSpec::problems()` 不通过就直接报错给用户），
按位置挂到 H2/H3，由 `planning::push_text_unit` 分页：
- 正文短（`content_units` ≤ `CHART_PAGE_SPLIT_UNITS` = 150，约 300 汉字）：首图随文本页（chart_focus：左侧 2-4 条解读 + 右侧图表）；
- 正文长：**拆成两页**——文本页不带图、照常选版式，图表紧跟着单独成页（带截断 200 字的原文做解读语境）；
- 同一文本单元的其余图表各追加一页；H2 走 `section_summary` 时它自己的图全部单独成页。
- **带图表的页固定 `chart_focus`**（`planning::forced_kind`，与 section_summary → section_intro 同一机制）：
  它是唯一渲染图表的模板，换成别的模板等于静默丢掉用户数据。所以 repair 最后一轮、`make_fallback_slide` 也都回到 chart_focus。
- **chart 由代码挂载，不让 LLM 抄**：内容提示词只让模型写 `points`；`normalize::ensure_chart_from_page_plan` 在**校验之前**
  和每轮 repair 之后用 page plan 的原值覆盖。数据错误 LLM 修不了，绝不能让它进 repair 循环（否则最后一轮换模板丢图）。
  反馈修复（`repair_slide`）同样保留原 chart。enrich 不会把 `visual_need=chart` 降回 `text_only`。
- **图表由 Typst 原生绘制**（`crates/slide-typst/typst/lib/chart.typ`）：缩略图、演示、编辑器预览、PDF 全走它，不需要 WebView。
  它移植自已删除的 ECharts 版 `buildChartOption`，几何按旧 ECharts SSR 输出逐项对过（栅格边距、平滑曲线的
  控制点与防过冲钳制、pin 气泡路径、环形图 48%/66% 半径与圆角、图例），五种图与旧 SVG 的平均像素差 ≤ 0.43。
  它现在是图表的唯一实现（前端已不依赖 ECharts）。与原 option 一样，只有 `fs(n)` 的字号乘本页字号系数（`ctx.scale`），
  边距、线宽、符号尺寸是固定 CSS px。
  - 数值轴：ECharts 的 `nice` 取整（`{1,2,3,5}×10ᵏ`，目标 5 段，含 0）；数字千分位、最多两位小数、无浮点尾巴（`format-number`）。
  - 类目太多时隔 k 个显示一个标签（不旋转、不缩小）；相邻数据标签会相撞时整组不画（高亮 pin 照画）。
  - 环形图：扇区之间切出约 2px 缝隙露出卡片（ECharts 用页面底色描边，主题里没有不透明的页面色）；0 和负值扇区不画；
    标签按左右两侧推开、引线避开圆环，扇区很多又挤在一侧时仍可能重叠（ECharts 同样会）。
  - 堆叠面积的负值按 0 处理；负值柱从 0 向下画，圆角在远端，标签在柱下。
  - 画不出来的数据（类型未知、缺列、非数字、环形图没有正数、chart 不是对象）由 `chart-read` 判定，卡片里显示占位和原因，
    **绝不能编译失败**：一张坏图会让整份导出和整副缩略图一起失败（有单测）。图表里的字符串一律当纯文本（有注入单测）。
- 内容量估算（typst `content-load`）跳过 `chart` 键：数据行不是排版文字，计入会让图表页一律落到最小字号。
- 图表数据校验只有一份：`crates/huanshu-core/src/chart_check.rs`（`ChartProblem` 枚举，`chart_problems_json` 接受任意 JSON）。
  `ChartSpec::problems()` 用 `message_en()`（解析 Markdown、`validate.rs`，措辞被测试与 LLM 报错依赖）；Step3 编辑器
  「图表数据（JSON）」框与高级 JSON 经 `chart_problems` 命令取 `message_zh()`。编辑框的校验是异步的：输入后 150ms 防抖，
  只有最新一次输入、且仍在发起校验的那一页时结果才生效；只有通过校验的 JSON 才写回牌组；失焦时立即校验还在等待的输入
  （否则粘贴后马上点别处会丢掉修改）。
- 配色两份：屏幕 CSS（`--chart-*`，`themeFromCssVars` 读取）与每个 typst 主题字典的 `chart`（`palette` 6 色、`grid`、`axis`、
  `success`，`_check.typ` 校验键集合与色数），文字色用主题的 `ink` / `muted`。改主题的图表色两边一起改。
- 卡片校准结果（删除 Vue 模板前在对照页实测）：绘图框宽度一致（646/647），高度差 ≤ 5px（仅「有副标题、无页脚」时，源自通用 `content-page` 几何）；
  左栏要点与 focus_example 同一套度量（card-title 标题、caption 正文 70%、序号同标题字号）；卡片底色取 `paper`；
  图表标题为墨色（模板里的 `text="var(--primary)"` 在屏幕上不生效，13 处模板都如此，typst 一律按屏幕实测）。
- 回归与目检：`crates/slide-typst/tests/fixtures/charts.json`（五种图）与 `charts-edge.json`（负值、千分位、小数、30 个类目、
  单点、全零、含 0/负值扇区、12 扇区、6 系列、四种占位）。改了 `chart.typ` 后渲染出图目检：

  ```bash
  cargo run --release -p slide-typst --example render_fixture -- crates/slide-typst/tests/fixtures/charts.json target/chart-check --png --theme dark-blue
  cargo run --release -p slide-typst --example render_fixture -- crates/slide-typst/tests/fixtures/charts-edge.json target/chart-check/edge --png
  ```
- 来源角标 `chartSourceLabel()` 与 `chart-focus.typ` 的 `source-caption` 必须同步；`source` 是内部枚举，不能原样显示。

### 适用条件表（`generator/layout_rules.rs`）

`LAYOUT_RULES` 是「模板何时可用」的唯一来源，以下全部从它派生，**不要再在别处手写模板白名单或硬约束**：
- 布局提示词（`prompt_rules`：可用 kind 白名单 + `use_when` 文案 + 不可用 kind 的原因 + 超宽比例的软偏好）
- LLM 候选过滤（`sanitize_candidates`，丢弃时 `eprintln` 原因）
- 所有确定性兜底：`fallback_candidates`、repair 最后一轮的 `pick_different_kind`、`make_fallback_slide`
  都走 `planning::eligible_fallback_kinds`，只会返回可用 kind

现有硬条件：`step_flow` 要点 ≤4 / `process` 要点 ≥5；`compare` 需 pair 或 comparison；
`issue_stack` 需 warning/causal 或问题类措辞；`swot` 需 matrix 或 SWOT 措辞；`spotlight` 需
`preferred_assets` 非空；32:9 禁 `spotlight`，48:9 禁 `spotlight`/`compare`/`focus_example`；
`section_intro` 仅且必须用于 `section_summary` 页；`chart_focus` 仅且必须用于带 ```` ```chart ```` 的页；Cover/Overview/Closing 不在表中，永不可选。

不变量：`UNIVERSAL_CONTENT_KINDS`（`feature_grid`、`section_list`）对任何内容页、任何比例都可用，
兜底链依赖它保证总能找到可用 kind——给这两个模板加硬条件前先改兜底逻辑（有单测守护）。

### 兜底打分表（`generator/layout_scoring.rs`）

LLM 没给出可用候选、repair 最后一轮换模板、生成兜底页时，用这张表给所有内容模板打分排序，
再经上面的适用条件表过滤。它取代了旧的 `safe_fallback_kind` if 链（旧链规则顺序会让
「summary + 2-4 要点 → outcome_grid」抢先截走 timeline/workflow/问题类页面）。

- `PRIORS`：每个内容模板一行先验分（平局时偏向通用模板），单测要求恰好覆盖 14 个内容模板
- `SCORE_RULES`：每行「信号命中 → 给某模板加权重」，带 `signal` 标签；兜底候选的 `reason`
  会列出命中的前 3 个信号，可在 `02-layout-assignment.json` 里看到
- 权重约定：具体结构信号（content_shape、warning、问题措辞）3-4，辅助信号 1.5-2，
  泛化提示（要点数、密度）0.5-1。调权重后跑 `layout_scoring` 的单测，它们固定了典型页面的首选
- `PROBLEM_WORDS` 由 `layout_rules` 导出，两张表共用

页面初始信号来自 `planning.rs::infer_signals`：关键词用多字词组（`ARCHITECTURE_PHRASES` 等），
**不要**再用单字「层」或泛词「时间」；density 用 `content_units`（拉丁词数 + 汉字数/2），
因为中文没有空格，按空白分词会让几乎所有中文页都判成 low。

### 观测与真机探针

每次生成在调试目录（项目的 `.slidev-gen-debug/`）写出：
- `02-layout-NN.parsed.json`：该页解析出的候选与 `dropped`（解析不了的条目及原因）
- `02-layout-assignment.json`：`stats.assigned` 与 `stats.llm_top_choice`（逐页取 LLM 首选时的
  牌组）的对比——相邻/隔页重复次数、kind 分布、最大占比（`section_intro` 不计）；
  `traded_down_pages`（为多样性改选）、`fallback_pages`（走了确定性兜底）、逐页候选
- `05-layout-final.json`：repair/兜底之后**实际渲染**的分布，及与分配结果不一致的页
- 生成日志「布局分配」一行汇总上述关键数字

调权重、改规则或换模型后，用真机探针在固定文档上对比（需 LM Studio 在跑，读应用的 settings.json）：

```bash
cargo test -p huanshu-core --lib layout_probe -- --ignored --nocapture
```

默认文档 `crates/huanshu-core/fixtures/layout-probe-sample.md`（覆盖对比/架构/流程/时间线/SWOT/风险等页型）；
环境变量 `LAYOUT_PROBE_MD` / `LAYOUT_PROBE_ASPECT`（`16x9`/`32x9`/`48x9`）/ `LAYOUT_PROBE_OUT` /
`LAYOUT_PROBE_SETTINGS` 可覆盖。只跑 enrich → 候选 → 分配，18 页约 90 秒（qwen3.5-9b，并发 4）。
LLM 回复即使不是合法 JSON（实测出现过单引号字符串），`candidate_parsing` 也会按模式抢救出 kind/score。


## 素材库图片与生成链路

素材有两个来源，统一由 `crates/huanshu-core/src/assets.rs` 的 `AssetCatalog` 表达：

- 项目目录下的 `figure/`、`public/` 文件：`collect_assets` 会为每个文件产出 `figure/x.png`、`/figure/x.png`、`x.png` 三种拼写；
- 素材库图片：文件在 `<media_dir>/projects/<id>/images/`，元数据在 `media_assets` 表，引用形式是 `media:<id>`。

`AssetCatalog.refs` 是「蓝图里合法可用的引用集合」（校验用），`entries` 是给提示词的目录（`media:` 条目带 caption 与宽高比）。

要点：

- `run_pipeline` / `retry_generation_page` / 反馈修复三处都通过 `services/generation.rs::load_asset_catalog` 构造目录
  （项目文件 + 该项目的素材库图片）。**不要把 `collect_assets` 直接当素材全集**——它扫不到素材库。
- 素材库图片只有在目录里出现，`preferred_assets` 才可能带上它（`planning::normalize_preferred_assets` 丢弃未知引用），
  `layout_rules::has_image` 才会放行 `spotlight`。
- 导入素材时 `services/media.rs` 用 `image_size::image_dimensions` 读文件头写 `media_assets.width/height`
  （零依赖，支持 PNG/JPEG/GIF/BMP/WebP/SVG）；`.keynn` 导出/导入携带宽高，旧包回退到读还原后的字节。
- 给模型的素材清单一律走 `AssetCatalog::prompt_section()`，不要手写路径列表——`media:` 条目必须带 caption 与比例，模型才选得准。
- `spotlight` 是唯一渲染图片的模板：`normalize::ensure_image_from_page_plan` 会在它缺图时用 `page.preferred_assets[0]` 兜底填充。
  `layout_scoring` 的 `has_assets` 规则（+2.0）只做辅助加权，**不要加到能压过 content_shape 结构信号（3-4）的量级**，
  否则会把流程/对比页抢成图片页（有单测守护这一点）。
- 图片目前一律 `object-contain`（保比例、留白），`SlideBlueprint` 没有裁剪/遮罩/画框字段；固定画框 + `cover` +
  槽位（16:9 / 4:3 / 1:1）是尚未实现的一步。素材库图片在自动生成里只影响「是否选 spotlight」，不影响图片自身呈现。

### 图文配对（生成时，Phase 1b）

caption 只说图片「是什么」，说不出它「该放哪」。所以 `generator/media_match.rs` 在 `derive_page_plans` 之后、
2a 之前，把每张素材图片连同**文稿大纲**一起交给多模态模型：

- 输入：`page_outline` 生成的分页大纲（`page_id`、页标题、章节、要点、原文摘要截断 120 字）+ 单张图片；
- 输出：`MediaPlacement`（目标页、用途、带语境的图注）。`page_id` 经**白名单校验**，不在牌组里的一律丢弃，
  模型编造的页码不会进 `preferred_assets`；`page_id` 为空（判断为不相关）时图注仍然保留用于回写；
- 上限 `MAX_MATCHED_IMAGES = 12`、按 `config.concurrency` 限流、逐图容错（失败只丢这一张，退回 caption-only 行为）；
- 结果写调试目录 `00-media-match.json`，并记入生成日志「图文配对」一行；
- 图注回写 `media_assets.caption`（用 `db::update_media_asset_caption_only`，**只动 caption，不动用户写的 description**）；
- `config.multimodal.model` 为空（未配置多模态）或项目没有素材图片时整段跳过，行为等同于旧路径。

配对结果是**软信号**，不要改成硬约束：

- `AssetCatalog::set_placements` 之后，`prompt_section_for(page_id)` 会在素材目录前插入 `Matched to this page …`
  段（引用 + 用途）；enrich（`planning.rs`）与内容生成（`slides.rs`）两条提示词都走它；
- `planning::merge_page_signal_patch` 只在模型**没有**给出 `preferred_assets` 时才用配对结果兜底（`take(2)`）；
- 因此配对只提高「选对页的概率」，不剥夺模型按版式改选的能力。

## 幻灯片字号体系（`type-sizes` / `--type-scale`）

幻灯片只由 Typst 渲染，字号体系在 `crates/slide-typst/typst/lib/base.typ`（它移植自已删除的 Vue 版 `--type-scale`）：

- 逐页系数 `slide-type-scale(slide)`（由 `content-load` 估算内容量）：内容极少的页（封面/结尾）`1.425`，
  中等 `1.325`，密集页 `1.25`；`main.typ` 把它放进 `ctx.scale`，`type-sizes(scale)` 得出 `ctx.ty` 的整套字号。
  版式里只用 `ctx.ty.*`（再乘 `fit-height` 的 `f`），**不要**写裸字号。
- `content-load` 跳过 `chart` 等非排版字段（见「图表页」）。
- 已知边界：`spotlight` 侧栏在「3 个面板 + 每面板多项」时放不下；`normalize` 已把面板条目收口到 4 项，
  侧栏用 `fit-height` 实测缩放，仍放不下时裁切并报告溢出。

## 幻灯片字体角色

用户在「设置 → 外观与字体」里独立选择 6 类字体：标题、副标题、正文、注释与标签、数字与序号、代码。
幻灯片里每一处文字都落到其中一类（界面字体 `--font-interface` 只服务 Studio 外壳）。

- Typst 侧：`services/export.rs` 的 `font_roles` 把设置写进 `ExportOptions.fonts`，`main.typ` 放进 `ctx.fonts`，版式里用
  `txt(ctx, "<role>", …)`（`components.typ`）。判断依据是「这段话在版式里扮演什么角色」，**不是字号大小**：
  例如 SWOT 的策略结论借用注释字号但属于正文，Process「关键动作 03」里的计数属于数字。
- 字重：全局 `set text(weight: "regular")`，要粗体标题须在设置里直接选粗体字族。
- CSS 侧（`src/themes/typography.css`）只剩 Step1 的 Markdown 预览 `.md-preview`：整体正文，标题与代码块单独接管；
  另有 `#app *` 把 Studio 外壳的字重拍平成常规体。

## 报刊·钴蓝主题（`light-editorial`）

取样自报刊/杂志版式，2026-10-16 按确认的设计预览升级：暖白纸面 `#f6f5f0`、黑墨 `#141414`、钴蓝 `#2549d8`；方角、细线与开放分栏。详细规范见 `docs/design/editorial-design.md`。
幻灯片样式在 `crates/slide-typst/typst/themes/light-editorial.typ`（`family: "editorial"`）；`src/themes/editorial.css`
现在只管 Studio 外壳与主题 token（Vue 模板删除后，它的幻灯片规则一并删除）。

- 顶部 3px 钴蓝线、内容页灰色细分隔线与灰色副标题；底部统一品牌、细线与真实页码。标题和数字加粗，封面采用左侧大标题、右侧描边编号（有长文测量收缩）。
- **双色大标题**：封面与结尾标题在第一个全角逗号/冒号/竖线或破折号之后断开（ASCII `:` / `|` 必须后跟空白，
  `10:30` 不会断），后半句换行并改用钴蓝。规则在 `lib/components.typ` 的 `title-tail` / `headline`（Typst 正则走
  Rust regex，不支持前瞻，所以写成 `[:|]\s`）；有单测 `editorial_headline_sets_the_tail_in_the_accent`。普通内容页标题不分色。
- 共用卡片与目录卡片改为上细线；成果页和图表页采用竖向列线。其余版式仍复用兼容的终端几何；不要全局移除 `is-terminal` 对 editorial 的兼容，否则会影响已有版式。
- 首页采用刊头标题、真实项目数、宽屏双列方角卡片与项目文字封面；Vue 与原生 token 同步。设置页「套用主题推荐字体」与自定义刊名/期号仍待后续需求。

## Typst 原生 PDF 导出（实验中，`crates/slide-typst`）

独立的 workspace crate，由 `crates/huanshu-core/src/services/export.rs` 接入应用（命令外壳在 `src-tauri/src/commands/pdf.rs`）（见下方「应用接入」）。它在进程内嵌入 Typst 编译器，把 `SlideBlueprint`
JSON 直接排版成矢量 PDF，不依赖浏览器和 typst CLI。它也是**唯一的幻灯片渲染器**（屏幕上的缩略图、演示、编辑器预览都是它
光栅化的 PNG，见下方「Typst 缩略图」），Vue 模板已于 2026-10 删除。原先它是 Vue 模板之外的另一套
版式实现。18 个模板（`SUPPORTED_KINDS`）与全部 6 个主题（`SUPPORTED_THEMES`：便签·稿纸、终端·纸/墨、报刊·钴蓝、暗·琥珀/星图）都已移植；
未知主题 id 回落到便签（`main.typ` 的 `themes` 表兜底，`services/export.rs` 会把回落记进导出报告）。以后新增的模板在移植前
会渲染成「Typst 版式尚未实现」占位页，不会丢页，导出结果弹窗会列出这些页。

- Rust（`src/lib.rs`）：`Renderer::with_system_fonts()` 扫描一次系统字体（约 0.5 秒，进程内复用），
  然后调用 `render_pdf(&slides, &ExportOptions)`，返回 `Rendered { pdf, pages, overflows, missing_assets }`。
  自定义 `World` 只提供四类文件：`include_str!` 内嵌的模板、虚拟的 `/deck.json`、
  `/icons/<set>/<name>/<rrggbb[aa]>.svg`（由 `build.rs` 内嵌的 Carbon 图标集按颜色现生成，alpha 转为不透明度），
  以及 `/assets/<n>.<ext>`（校验过的图片，见下）。
- 图片（`src/assets.rs`）：调用方用 `ExportOptions.assets`（`Assets`）把蓝图里出现的每种引用写法
  （`media:<id>`、`figure/x.png`、`/figure/x.png`、`x.png`）映射到文件路径或字节，`data:` URI 会直接解码。
  编译前每张图都会按文件头识别格式，并用与 Typst 相同的解码器完整解码一遍：**一张坏图本来会让整份导出失败**。
  BMP 转成 PNG（Typst 不支持 BMP，素材库支持）；SVG 用 usvg 校验。引用缺失、远程 URL、坏文件都记进
  `missing_assets`（页号 + 原因），模板显示带引用名的占位图。Typst 只看到按序号命名的虚拟路径和像素尺寸，
  看不到引用字符串本身。只有会显示图片的模板（`assets.rs` 的 `IMAGE_KINDS`，目前只有 `spotlight`）才解析图片：
  蓝图在切换 kind 时会保留 `image` 字段，若对所有页面都解析，会白白读盘解码，还会误报缺图。
- `spotlight` 与屏幕版的有意差异：`images` 在屏幕上是轮播，PDF 里改为平铺（最多 6 张，超出的显示 `+N`）；
  位图不会放大到超过原始像素尺寸（与 `object-contain max-w-full max-h-full` 一致），SVG 可以自由缩放；
  侧栏用 `fit-height` 实测缩放，取代按字数分档的 `sideScale`。
- 模板（`typst/`）：`main.typ` 负责分发；`lib/base.typ` 放单位、字号体系、富文本、图标和自动缩放；
  `lib/components.typ` 放标题、页面骨架、卡片和色调；`themes/<id>.typ` 是主题 token（取自对应的主题 CSS）；
  每个版式一个 `layouts/<kind>.typ`。
- 主题接口：每个 `themes/<id>.typ` 导出一个 `theme` 字典（paper/card/ink/muted/edge/primary 系、media-bg、stripe、
  sheet 系、lead 系等）加 `background(w, h)` 页面背景绘制函数。子字典固定为 `radius`（card/sheet/pill）、
  `tracking`（title/label）、`deco`（ruled/tape/double-frame/certificate/stamp/section-num 开关）和
  `tones`（neutral/blue/green/amber/red，各含 solid/text/bg/border/contrast，对应屏幕版 `toneVars()` 的
  `--*-bg/--*-border/--*-light/--*-contrast/--*`）。键集合由 `themes/_check.typ` 编译期校验
  （`Renderer::check_theme_interface`，有单测守护），新增键要所有主题一起加。
- 主题分支：版式里优先用 `deco` 开关和 token 表达差异；不够时再按主题的 `family` 分支（`"notes"` /
  `"terminal"` / `"glass"` / `"editorial"`，对应屏幕版 CSS：`notes-layouts.css`、`terminal.css`、无专属版式 CSS、
  `editorial.css`）。`"editorial"` 目前**同时让 `is-terminal(ctx)` 为真**（第一期借用终端几何），
  只在 `is-editorial(ctx)` 处另走分支：墨色标题线、实心钴蓝徽章、双色大标题。
  `is-terminal(ctx)`（终端 CSS 把所有元素拍平成面板）和 `is-glass(ctx)`（模板原始样式生效：玻璃卡、999px 胶囊、
  primary→透明的渐变分隔线——`divider-line()` 用渐变 rect 画，Typst 的 line 带不了渐变）都读 `family`，
  **不要按主题 id 判断**：新主题只要声明所属 family 就会走对分支，`_check.typ` 会校验它的取值。
- 约定：
  - 尺寸按 Vue 画布的 CSS px 书写，`px = 0.75pt`，16:9 页面正好是 13.333in × 7.5in。
  - 字号来自 `type-sizes(scale)`（移植了 `contentLoad` / `--type-scale`）。
  - 文本用 `line-box` 模拟 CSS 行高。
  - 模型输出的字符串一律经过 `rich()` 或作为纯文本渲染，**绝不拼成 Typst 标记**（有注入测试守护）。
  - 垂直居中用 `center-v`，**不要用 `align(horizon)`**：它会被继承进每个子网格单元格。
  - 内容自适应用 `fit-height`：先测量，再二分搜索缩放系数（下限 0.4）。仍放不下时裁切，并发出
    `<slide-overflow>` 标记，Rust 侧在 `Rendered.overflows` 里报告。一页幻灯片必须恰好是一个 PDF 页面。
    裁切时内层块必须拿到实测的完整高度并设为 `breakable: false`，否则可分页的子块会被拆成片段叠在一起。
  - 布局参数**不要命名成 `h` / `v`**：会遮蔽 Typst 的 `h()` / `v()` 函数（报错 `expected function, found length`）。
  - 代码块里，**以 `+` 开头的续行会被当成新语句**（报错 `cannot join length with length`），多行算式要用括号包起来。
  - 屏幕版的 `flex justify-between` 两端文字**不要译成 `grid(columns: (1fr, auto))`**：`auto` 列先拿走右侧文字的
    完整自然宽度，右侧一长，左侧就被挤成一字一行（`split_layers` 层级条曾因此出错）。按 CSS `flex-shrink` 的
    做法实测两侧自然宽度、按比例收缩（见 `split-layers.typ` 的 `layer-row`）。`auto` 列只放图标、序号这类定宽内容。
  - 局部辅助函数**不要命名成 `center` / `left` / `top` 等对齐值**：会遮蔽内置的对齐常量，`align(center, …)`
    随即报错 `expected content, found function`。
  - 比例一律用 `aspect-of(ctx)`（由画布宽度推出），**不要读页面自己的 `aspect_ratio` 字段**：旧数据或手改过的
    页面可能没有这个字段，此时 32:9 页面会按 16:9 排版；`none in 字典` 还会直接编译失败。
  - 版式要超出内容区（如 `section_intro` 上下各外扩 4px）时，用 `place` 定位，不要放进文档流：
    哪怕在文档流里只多占 4px，也会被挤出一个新页面。
- 应用接入：
  - Tauri 命令 `export_pdf(project_id, slides?, path, theme)`：传了 `slides` 就导出它（编辑器传的是
    **已应用**的牌组，与预览、演示一致）；不传就读数据库里保存的蓝图。字体取「设置 → 外观与字体」的
    6 个角色。`Renderer` 用 `OnceLock` 在进程内只建一次，首次导出要扫描系统字体（约 0.5 秒），
    渲染放在 `spawn_blocking` 里执行。
  - 素材映射在 `export_assets`：项目文件来自 `assets::project_file_refs`，它和生成流程的校验集合
    `collect_assets` 共用同一份写法规则，素材库是 `media:<id>` → `<media_dir>/<storage_path>`。
    **不要另写一套引用规则**，否则生成时合法的图片可能在导出时找不到。
  - 返回 `PdfExportReport`：占位页（模板还没有 PDF 版式）、溢出裁切页、缺失图片，由前端弹窗展示。
    主题 id 不在 `SUPPORTED_THEMES` 时回落到便签，`theme_fallback` 字段记录请求的原 id。
    另有 `reveal_exported_file` 命令，在文件管理器中选中导出的文件。
  - 前端入口：编辑器预览栏的「导出 PDF」按钮（`Step3Editor` 的 `export-pdf` 事件），以及首页导出弹窗
    （`ExportDialog`）里的「PDF 文档」选项。导出状态由 `composables/usePdfExport.ts` 全局共享，进度和结果
    由 `components/PdfExportDialog.vue` 显示，它需要挂在用到导出的 view 里。
  - 真机验证：用环境变量打开 WebView2 调试端口，再通过 CDP 驱动窗口（Node 22 自带 WebSocket，
    不用装任何包）：

    ```powershell
    $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = '--remote-debugging-port=9223'; bunx tauri dev
    ```

    之后从 `http://127.0.0.1:9223/json` 拿到页面的 `webSocketDebuggerUrl`，就可以发
    `Runtime.evaluate` / `Page.captureScreenshot`。
    保存对话框是系统原生窗口，CDP 操作不到，需要人工点一下；在页面里覆盖 `__TAURI_INTERNALS__.invoke`
    来跳过它的做法**实测无效**。
- 新增一个版式：写 `layouts/<kind>.typ`，在 `main.typ` 的 `layouts` 表登记，在 `lib.rs` 的 `TEMPLATES` 和
  `SUPPORTED_KINDS` 里各加一项，并确保 `fixtures/all-layout-blueprints.json` 里有该 kind 的页面（有单测检查）。
- 新增一个主题：写 `themes/<id>.typ`（theme 字典 + `background(w, h)`，键集合照抄现有主题，`family` 填它
  所属的 CSS 层），在 `main.typ`
  的 `themes` 表登记，在 `lib.rs` 的 `TEMPLATES` 和 `SUPPORTED_THEMES` 里各加一项，并在 `themes/_check.typ`
  里加 import（键集合校验会立刻发现缺漏）。token 值可以从同名主题的 CSS 变量抄（Studio 外壳也用它们），然后用
  `render_fixture --theme <id> --png` 逐 kind 目检（Vue 对照页已随模板删除，历史版本在 git 的 `experiments/typst/preview/`）。
- 验证：

```bash
cargo test -p slide-typst
cargo run -p slide-typst --example render_fixture -- --png
```

  第二条输出到 `target/slide-typst/`（PDF 加逐页 PNG）。还支持这些参数：`--kinds all`（包含占位页）、
  `--aspect ratio_32x9`、`--assets <manifest.json>`（图片清单，格式为 `{ "<引用>": "<相对清单的路径>" }`），
  以及指定输入：`crates/slide-typst/tests/fixtures/stress.json <out-dir>`（压力用例）、
  `crates/slide-typst/tests/fixtures/spotlight.json`（图片用例，配合 `--assets crates/slide-typst/tests/fixtures/assets.json`，
  覆盖 PNG/JPG/WebP/BMP/SVG、多图、缺图、坏图）。
- 移植时与旧 Vue 版（当时的屏幕版）有意不同的地方，现在屏幕上也是这样：Vue 的时间线在卡片较高时，节点会偏离时间轴（实测偶数列 y=407、奇数列 y=328，轴在 y=376），
  Typst 版按两侧最高的卡片计算上下区域，保证节点都在轴上；spotlight 的多图改为平铺；所有「按字数估算」的
  收缩规则都改成了实测；`center_grid` 的卡片区宽度上限（屏幕版固定 `max-w-2xl` 即 672px）在 32:9 / 48:9 下
  按 2 / 3 倍放宽，否则 8 列、12 列会被挤成细条；`outcome_grid` 卡片没有 `tag` 时不画空标签。
  按主题分：
  - 便签·稿纸：基本没有省略，装饰（稿线、纸胶带、双线框、证书框）全画。
  - 终端·纸/墨：玻璃阴影、inset 高光和 backdrop-filter 省略（终端卡片本来就是纯色面板）；动画一律取静止态。
  - 报刊·钴蓝：屏幕上卡片是「上方一条细线、无框」的报刊分栏，PDF 第一期走终端几何，卡片仍画细线全框
    （与纸同色、不显底）；Typst 侧不少 `else { 2 * px }` 硬编码圆角也保留着。其余（顶部 6px 钴蓝条、2px 墨色
    标题线、浅蓝副标题带、实心钴蓝徽章、双色封面/结尾标题）两边一致。
  - 暗·琥珀/星图：backdrop-filter 的模糊/饱和度丢弃，玻璃卡画半透明平铺填充（恰好等于 GlassCard 的内联样式）；
    `.slide-shell__ambient` 的动画模糊光斑按 preset 0 位置画静态径向渐斑（PDF 无模糊，且背景函数拿不到
    每页的预设选择）；`--shell-frame` 顶部约 1% 白的光泽低于可见阈值，省略；`.section-num` 水印在屏幕上被卡片的
    backdrop-blur 糊掉，PDF 里会透过半透明卡显出来；phase-chip/step-dot 的 86%→62% mix 渐变画成平的 tone.solid；
    tl-node 的 glow 阴影、phase-head::after / steps-wrap__glow 模糊光斑省略。closing 的 kicker/subtitle/note/mark
    照抄了模板里硬编码的 slate/琥珀色（屏幕在这些主题下同样不随主题变量变）。
- 数学公式（`$...$` / `$$...$$`）：PDF 由 `src/math.rs` 经 mitex 转成 `\u{1}I…\u{2}` 哨兵段、`rich()` 里 `eval`；
  原生界面的第一步预览由 `Renderer::render_formula` 出透明底 PNG 内嵌（`apps/huanshu-native/src/formulas.rs`
  的 TextView 插件，出图在后台、按「公式 + 行内/块级 + 颜色 + 字号」缓存，坏公式显示红色原文）。屏幕公式与
  幻灯片、PDF 是同一套 Typst 排版，**有意不用 KaTeX**（Vue 模板的 `markdown-it-katex` 已随模板删除）。
  三处定界规则必须一致：`math.rs`（PDF）、`generator/utils.rs`（生成的公式保护）、
  `huanshu_core::markdown_tools::{inline_math_content_ok, inline_math_close_ok, display_math_content_ok}`
  （预览插件据此后退不合法的 `$…$`，价格类文本不被吞；markdown-rs 的解析器自己做记号化，逐字对齐以 PDF 侧为准）。
  - Typst 的 `eval` 出错无法捕获，一个坏公式会让整份导出失败：`Renderer::failing_math` 先用 `math-check.typ`
    试编译全部公式（一次），失败时逐个定位，被拒的公式退回原文。mitex 能转换但 Typst 会拒的例子：`\frac{1}`、`\left(`、`\ref{x}`。
  - 输入字符串里的 `\u{1}`/`\u{2}` 会先被删掉，否则模型文本可以伪造哨兵段直接 `eval` 任意 Typst 代码（有单测）。
  - 内嵌的 mitex spec 在 `standard.typ` 的 `arrow-handle` 打了本地补丁（用 `math.stretch`）：上游依赖的 xarrow 用了
    Typst 0.15 已删除的 `style()`，且是 GPL-3.0，**不要重新 vendor xarrow**。
  - 预览公式按「正文字号 px × 0.75」排成 pt、出图 4 px/pt，显示尺寸 = 像素 ÷ 4 ÷ 0.75（`formulas.rs::logical_px`；
    只 ÷ 4 会把 pt 当 px，公式只有正文的 75%）；行内基线按图高 × 0.8 对齐（启发式）。
- 已知差异：没有阴影和毛玻璃（便签主题本身也基本不用）；`text-wrap: balance`（`base.typ` 的 `balanced`）
  目前只用在 `center_grid` 的居中标题上，其他页面标题仍按普通换行；屏幕版标题还会先分词再换行，PDF 没有；
  屏幕上一行放不下的超长标题会被裁掉（如 cover），PDF 版换行显示；未知图标在 PDF 里渲染为兜底图标
  （屏幕上 unocss 缺图标时不显示）。

### Typst 缩略图（迁移第一步）

迁移分五步完成：缩略图（Step2 看板、演示全览）→ 演示主画面 → Step3 编辑器预览与 Step2 详情 → 图表原生绘制 →
删除 Vue 模板。现在屏幕上每一处幻灯片都是 `components/SlideFrame.vue` 里的一张 Typst PNG，没有别的渲染器兜底。

- 链路：`useTypstThumbnails` → `render_slide_thumbnails`（`commands/thumbnails.rs`）→
  `Renderer::render_png`（**整副编译**，按 `only` 光栅化，返回 `data:image/png` 给 `<img>`）。
  命令复用 `services/export.rs` 的 `renderer()` / `prepare_export()`，主题、比例、字体、素材映射与 PDF 导出一致；
  图表由 Typst 直接绘制，不经过 WebView。启动时 `.setup` 在后台预热字体扫描。
- **页序**：`ctx.index` 决定溢出标记的页号。Step2 生成中有空位时只发已有蓝图组成的子集，Typst 的页序、返回的
  `index` 都是**子集位置**，组合函数再映射回传入数组的下标。
- 缓存键 = 蓝图 JSON + 子集位置 + 主题 id + 字体 + 宽度档位（64px）。响应按**键**落图，不按请求序号整批丢弃：
  后一次请求只含它那一刻变了的页，整批丢弃会让前一批的页永远等不到图（Step2 会一直显示加载底）。
- `enabled()` 为假只是不发请求、保留已有的图（全览关了再开不重渲）。
- **没有图时**：`SlideFrame` 显示加载框（`pending` 持续 300ms 后才淡入「渲染中…」，免得翻页就闪）。命令失败时
  `failed` 为真：有旧图就继续显示旧图、角上给「渲染失败 · 重试」，没有图就显示失败和重试；重试调 `refresh()`。
  **失败不自动重试**：`refresh` 每次都会立起 `dirty`，失败页的键又没落下，若照常接着请求会变成对 Rust 的死循环
  （编译不过的牌组每次都失败）。`placeholder` 页（没有 Typst 版式的 kind）照常显示 Typst 返回的占位图。
- 主题切换、字体变化时旧图一直显示到新图落下（键变了但 `thumbs` 不清），不会闪成加载框；只有 `resetKey` 变化会清空。
- Step2 的 `slideItems` 下标是**内容页序号**（牌组装配后前面有封面+目录、末尾有结尾页），缩略图直接用
  `slideItems` 的蓝图（含取自日志的蓝图），下标就是 `item.index`；不要再从 `props.blueprints` 切片加偏移。
- 性能：release 下 560px 宽每页约 5ms（光栅化）+ 整副增量编译约 20ms。根 `Cargo.toml` 的
  `[profile.dev.package."*"] opt-level = 2` 让依赖（Typst、光栅化）在 dev 构建里也优化：否则 `tauri dev` 下一页要等
  几秒，而现在屏幕上的幻灯片全靠它（加了之后编辑器换页约 0.3 秒、`slide-typst` 渲染测试 24–45 秒 → 7 秒）。
- 位图宽度要按**显示的 CSS 宽度 × dpr** 给：Step2 卡片曾把实测的卡片宽再乘一次页面宽高比，16:9 下多渲 1.8 倍宽、
  48:9 下 5.3 倍，光栅化代价按像素数涨。
- **演示主画面**也走同一个组合函数的另一个实例：`wanted` = 当前页前 1 页到后 3 页、`maxCached: 10`、
  宽度 = 覆盖舞台所需的像素宽（拉伸模式取 `max(舞台宽, 舞台高 × 页宽/页高) × dpr`）。**当前页还没有图时 `wanted`
  只给当前页**：一次请求要等 `only` 里每一页都栅格化完才返回，带上 4 张全屏宽的邻页会让眼前这页多等几倍
  （dev 下进入演示从 6 秒降到 0.5 秒）；落图后再预取窗口。`wanted` 读 `stage.thumbs`，但组合函数初始化时就会调用它，
  所以经 `stageReady` 标志转一手（暂时性死区）。封面/结尾/SWOT 的入场动画、spotlight 轮播、图表动画都没有了。
- `wanted` 只决定请求哪几页，发给 Rust 的仍是整副牌（键里有子集位置，裁剪牌组会让翻页时全部失效）。
  `thumbs` / `lastUsed` 以数组下标为键，`shownKeys` / `pendingKeys` 以子集位置为键，淘汰时要换算。
- **comemo 缓存要回收**：`Renderer::compile` 每次编译后 `typst::comemo::evict(50)`。不回收的话 Typst 的
  记忆化缓存只增不减，应用开得越久内存越大。typst CLI 用 10，这里太紧：缩略图、主画面、PDF 导出交替编译
  不同的牌组，同一页两次编辑之间缓存就被清掉了。

### 自适应缩放（`fit-scale`）

`lib/base.typ` 的 `fit-scale(metric, limit)` 找最大可行的 `f`（区间 ≤ 0.005 停止），每次探测都要把整块内容重新排版：

1. 先测 `f = max`，可行直接返回；
2. 按幂律 `metric ∝ f²` 从不可行端往下猜（瞄准预测点略低处），直到找到一个可行点；
3. 之后在 (ln f, ln metric) 上做 **Illinois 修正的试位法**：同一端连续保留两次就把它的值减半，下一次探测
   会越过边界，而不是从一侧慢慢逼近（纯插值会这样，区间收不拢，探测次数和二分差不多）；
4. **只有**没找到更大的可行点时才测 `min`，它一个人决定返回 none。每次插值探测都夹在区间内侧 10%–90%。

`fit-height` 用它，`center: true` 直接用搜索时测过的高度居中。「单个数值 ≤ 上限」的调用点都用 `fit-scale`；
多条件的 `fits`（split-layers、section-intro 引导卡）和 metric 随 `f` 不单调的 overview 保留二分的 `fit-factor`。

- 与二分版对比：字号略大（更贴近可行边界），折行不变；fixture/stress 两副牌最大平均像素差约 4.6（整行子像素位移）。
- **性能结论**：`feature_grid` 持续追加正文时探测次数从 8–12（第二版插值）降到 5–7，但冷编译耗时与二分版
  打平（约 95–115ms，随机器负载漂移）。新内容的成本主要是首次排版与文字整形，不在搜索次数上；
  编辑器流畅靠的是前端的 `serial` + `progressive`，不是再抠搜索。
- 测量工具：`examples/bench_edit.rs`（`--only clip` 只跑持续追加正文的一段）、`examples/png_diff.rs`
  （`--align` 容忍整页平移）。**性能对比必须每次起新进程、`--rounds 1`**：同一进程里重复同样的内容会全部命中
  comemo 缓存（第三轮的「5 轮中位数」测出来的全是缓存命中，约 43ms）；新旧版本用改代码前复制出来的
  example 程序（模板是 `include_str!` 编进去的），交替多轮、取中位数。「替换一个等长字符」同样只测到缓存。
- 本机基准噪声很大（LM Studio 等负载下同一版本可漂移 2–3 倍），单次结果不能下结论。

### 编辑器实时预览与 Step2 详情（第三步）

- 编辑器只发**当前这一页**（`slides: () => [editorPreviewSlide]`，图在 `thumbs[0]`）：版式只依赖本页数据，单页编译最快。
- `useTypstThumbnails` 的 `serial`（同一时刻一个请求，期间变化合并成尾部请求）+ `progressive`（比画面新的过期图先落）
  让打字时预览每隔一次渲染更新一次、停手后落在最新内容；`resetKey: () => activeSlide` 换页时清空。
  换页作废在途响应靠 **generation 计数**（`resetAll` +1，响应带着发出时的值比较），不能靠键：
  换页后 `latestKeys` 要等下一次 `refresh` 才更新，旧页响应的键此刻仍然「相等」，会把上一页的图落到新页上。
- 没有图时一律显示 `SlideFrame` 的加载框，不放裸的空白（这条规则在有 Vue 兜底的三轮里被违反过两次）。
- 位图一律放在舞台层、铺满舞台，**不要**放进带 `transform: scale()` 的画布：Step2 详情曾把按舞台尺寸写的
  `<img>` 放进 `scale(舞台宽/1280)` 的画布，图被缩两次，只剩左上角四分之一。
- 内容缩到 `fit-min` 仍放不下时，`Thumbnail.overflow` 为真，编辑器在预览下方提示「超出部分被裁切」。

## 多比例幻灯片支持 (16:9 / 32:9 / 48:9)

### 功能说明
- 支持 16:9（标准）、32:9（超宽）、48:9（全景）三种幻灯片比例
- 生成时可选择比例，32:9 和 48:9 会自动调整布局（更多列、更宽的间距）
- **播放时拉伸到全屏**：无论什么比例的幻灯片，播放时都会拉伸填满屏幕
  - 32:9 幻灯片在 16:9 屏幕上播放时会横向压缩（看起来被压扁），这正是投到 32:9 大屏幕上的真实效果

### 相关文件

#### 后端 (Rust)
| 文件 | 改动 |
|------|------|
| `crates/huanshu-core/src/types.rs` | 新增 `AspectRatio` 枚举和辅助方法，`SlideBlueprint` 增加 `aspect_ratio` 字段 |
| `crates/huanshu-core/src/config.rs` | `GenerationConfig` 增加 `aspect_ratio` 字段 |
| `crates/huanshu-core/src/db.rs` | `Project` 增加 `aspect_ratio` 字段，projects 表增加 `aspect_ratio TEXT` 列 |
| `src-tauri/src/commands/projects.rs` | 新增 `update_project_aspect_ratio` 和 `get_project_aspect_ratio` 命令 |
| `src-tauri/src/commands/generate.rs` | `generate_slides` 接受 `aspect_ratio` 参数，传递给 Generator |
| `crates/huanshu-core/src/generator/planning.rs` | 布局规划提示词增加比例相关的布局规则（32:9 用 6-8 列，48:9 用 8-12 列等） |
| `crates/huanshu-core/src/generator/normalize.rs` | `apply_component_defaults` 根据比例调整 `cards.truncate` 等数量限制 |

#### 前端 (TypeScript/Vue)
| 文件 | 改动 |
|------|------|
| `src/components/types.ts` | 新增 `AspectRatio` 类型和 `ASPECT_DIMENSIONS` 常量表 |
| `src/stores/generation.ts` | `generate` 函数接受 `aspectRatio` 参数 |
| `src/views/steps/Step1Prepare.vue` | 新增比例选择器 UI（16:9 / 32:9 / 48:9 Segmented Control） |
| `src/views/PresentationOverlay.vue` | 拉伸到全屏播放：Typst 位图按舞台尺寸非等比铺满 |
| `src/views/steps/Step2Generate.vue` | 卡片与详情按页面比例定框 |
| `src/composables/useSlideEditor.ts` | 编辑器预览根据实际比例计算尺寸 |
| `src/views/steps/Step3Editor.vue` | 编辑器预览舞台按页面比例定尺寸（`editorStageW/H`） |
| `src/style.css` | `.pres-slide, .slidev-slide` 的 width/height 改为 CSS 变量 |

### 实现要点
1. **生成时**：`layout_rules.rs` 按比例屏蔽不适合的模板（`blocked_aspects`）并在布局提示词里加入软偏好
2. **normalize 时**：`apply_component_defaults` 根据比例调整各项数量限制（如 `cards.truncate(4 * cols_multiplier)`）
3. **播放时**：PresentationOverlay 计算 `stretchScale = { scaleX: vw/slideW, scaleY: vh/slideH }`，据此定出舞台尺寸，Typst 位图非等比铺满舞台（拉伸到全屏）


## 加密导出/导入功能 (.keynn)

### 功能说明
- 导出时可选择加密，勾选后需设置密码
- 导入时自动检测文件是否加密，如加密则提示输入密码
- 使用 AES-256-GCM + Argon2id 加密，密码强度需至少8字符且包含大小写字母和数字

### 加密文件格式
加密后的 .keynn 文件结构：
```
header.json (非加密，可读)
├── magic: "KNN2"
├── version: "2"
├── algorithm: "AES-256-GCM"
├── kdf: "Argon2id"
├── salt: "<base64 16字节>"
├── nonce: "<base64 12字节>"
└── test_ciphertext: "<加密的验证数据>"
payload.bin (加密的原始 ZIP 内容)
```

### 相关文件

#### 后端 (Rust)
| 文件 | 改动 |
|------|------|
| `crates/huanshu-core/Cargo.toml` | 新增 `aes-gcm`, `argon2`, `rand`, `base64` 依赖 |
| `crates/huanshu-core/src/crypto.rs` | 新增加密解密模块 `CryptoService` |
| `src-tauri/src/commands/storage.rs` | `export_project` 增加 `encrypted` 和 `password` 参数，`import_project` 增加 `password` 参数 |
| `src-tauri/src/lib.rs` | 注册 `is_encrypted_file` 命令（`crypto` 模块现在在 core） |

#### 前端 (TypeScript/Vue)
| 文件 | 改动 |
|------|------|
| `src/components/PasswordDialog.vue` | 新增密码输入对话框组件（支持加密/解密模式、密码强度指示器） |
| `src/views/HomeView.vue` | 导出按钮增加加密选项弹出层，导入时自动检测加密状态 |
| `src/stores/projects.ts` | 无需改动（密码参数直接透传给 Tauri 命令） |

### 实现要点
1. **加密时**：先创建标准 .keynn ZIP，然后用 Argon2id 从密码派生 32 字节密钥，再用 AES-256-GCM 加密
2. **验证机制**：header 中包含用同一密钥加密的测试字符串，解密时先验证测试字符串是否正确
3. **自动检测**：导入时调用 `is_encrypted_file` 判断是否需要密码
4. **密码强度**：必须包含大小写字母和数字，最少 8 字符

## 打包与发布（Tauri）

### 一键打包（推荐）

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
```

脚本做的事：`bun install` → `bun run build`（前端 → `src-tauri/dist-studio`）→ `bunx tauri build --bundles nsis` → 把两个产物汇总复制到 `dist-package/`（命名与 CI release 一致）：

| 产物 | 路径 | 用途 |
|---|---|---|
| `Huanshu_<version>_x64-setup.exe` | `dist-package/` | NSIS 安装包，currentUser 安装，静默 `/S`，卸载 `/S` |
| `auto-slidev-studio_windows_x64.exe` | `dist-package/` | 免安装单文件，双击即跑 |

参数：`-SkipFrontend`（只改了 Rust，复用已有 dist-studio）、`-NoInstall`（只出 exe）、`-DebugProfile`（debug 编译，编译快、冒烟用）。

### 手动等价命令

```bash
bun run build                                  # 或 bunx vite build（跳过类型检查）
bunx tauri build --bundles nsis                # 自动先跑 beforeBuildCommand
# 产物：target/release/bundle/nsis/Huanshu_<version>_x64-setup.exe
#       target/release/auto-slidev-studio.exe（免安装）
```

### 必须记住的坑

1. **没有 `cargo-tauri`**：`cargo tauri build` 会直接报 `no such command: tauri`。Tauri CLI 走 devDependency，统一用 `bunx tauri`。
2. **构建目录是 `<repo>/target`**：根 `Cargo.toml` 是 workspace（members = src-tauri），不是 `src-tauri/target`。README/SETUP 早期写的 `src-tauri/target/.../bundle/msi/` 是错的，且本项目没有 msi target。
3. **前端产物目录固定为 `src-tauri/dist-studio`**：由 `vite.config.ts` 的 `build.outDir` 决定，与 `tauri.conf.json` 的 `build.frontendDist` 对应。改了 vite outDir 就必须同步改 tauri.conf.json。
4. **Windows 只能出 nsis**：`bundle.targets` 里的 `appimage`/`deb` 需要 Linux 宿主，本机构建要显式 `--bundles nsis`。Linux/发布产物交给 `.github/workflows/release.yml`（tag `v*` 触发，tauri-action）。
5. **`beforeBuildCommand` 是 `bun run build`**：不是 `bun build`（`bun build` 是 Bun 自带 bundler，不会跑 vue-tsc + vite）。前端类型错误会直接让 tauri build 失败。
6. **PowerShell 脚本必须 ASCII-only**：Windows PowerShell 5.1 把无 BOM 的 .ps1 按 ANSI(GBK) 读，中文注释会变成乱码并引发 parse error。中文只写进 `.md` 文档。
7. **打包前必须关掉正在运行的 幻述**：Windows 上运行中的 exe 是活映像，cargo 覆盖时直接失败：
   `error: failed to remove file ... target\release\auto-slidev-studio.exe` + `拒绝访问。 (os error 5)`。
   （注意：Windows 允许重命名运行中的 exe，所以“文件能改名”不代表可以重建——别看错现象。）
   `scripts/package.ps1` 已内置前置检查，会直接报出占用进程 pid，不会白等一趟完整编译。
   优雅关闭：`(Get-Process auto-slidev-studio).CloseMainWindow()`（`decorations:false` 的窗口同样响应 WM_CLOSE），不行再 `Stop-Process -Force`。
8. **`src-tauri/Cargo.toml` 的 `[features] custom-protocol` 不能删**：Tauri v2 靠这个 feature 决定内嵌前端资源还是走 `devUrl`。删掉后 `cargo build --release` 照样编得过，但产物是 **dev 模式**（前端资源不内嵌，运行时去连 `http://localhost:5173`），双击只有 `ERR_CONNECTION_REFUSED`。用 `bunx tauri build` 就不会踩到。
   构建后自检：`grep -c "rustc-cfg=dev" target/release/build/auto-slidev-studio-*/output` 应全为 `0`，出现 `1` 说明这次是 dev 构建。

### 打包后必须验证（各步骤实测命令）

```powershell
# 1) 冒烟：直接跑未安装二进制，看窗口是否真的起来
$p = Start-Process D:\huanshu\target\release\auto-slidev-studio.exe -PassThru
Start-Sleep 12; $p.Refresh()
if ($p.HasExited) { "EXITED $($p.ExitCode)" } else { "RUNNING $($p.MainWindowTitle)" }

# 2) 安装包静默安装 + 校验落盘
Start-Process dist-package\Huanshu_0.1.0_x64-setup.exe -ArgumentList '/S' -Wait   # 落到 %LOCALAPPDATA%\Huanshu
Start-Process "$env:LOCALAPPDATA\Huanshu\auto-slidev-studio.exe" -PassThru       # 再跑一次

# 3) 卸载清理（避免留残留影响下次验证）
Start-Process "$env:LOCALAPPDATA\Huanshu\uninstall.exe" -ArgumentList '/S' -Wait
```

预期：`MainWindowTitle` 为 `幻述`。窗口 `decorations: false`，标题来自 `tauri.conf.json` 的 `app.windows[0].title`。

### 运行前置与分发注意

- 运行时依赖本机 **WebView2 运行时**（Win10+ 一般已预装；可用 `reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" /v pv` 查版本）。
- `webviewInstallMode` 当前是 `downloadBootstrapper`：目标机缺 WebView2 时需联网下载。要纯离线装机改成 `embedBootstrapper` 或 `offlineInstaller`（体积显著增大）。
- 安装包未签名，首次安装会有 SmartScreen 提示。
- LM Studio 是外部依赖，不打包；装完需在应用「设置」里填模型名，否则生成流程不可用。
- `build.rs` 依赖 `node_modules/@iconify-json/carbon`，所以打包前必须 `bun install`。

## 打包与发布（原生版，`apps/huanshu-native`）

### 一键打包

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package-native.ps1
```

`cargo build --release -p huanshu-native` → 产物汇总到 `dist-package/`：

| 产物 | 用途 |
|---|---|
| `huanshu-native_windows_x64.exe` | 免安装单文件，双击即跑 |
| `HuanshuNative_<version>_x64-setup.exe` | NSIS 安装包（currentUser 静默 `/S`，卸载 `uninstall.exe /S`） |

找不到 makensis（本机 Tauri 自带 `%LOCALAPPDATA%\tauri\NSIS`）时只出免安装版并提示，不失败。

### 随 tag 发布

`.github/workflows/release.yml` 的 `native-windows` job 在 `windows-latest` 上跑同一个脚本，
产物追加到同一个 Release：`needs: release`（tauri-action 负责创建 Release，两个 job 同时创建会撞车），
先 `choco install nsis`——**CI 上没有 Tauri 自带的那份 NSIS，不装就只能出免安装 exe**。
发布说明由 tauri-action 先写一遍（只有 Tauri 产物），native job 再用 `.github/release-notes.md`
重写一遍；说明文字放文件而不是内联，`.ps1` 里内联中文会被 ANSI 读成乱码。Linux 暂时只发 Tauri 版。

### 与 Tauri 版的关系

- 安装目录 `%LOCALAPPDATA%\HuanshuNative`、产品/卸载项「幻述（原生版）」，与 Tauri 版
  （`%LOCALAPPDATA%\Huanshu`、「幻述」）**完全分开**，不会互相覆盖或卸载。
- 两版**共用同一个数据目录**（不设 `HUANSHU_HOME` 时的真实数据路径），这是有意的：原生版要接替 Tauri 版。
- 版本号三处一致：`apps/huanshu-native/Cargo.toml`（`CARGO_PKG_VERSION` → exe 版本资源）、
  `src-tauri/tauri.conf.json`、`scripts/package-native.ps1` 打的安装包文件名。 bump 版本时一起改。

### 必须记住的坑

1. **release 不弹控制台**：`main.rs` 的 `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]`。
   Windows 子系统进程在父进程重定向句柄时 stderr 仍可读——`tools/drive.ps1` 依赖的
   `[coldstart]` 行（`Start-Process -RedirectStandardError`）与 `--export` 都不受影响（已实测）。
2. **exe 图标与版本资源在 `build.rs`**（winresource）：ico 是 `src-tauri/icons/icon.ico`，
   多分辨率原样嵌入（id 1）；GPUI 的 Windows 后端从 `MAKEINTRESOURCE(1)` 读窗口/任务栏图标，
   没有 per-window 图标 API。中文产品名能正确进资源：winresource 写的 .rc 是 UTF-8 且带
   `#pragma code_page(65001)`，MSVC `rc.exe` 认。
3. **静态 CRT**：release 下 `/DEFAULTLIB:libcmt.lib` 等三条 link-arg（对齐 tauri_build 的做法），
   免安装版在干净机器上不需要 VC++ Redistributable；debug 保留动态 CRT（libcmtd 与预编译依赖冲突）。
4. **NSIS 脚本 ASCII-only**：PowerShell 5.1 与 makensis 都按 ANSI 读脚本。中文产品名「幻述（原生版）」
   在 `package-native.ps1` 里用码点构造、经 makensis 命令行 `-D` 传入（Windows 命令行是 UTF-16）。
5. 打包前关掉运行中的 huanshu-native（活映像占用，cargo 覆盖失败）；脚本内置 fail-fast 列出 pid。
6. **真机验证只在沙箱**（`HUANSHU_HOME`）：免安装 exe 与安装版都一样。安装版验收一轮
   「静默装 → 带沙箱启动 → 静默卸 → 查残留」，装/卸各一次就够，别反复。


<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
