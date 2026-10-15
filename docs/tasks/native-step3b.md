# 任务：原生界面第五轮——第三步「编辑完善」（二）＋生成与当前项目解耦（第十三步）

你在仓库 `E:\huanshu`（幻述）里工作。原生界面第三步的编辑器主体上一轮已做完并审过（`feat/native-step3a`）。本轮四件事：

- **A. core**：生成与单页重试不再依赖「当前打开的项目」（含 Tauri 侧的配套改动）；
- **B. 原生**：生成状态改用 A 的新接口，第三步只在**本项目**生成时只读，打开项目时预热图标向量；
- **C. 原生全局**：界面文字一律常规体（原版把外壳字重拍平了，原生版到处在用粗体）；
- **D. 第三步剩下的四块**：图标助手、素材插图、图表数据（JSON）框、高级 JSON 模式。

开始前**必须**先读完：

1. `apps/huanshu-native/README.md`——框架与「写页面的规矩」（逐条遵守）；
2. `docs/native/parity.md`——本轮把「第三步」剩下的四行做成 ✅，「工作流」里「生成中只读」那行按 B 节改；
3. `CLAUDE.md` 开头三节（尤其「原生界面」里生成状态、`commit_deck`、**core 的生成依赖当前打开的项目**三条）与
   「图表页（`chart_focus`）」一节里图表 JSON 框的行为说明；
4. 现有代码：`pages/workflow/step3.rs`（四个 `todo_block` 的位置）、`pages/workflow/mod.rs`（`editing_locked`、`commit_deck`、
   `load_project`）、`generation.rs`（`GenerationStore::generate` 里取最终牌组的那段）、`pages/workflow/step1.rs`（素材缩略图 `media_thumb`）；
5. 原版源码：`src/views/steps/Step3Editor.vue`（`syncSemanticErrors` / `advancedChartErrors`、图表 JSON 那一整段、`recommendIcon` /
   `applySuggestedIcon`、`mediaCandidates` / `setPrimaryImage` / `appendGalleryImage`、模板里「图片素材」「图标助手」「图表数据（JSON）」
   「高级 Blueprint JSON」四段）、`src/composables/useSlideEditor.ts`（`jsonText` / `onEditorJsonInput` / `formatEditorJson` /
   `applyEditorChanges` / `onEditorJsonKeydown`）、`src/views/WorkflowView.vue`（`onMounted` 里的 `ensure_icon_embeddings`）、
   `src/stores/generation.ts`（两处 `invoke('get_blueprints')`）、`src/themes/typography.css`（`#app *`）；
6. core：`services/generation.rs` 的 `generate_slides`、`run_pipeline` 组装那段（「Keep the assembled deck…」）、`retry_generation_page`、
   `get_retry_layouts`、`get_blueprints`、`recommend_icons_for_query`、`ensure_icon_embeddings`；`services/projects.rs::open_project`；
   `chart_check.rs`（`chart_problems_json`、`message_zh`）。

## 原则（同前几轮）

- **复刻**：布局、文案逐字、交互、规则以原版为准；可按 GPUI 特点调整控件形态，但**不能少功能**。拿不准时读原版代码。
- **不重写框架**：缺部件往 `widgets.rs` **加**；改框架接口要在汇报第 8 节说明理由。
- **数据只经 `huanshu_core::services`**；新规则放 core（带单测），界面里不写规则。

## 前几轮审查发现的问题（不要再犯）

1. **真机验证碰了真实数据**：只用 `tools/drive.ps1` / `tools/shots.ps1`。动手前备份沙箱（→ `target/sandbox-home.bak7`），然后 `seed_demo --force`。
   驱动脚本超时退出时窗口可能还开着（`--quit-after 300`），下一次 `cargo build` 会因 exe 被占用失败：先关掉残留的 `huanshu-native` 进程。
2. **UI 线程里的延时用了 tokio**：界面里的延时一律 GPUI 的 `background_executor().timer(..)`。
3. **「进行中」标志没复位导致永久卡住**：每个标志（推荐中、校验中、保存中）的每条出口都要复位，汇报里逐条列出。
4. **Vue 文件里注释放错位置**：上一轮把 `//` 注释写在 `<script setup>` 之前（块外）。改 `.vue` 时注释放进 `<script>` 块里，改完跑 `bunx vue-tsc --noEmit`。
5. **汇报要诚实**：没真机验证的写「未验证」。

## A. core：生成与单页重试不依赖当前打开的项目

现状（`CLAUDE.md` 有记）：`AppState.blueprints` 是「当前打开的项目」的牌组，`active_project_id` 是哪个项目打开着。生成流水线、单页重试、
`get_retry_layouts` 都读它们，所以**生成中打开别的项目**（原生版允许离开项目再回来）会让流水线在组装时以「当前项目已切换」失败；
单页重试中离开项目同样失败。上一轮已把生成结束时的落库改成写开始时的项目。本轮把整条链解耦，**不变量**：

1. 一次生成**属于它开始时的项目**（`generation_runs.project_id`）。它的牌组只写进**那个项目**的数据库行；`AppState.blueprints` 只在那个项目
   **此刻仍打开着**时才跟着更新。打开别的项目不会让它失败，也不会让它写错地方；
2. 「这次运行组装出来的牌组」要能取回：新增 `get_generation_deck(state, project_id) -> Result<Vec<SlideBlueprint>>`——取该项目**最近一次运行**
   （全局同一时刻只有一次生成，刚结束的那次就是最近一次），运行**到达过组装**时返回该项目在库里的牌组，否则返回空（没组装就失败时库里还是上一次的旧牌组，**不能**被当成这次的结果）。「到达过组装」要有
   明确记录：组装落库后写一条日志（`stage = "assembling"`、`kind = "assembled"`、标题「已组装牌组」、摘要「共 N 张幻灯片」、`important = false`），
   判断就查这条；
3. 单页重试的底稿是**运行所属项目在库里的牌组**（要求该运行到达过组装、页数 = 规划页数 + 3），结果写回那个项目；`AppState.blueprints` 只在该项目
   打开着时更新。「最新一次运行」按 `run.project_id` 查（现在按 `active_project_id`）。去掉「当前项目已切换，重试结果未写入」与
   「生成记录不属于当前项目」两处判断；
4. `get_retry_layouts` 同样按运行所属项目取比例与校验，不看当前项目；
5. `generation_running`（全局同一时刻只许一次生成或重试）**保持不变**；`repair_slide`、`get_blueprints` 的语义不变（编辑器的「当前牌组」）。

**Tauri 侧配套（必须做，否则会写坏数据）**：原版 store 生成结束后 `invoke('get_blueprints')` 取「当前牌组」，`WorkflowView` 再把它存进**发起生成的那个
组件的** `projectId`。解耦之后用户若已打开别的项目，这里拿到的是别的项目的牌组，会被存进发起生成的项目。所以：

- 新增命令 `get_generation_deck(project_id)`（`src-tauri/src/commands/generate.rs`，只做转发，并在 `lib.rs` 注册）；
- `src/stores/generation.ts` 里两处 `invoke('get_blueprints')` 改成按发起生成的项目取（`activeProjectId.value`；为 `null` 时——原版 `/new` 还没落库——
  保留 `get_blueprints`）。**不要**用 `currentRun.value?.id`：运行 id 来自 `gen:log` 事件，事件与 `invoke` 的返回没有先后保证；
- 改完跑 `bunx vue-tsc --noEmit`，Tauri 版不做真机验证，但要在汇报里写清这两处改动。

单测（core，不需要 LM Studio）：用临时库造「项目 A 的一次运行 + 日志 + 牌组」，再 `open_project(B)`，验证
`get_generation_deck(A)` 只在 A 的最近一次运行有 `assembled` 日志时返回 A 的牌组、`get_retry_layouts` 在 B 打开时仍能读 A 的运行、重试的前置检查
（把重试里「取底稿 + 校验」这段抽成同步函数再测）在 B 打开时通过、并拒绝没到达组装的运行。流水线本身要 LLM，写进人工验收清单。

## B. 原生：接上 A，放开只读

- `GenerationStore::generate` 的异步块里，`generate_slides` 返回后用 `get_generation_deck(project_id)` 取牌组（成功与失败两条路都换掉 `get_blueprints`；
  不要等事件里的运行 id，事件可能比结果晚到）；
- `WorkflowPage::editing_locked` 改成**只在本项目生成 / 重试时**为真（store 正持有本项目且 `busy()`）。别的项目在生成时第三步照常可编辑；
  `GenerationStore::set_deck` 的条件不变（写不进去时模型是唯一状态，`commit_deck` 照常落库）。parity「生成中只读」那行同步改说明；
- 打开项目时预热图标向量（原版 `WorkflowView.onMounted` 的 `invoke('ensure_icon_embeddings').catch(() => {})`）：`run_async`，结果忽略。

## C. 原生全局：界面文字一律常规体

原版 `src/themes/typography.css` 的 `#app * { font-weight: normal !important; }` 把 Studio 外壳的字重全部拍平（要粗体须选粗体字族），
原生版却有约 55 处 `FontWeight::BOLD` / `SEMIBOLD`（`grep -rn "FontWeight::" apps/huanshu-native/src`）。全部去掉，并把这条写进
README「写页面的规矩」。GPUI Kit 组件内部自带的字重（对话框标题、按钮）如果改不了，汇报里列出来。改完重拍 `tools/shots.ps1` 的全部截图，
挑首页、第二步、第三步与 `docs/native/reference/` 对比字重。

## D. 第三步剩下的四块（`pages/workflow/step3.rs`）

### D1. 图标助手（列表区 / 对象区子列表里有 `icon_query_keys` 的卡片）

原版 `recommendIcon` / `applySuggestedIcon` 逐条照做：查询词 = 该卡片 `icon_query_keys` 各字段的值 trim 后非空的用空格连接，空就不发；
`recommend_icons_for_query(query, Some(6))`（async → `run_async`）；进行中按钮显示「推荐中…」（`renew` 图标转动）并禁用，否则「AI 推荐图标」（`magic-wand`）；
成功后记下这张卡片的候选、**第一个候选直接写进 `{卡片路径}.icon`**；失败候选记为空。候选显示成小片：图标预览（`carbon(icon)`）+ 去掉 `i-carbon:`
前缀的名字，点一下写进 `.icon`。「推荐中」的键是卡片路径，每条出口（成功、失败、换页、离开页面）都复位；换页 / 列表增删移后迟到的结果作废（带上发起时的
页与表单代次）。

### D2. 素材插图（「图片素材」组）

显示条件照原版：kind 的 `fields` 含 `images`，或当前页 `image` 非空。主图 / 图集的文本输入已有；补上素材条：项目素材里 `type == image` 的每一项一张小卡
（缩略图，做法同 `step1.rs` 的 `media_thumb`；名字；「设为主图」→ `image = ref`；图集可用时再加「加入图集」→ 追加到 `images`，已有就不重复）。
写草稿走 `slide_edit`，不手写路径逻辑。素材列表来自 `ProjectModel.media`。

### D3. 图表数据（JSON）框（`chart_focus`）

行为以原版与 `CLAUDE.md`「图表页」一节为准，逐条：

- 框的内容 = 草稿 `chart` 的两空格缩进 JSON；**框有焦点时不被草稿的变化覆盖**；换页 / 换版式时重置；
- 输入：语法错 → 立即显示「JSON 语法错误：{原因}」、作废等待中的校验；语法对 → 150ms 防抖后校验（`chart_check::chart_problems_json` + `message_zh`，
  多条用「；」连接），**只有最新一次输入**的结果生效，且只写回发起时所在的那一页；等待期间提示文字是「正在校验…」，否则是
  「type / data / encoding / series，校验通过才会应用到预览」；
- **只有通过校验的 JSON 才写回草稿**（`chart` 字段，走 `slide_edit::set_at_path`），未通过时预览保持上一次合法的图表；
- 失焦：还在等防抖就**立即校验**（粘贴后马上点别处不能丢）；没有错误时把框规范化成草稿里的实际内容；有错误时保留输入让用户继续改；
- 输入框用多行等宽（12 行）。

### D4. 高级 JSON 模式

把上一轮的「打开高级 JSON」占位换成原版的「高级 Blueprint JSON」段。状态照 `useSlideEditor`：

- `json_text`：每次表单改动 / 换版式 / 换页 / 应用后 = `filter_slide_json(草稿)` 的两空格缩进 JSON（原版 `replaceEditorDraft`）；
- 在 JSON 框里输入：能解析 → 草稿 = 解析结果（**不过滤**）、清错误、`has_unsaved = true`、表单按新草稿重建；解析失败 → 记错误，草稿不动；
- **应用更改写回的是 `json_text` 解析出来的那一页**（原版 `applyEditorChanges` 是 `JSON.parse(jsonText)`）：平时它就是过滤后的草稿，用户在 JSON 里
  手加的字段会原样保存。有 JSON 错误时应用按钮禁用、Ctrl+S 无效；
- 「格式化」：能解析就按 `replaceEditorDraft` 规整（会经过 `filter_slide_json`），不能解析就不动；
- 编辑框：多行、等宽、显示行号（GPUI Kit `InputState` 的 `code_editor` / `line_number`）；Tab 插入两个空格；
- 语法错误显示「第 N 行：{原因}」（从 serde_json 的错误里取行号；原版高亮行号栏那一行，GPUI 做不到就只给文字，写进第 8 节）；
- 语义错误（只在没有语法错误时显示，每条前面 `warning` 图标）：把原版 `syncSemanticErrors` 搬进 core（`slide_edit::semantic_errors(schema, value)`，
  文案逐字：`direction "x" 无效，应为 horizontal 或 vertical`、`{路径}.{tone 键} "x" 无效`（路径规则照 `walkTone`，根是 `root`）、
  `chart_focus 页缺少 chart（图表数据）`），并在上一轮的 bun 脚本里加上它、扩充 `slide-edit.json` 快照；图表的语义错误（`chart：{问题}`）
  跟 D3 一样 150ms 防抖、只认最新一次。

## 分支与提交

从 `feat/native-step3a` 开 `feat/native-step3b`，五个提交：

1. `fix(core): generation and page retry no longer depend on the open project`（A 节 core、单测、Tauri 命令与 store 两处）
2. `feat(native): generation deck by run; lock editing only for this project's generation`（B 节）
3. `style(native): regular weight for all interface text, as in the original`（C 节，含重拍的截图）
4. `feat(native): step 3 — icon helper, media strip, chart JSON, advanced JSON`（D 节，含 `semantic_errors` 与快照扩充）
5. `docs(native): parity for step 3 part 2`

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo build -p huanshu-native --release
bunx vue-tsc --noEmit
```

真机验证（**全部在沙箱**，先备份再 `seed_demo --force`）：

- 图标助手：在「全部版式 16:9」带图标的卡片上点「AI 推荐图标」。没有 LM Studio / 向量库时应失败而不卡住（按钮恢复、无候选）；有的话贴候选截图，
  并验证第一个候选已写进 `.icon`、点别的候选能换；
- 素材插图：「图片」项目的 spotlight 页：设为主图、加入图集（重复加入不重复），预览更新，应用后 sqlite 只读核对 `image` / `images`；
- 图表 JSON：「图表」项目：改一个数据值 → 150ms 后预览更新；输入半截 JSON → 显示语法错误、预览不坏；输入合法但不通过校验的图表（如缺 encoding 列）→
  显示中文问题、预览不变；**粘贴后立即点别处**（不等 150ms）→ 修改仍生效；
- 高级 JSON：打开 → 内容是过滤后的当前页；改表单 → JSON 跟着变；在 JSON 里改标题 → 预览与表单跟着变；写错一个逗号 → 「第 N 行」、应用按钮禁用；
  写一个无效 tone → 语义错误；手加一个字段并应用 → sqlite 里该字段还在；
- 只读：用 sqlite 直接把某项目的最新运行标成 `running` 不能模拟「正在生成」（那是「已中断」），所以这条写进人工验收清单：生成 A 时打开 B 的第三步可编辑，
  A 的第三步只读；
- 字重：截图对比，列出仍有粗体的地方与原因。

人工验收清单里必须有（需要 LM Studio）：生成项目 A 途中回首页打开项目 B 并在 B 的第三步改一页、应用；A 生成完成后 A 的牌组正确、B 的修改仍在、
B 的牌组没有被 A 覆盖；单页重试途中离开项目，重试结果仍写回原项目。

## 最终汇报（按此格式，不要省略）

1. 五个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）
3. parity 表本轮改动的行（改动前 → 改动后，附依据）
4. A 节：改了哪些函数、新增的日志与接口、单测清单；Tauri 侧的两处改动；`semantic_errors` 的快照怎么扩充的、Rust 与 TS 有没有不一致
5. 截图对比清单与差异（含字重）
6. 真机验证逐项结果（没做的写「未做」）；给用户的人工验收清单
7. 新增 / 修改的 core 函数与单测；有意修改的测试期望值及原因（没有就写「无」）
8. 与本说明或原版不一致的实现及原因；改动了框架接口的地方；每个「进行中」标志的复位出口；没做完或不确定的点
