# 任务：原生界面第三轮——第二步「AI 生成」（第十一步）

你在仓库 `E:\huanshu`（幻述）里工作。原生界面 `apps/huanshu-native` 已完成框架、设置、向导、第一步与工作流（上一轮已审过）。
本轮把**第二步「AI 生成」**补到与原版一致：生成状态、整体进度与五个阶段、看板逐页状态、详情分段、单页重试与换版式重试、
出错后的「返回修改 / 重新生成全部」。开始前**必须**先读完：

1. `apps/huanshu-native/README.md`——框架与「写页面的规矩」（逐条遵守）；
2. `docs/native/parity.md`——本轮要把「第二步」一节全部做成 ✅，并修正「工作流」里与生成相关的几行（见 C 节）；
3. `CLAUDE.md` 开头三节（代码结构、隔离数据目录、原生界面）与「Typst 缩略图」一节（Step2 的下标规则）；
4. 现有代码：`pages/workflow/mod.rs`（`start_generation` / `launch_generation` / `on_gen_event` / `on_generation_finished` / `load_project`）、
   `pages/workflow/step2.rs`（骨架）、`tasks.rs`、`slides.rs`；
5. 原版源码：`src/views/steps/Step2Generate.vue`（全部）、`src/stores/generation.ts`（全部）、`src/utils/generationProgress.ts`、
   `src/components/RetryLayoutDialog.vue`、`src/views/WorkflowView.vue`（`onMounted` 的加载顺序、`steps` 计算属性、`canGenerate`、自动跳转的 `watch`）；
6. core：`services/generation.rs` 的 `generate_slides`、`retry_generation_page`、`get_retry_layouts`、`get_latest_generation_run`、
   `get_generation_logs`、`get_blueprints`，以及流水线里写日志的地方（哪些日志的 `detail` 带 `page_plans` / `layout_plans` / `page_plan` / `blueprint` / `system_prompt` …）；
7. 原版截图：`docs/native/reference/project-step2.png`、`step2-board.png`、`step2-detail.png`。

## 原则（同前几轮）

- **复刻**：布局、文案逐字、交互、规则以原版为准；可按 GPUI 特点调整控件形态，但**不能少功能**。拿不准时读原版代码。
- **不重写框架**：缺部件往 `widgets.rs` **加**；改框架接口要在汇报第 8 节说明理由。
- **数据只经 `huanshu_core::services`**；新规则放 core（带单测），界面里不写规则。

## 前几轮审查发现的问题（不要再犯）

1. **真机验证碰了真实数据**：每次启动前确认 `HUANSHU_HOME=E:\huanshu\target\sandbox-home`，**只用 `tools/drive.ps1` / `tools/shots.ps1`**
   （它们自己设置沙箱），不要直接双击 exe 或不带环境变量 `cargo run`。动手前备份沙箱（`target/sandbox-home` → `target/sandbox-home.bak3`）。
2. **UI 线程里的延时用了 tokio**：上一轮的防抖与自动跳页用了 `tokio::time::sleep`（守卫跨 `.await`，两个计时并发就 panic）。
   界面里的延时一律 `window.background_executor().timer(..)` 或 `cx.background_executor().timer(..)`。
3. **随项目的数据没跟着删**：新增任何按项目存的东西，`delete_project` 要一起删，并加单测。
4. **报告与事实不符**：没验证的写「未验证」；截图逐张亲眼看过再写「一致」。**必须**按文末格式交汇报。
5. **文案凭记忆打**：所有文案从原版复制。注意 Step2 的版式名用的是 `Step2Generate.vue` 自己的 `KIND_LABELS`（「特性格」「焦点例」…），
   **不是** `editor_schema` 的中文名——现在的骨架用错了；而 `RetryLayoutDialog` 用的才是 `editor_schema` 的 label 与 desc。

## 现状里要先纠正的偏差（骨架的问题，不是你的问题，但本轮要改掉）

- **看板只列内容页**：原版 `slideItems` 去掉了封面、目录（前 2 张）和结尾（最后 1 张），下标是内容页序号；骨架列的是整副牌组。
  条目数取 `page_plans`、`layout_plans`、content 日志、`pageStatuses`、内容蓝图五者的最大值（原版 `slideItems`）。
- 顶部标题：生成中是「`{stageLabel}...`」、完成是「生成完成 · N 张幻灯片」（N = 看板条目数）、出错是「生成遇到问题」、否则「准备开始」；
  第二行是 `message`。骨架写的「生成中… / 生成出错」不对。
- 底栏：**生成中不显示**；完成 → 「前往编辑」（有页在重试时禁用）；出错 → 「返回修改」+「重新生成全部」（有页在重试时禁用）。
- 键盘只有 **↑ / ↓**（原版 `handleStepKeydown`；带修饰键或焦点在输入框里时不处理）。parity 表写的 ←/→ 是错的，一并改掉。
- 生成状态挂在 `WorkflowPage` 上：离开项目再回来（或回首页再进），页面重建，事件接收方没了，生成还在跑但界面再也收不到进度；
  而数据库里那次运行停在 `running`，`load_project` 把它显示成「生成中」且永远不会结束。原版的 pinia store 是全局的，没有这个问题。见 B 节。

## A. core：进度规则搬进 Rust

原版 `generationProgress.ts` 与 `Step2Generate.vue` 里有一批纯规则，放进 core 新模块 `huanshu_core::generation_progress`（纯函数，不碰状态），
**逐条照抄原版的分支与边界行为**：

- 类型：`PageProgressStatus { stage, message, previous_stage, waiting }`、`SlideProgressItem { index, page_plan, layout_plan, blueprint, content_log }`
  （`page_plan` / `layout_plan` / `blueprint` 用 `serde_json::Value` 或带 `Option` 字段的宽松结构体，原版字段全部可缺）；
- `slide_items(blueprints, logs, page_statuses)`（原版 `slideItems` + `extractArrayDetail` + `contentLogBySlideIndex` +
  `extractPagePlanFromLog` / `extractLayoutPlanFromLog` / `extractBlueprintFromLog`；首张是 `cover` 时切掉前 2 张与最后 1 张）；
- `resolve_progress_stage`、`page_step_state`、`summarize_progress`（含 `stageCards`、`percent` 四舍五入、`interrupted` 的判定）；
- `slide_status(item, stage, running, page_statuses, retrying_pages) -> (tone, label)`（原版 `slideStatus`，含「等待重试 / 重试中 / 生成中断 / 已中断 / 等待开始 / xx中」）；
- `preferred_slide_index`、`important_log_count`、`prompt_log_count`、`recent_logs`（最后 10 条倒序）；
- 三张文案表：`stage_label`（store 的 `STAGE_LABELS`）、`log_stage_label`（`LOG_STAGE_LABELS`）、`step2_kind_label`（Step2 的 `KIND_LABELS`，空 → 「待定」）；
- `statuses_from_logs(logs)`（`loadLatestRun` 里从 `page_error` / `retry` 日志恢复页状态那段）；
- `apply_page_status_event(statuses, event)`（`gen:page_status` 监听里 `previousStage` 的写法：连续两次 error 保留最早的非 error 阶段）。

**对照测试必须用原版代码生成期望值**（与上一轮 `markdown-tools.json` 同样做法）：一次性 `bun` 脚本放 `target/` 下、不提交，把上述 TS 函数**原样**拷进去，
对一组输入跑一遍，结果写成 `crates/huanshu-core/tests/fixtures/generation-progress.json`；Rust 单测逐条比对。输入至少覆盖：
空牌组、生成中只有 `page_plans`、有 `layout_plans` 无蓝图、部分页 `waiting`、某页 error 且 `previousStage` 各种取值、
连续两次 error、running 为真/假各一遍、`stage === 'error'` 的整体出错、重试中的页、带封面的完整牌组与不带封面的牌组、
日志里 `page_error` 与成功/失败的 `retry`。汇报里写清脚本怎么生成的。

**允许的重复（四）**：Tauri 版的 TypeScript 继续保留，两边文件头互相注释「改一边必须改另一边」，快照测试守住一致性；在 `CLAUDE.md` 的「暂时允许的重复」里**不要**自己加，汇报里提出来由审查者加。

另外两处 core 小改：

- `RetryPageResult`、`RetryLayoutOptions` 的字段现在是私有的，原生界面读不到：改成 `pub`（加 `Debug, Clone`），序列化格式不变（Tauri 版靠它）。
- 沙箱演示数据：`seed_demo` 加一个「生成记录演示」项目，用 `huanshu_core::db` 的 `create_generation_run` / `append_generation_log` /
  `finish_generation_run` 写入**一次以 error 结束的运行**：带 `page_plans` + `layout_plans` 的规划日志、若干页 `content/slide` 日志
  （`detail` 里有 `page_plan`、`layout_plan`、`blueprint`、`system_prompt`、`user_prompt`、`raw_output`、`parsed_output`，字段名照流水线实际写的抄）、
  一页 `page_error`、一页失败的 `retry`，牌组里那页用兜底蓝图。内容全部**自己编**，不要从任何真实数据库拷。这样没有 LM Studio 也能截图验证看板、详情、
  失败段与「选择重试布局」对话框（`get_retry_layouts` 读的就是这些日志）。
- 同一提交里再加一个「长牌组」项目：把「全部版式」的内容页重复到约 60 页内容页（封面、目录、结尾各一张），用来验证看板的可见区要图与内存。
  沙箱里现有最大的牌组只有 21 页，测不出虚拟化。

## B. 原生：全局生成状态（`apps/huanshu-native/src/generation.rs`）

照原版 pinia store 的职责做一个**应用级**的 `GenerationStore`（`Entity`，由 `Shell` 创建并经 `ShellHandle` 取用，或放进 GPUI global——二选一，汇报里说明），
它比任何页面活得久：

- 字段对应原版 store：`project_id`、`stage`、`message`、`progress`、`blueprints: Vec<Option<Value>>`（生成中按下标落位）、`page_statuses`、
  `retrying_pages`、`queued_retries`、`logs`、`current_run`、`running`、`last_error`、`version`（原版 `generationVersion`）。
- 方法对应原版：`generate`（含成功后 `get_blueprints`、保留失败页状态、失败时找回已组装的牌组、`finally` 里执行排队的重试）、
  `retry_page` / `execute_page_retry`（排队规则、`blueprints[index + 2]`、`deck_ready` / `invalid_pages` / `remaining` 三种结果、版本号作废迟到结果）、
  `load_from_deck`（原版 `loadFromJson`）、`load_latest_run`、`reset`。规则判断一律调 A 节的 core 函数。
- 事件：`listen_events` 现在要 `Window`；store 没有窗口，可以在 `tasks.rs` **加**不带窗口的变体（`run_async` 同理），汇报第 8 节说明。
- **同一时刻只有一次生成或重试**（core 的 `generation_running` 也这么限制）：别的项目在生成时，第一步的「开始 AI 生成」不可用（原版 `canGenerate`
  看的是全局 `gen.running`）；store 只保存一个项目的状态，打开别的项目时，若 store 正在为另一个项目跑，就不要 `reset` 它，该项目按库里的数据显示。
- **库里停在 `running` 的旧运行**：如果 store 并没有在为这个项目跑（例如上次进程崩了），按「已中断」显示（`running = false`，阶段与看板照 `slideStatus`
  的非运行分支），允许重新生成；**不要**改库。这是对原版的有意偏离（原版会一直转圈），写进汇报第 8 节。
- `WorkflowPage` 与 `Step2` 观察 store；`ProjectModel` 里的 `generating` / `generation_failed` / `gen_*` 字段删掉，统一从 store 读，避免两份状态。
  生成或重试改了牌组后，`ProjectModel.deck` 要跟着更新（第三步用它）。
- 离开项目时生成继续；回来时看板接着动。自动跳页只在「用户正停在**这个项目**的第 2 步」时发生。

## C. 工作流（`pages/workflow/mod.rs`）的相关修正

- 侧栏第 2 步说明：出错 →「生成出错」，生成中 → `stageLabel`，否则有内容页 →「已生成 N 张」，否则「等待开始」（N 是 store 的 `slideCount`，
  即非空蓝图数）；第 2 步状态出错时为 error。第 3 步：说明「N 张幻灯片 / 生成后可编辑」，**有页在重试时不可点**。
- `canGenerate` 加上「没有页在重试」。
- 打开项目的顺序照原版 `onMounted`：先 `load_latest_run`，没有进行中的生成时 `load_from_deck(blueprints_json)`（或 `reset`）再 `load_latest_run`
  （日志恢复的失败状态要盖过「全部 done」）；进入哪一步：生成中 → 2，有蓝图 → 3，否则 → 1。
- 自动跳页照原版 `watch`：`stage == done` 且没有页在重试、没有失败页、当前在第 2 步时，800ms 后再检查一遍再跳；条件变了就取消计时（替换 `advance_task` 即可）。

## D. 第二步（`pages/workflow/step2.rs`）

把 `todo_block` 与 `TODO(parity)` 全部替换成实现，parity 表「第二步」一节全部 ✅：

1. **头部**：状态圆点（按 stage 着色）、标题与 message（见上文「偏差」）、三个统计：幻灯片（看板条目数）/ 关键事件（`important`）/ 生成记录（`input`、`prompt_io`、`slide`、`repair` 四类）。
2. **整体进度**：「整体进度」「已完成 d/t」「进行中 n」「失败 n」「已中断 n」（为 0 的不显示）、百分比、「查看详情 / 收起详情」、进度条、五个阶段
   （图标：完成打勾、进行中实心点；「进行中」徽章；`completed/total`）。展开后两段说明文字与四列表格逐字照抄。
3. **看板**：空状态文案；卡片：`#n`、标题（`page_title` → layout 标题 → 蓝图标题 → 「第 n 张」）、状态标签（A 节 `slide_status`，按 tone 着色）、
   五个阶段标签「规划 / 布局 / 内容 / 规范 / 校验」（done / active / pending 三种样式）、版式标签或「等待布局排版结果」、缩略图或占位
   （「内容生成后将在这里出现预览」/「当前还没有可渲染内容」）；生成中面板标题旁有闪烁的实时点。
   - 网格照原版 `repeat(auto-fill, minmax(260px, 1fr))`：按面板宽度算列数，卡片宽 = 列宽，缩略图按实际卡片宽度要图（不要固定 300）。
   - **只给可见的卡片要图**：用 GPUI 的 `uniform_list`（按行虚拟化）或等价做法，`set_wanted` = 可见行的卡片下标（可多预取一行）。滚出视野的图交给
     `PreviewCache` 的缓存上限淘汰；汇报里写清楚「长牌组」打开时与滚到底时各实际请求了几页。
   - 缩略图牌组 = 看板条目的蓝图（含取自日志的蓝图，空位为 `None`），下标就是 `item.index`（`CLAUDE.md`「Typst 缩略图」）。
4. **选择**：点击选中；↑ / ↓ 换页（`track_focus` + `on_key_down`，进入第 2 步时让看板拿到焦点）；选中下标越界或没有时取 `preferred_slide_index`。
   选中的卡片滚动到可见（原版没有，GPUI 下顺手做，可选）。
5. **详情**：头部「第 n 张」、标题、状态 / 版式 / 章节三个标签；大图（无蓝图时「当前页尚未生成最终内容。」）；然后依次：
   - 失败段（该页 error 或在重试）：「生成失败，可单独重试」、状态文字（排队 / 正在重新生成 / 错误信息）、「重试这一页 / 重试处理中」按钮
     （禁用条件照原版）、有蓝图时「当前保留简化内容，可继续编辑，也可重试生成。」；
   - 「规划页面」「布局排版」（含「· 已调整布局」与 reason）「内容结果」三段，各自的空提示；
   - 两个可折叠段「查看当前页调试信息」（System Prompt / User Prompt / 模型原始输出 / 解析结果，等宽字体、可选中复制、长文本可滚动）与「查看最近日志」。
   - 没有选中时：「生成开始后，选择任意一张幻灯片查看详情。」
6. **重试布局对话框**（`RetryLayoutDialog.vue`，用 GPUI Kit 的 dialog）：标题「选择重试布局」、「第 n 页将按所选布局重新生成。」、读取中 / 错误 / 下拉选择
   （label 与说明取 `editor_schema`）、「取消」「按此布局重试」；默认选中当前版式；`get_retry_layouts` 是同步的，用 `run_bg`，关掉对话框后迟到的结果作废。
7. **底栏**：见上文「偏差」；「重新生成全部」走与第一步「开始 AI 生成」同一条路（原版 `@retry="onStep1StartGenerate"`），「返回修改」切第 1 步。
8. 离开第 2 步、离开项目时 `release` 两份 `SlideImages`（已有，保持）。

## 分支与提交

从 `feat/native-step1` 新建分支 `feat/native-step2`，按顺序提交（每步提交前跑完验证，并同步更新 `docs/native/parity.md`）：

1. `feat(core): generation progress rules shared with step 2`（A 节规则、快照、`pub` 字段）
2. `chore(core): seed a failed generation run and a long deck`（A 节 seed）
3. `feat(native): app-level generation store`（B、C）
4. `feat(native): step 2 — progress, board, detail, page retry`（D）

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo build -p huanshu-native --release
```

真机验证（**全部在沙箱**，用 `tools/drive.ps1` 与 `tools/shots.ps1`；先 `seed_demo --force` 重建沙箱，记得先备份）：

- 截图与 `docs/native/reference/project-step2.png`、`step2-board.png`、`step2-detail.png` 逐张对比，列出差异。在 `tools/shots.ps1` 里加上
  「生成记录演示」项目的第 2 步（看板 + 选中失败页的详情）。
- 「生成记录演示」：看板条目数 = 内容页数（不含封面 / 目录 / 结尾）；失败页显示「生成失败」，其五个阶段标签停在失败前的阶段；
  整体进度的数字与原版对同一份日志的计算一致（用 A 节的快照证明）；↑ / ↓ 换页；展开「查看详情」「查看当前页调试信息」「查看最近日志」；
  打开「选择重试布局」→ 下拉里是可用版式、说明随选择变化 → 取消；再打开 → 「按此布局重试」：没有 LM Studio 时该页回到「生成失败」并显示错误，界面不崩。
- 底栏：出错状态下「返回修改」回到第 1 步；从首页打开一个正常演示项目，第 2 步显示「生成完成 · N 张幻灯片」与「前往编辑」。
- 生成中离开再回来：LM Studio 在跑就做一次真实生成（3–5 节的小文稿），中途回首页再进项目，确认进度接着走、完成后自动跳到第 3 步、牌组可预览；
  没在跑就写「生成流程未真机验证」，并至少验证：生成出错后离开再回来，状态仍是「生成遇到问题」。
- 「长牌组」项目：打开第 2 步、滚到底再滚回顶，记录实际请求的缩略图页数与工作集内存（量**本进程**，不是 PowerShell 的 `$PID`）。

## 最终汇报（按此格式，不要省略）

1. 四个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）
3. parity 表本轮改动的行（改动前 → 改动后，附依据）
4. A 节快照：生成脚本怎么写的、覆盖了哪些输入、Rust 与 TS 有没有不一致（有的话怎么处理的）
5. 截图对比清单与差异
6. 真机验证逐项结果（没做的写「未做」）；给用户的人工验收清单
7. 新增 / 修改的 core 函数与单测；有意修改的测试期望值及原因（没有就写「无」）
8. 与本说明或原版不一致的实现及原因；改动了框架接口的地方（`tasks.rs` 的新变体、store 放在哪）；没做完或不确定的点
