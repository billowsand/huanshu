# 任务：原生界面第六轮——演示收尾、剩余项与逐页审计（第十四步）

你在仓库 `E:\huanshu`（幻述）里工作。原生界面 `apps/huanshu-native` 的各页都已做完并审过（最新分支 `feat/native-step3b`）。
`docs/native/parity.md` 只剩几行没做完。本轮的目标是**为原生版完整替代原版做准备**：

- **A. 演示收尾**：全屏时幻灯片铺满屏幕、控制条浮在上面悬停才出现；非全屏的留白照原版；触摸滑动翻页；
- **B. 剩下的 parity 行**：核实并结清；
- **C. 逐页审计**：拿原版每个界面文件逐段对照原生版，写出审计表 `docs/native/audit.md`，小差距当场修，大差距列出来；
- **D. 需要 LM Studio 的验收**：LM Studio 在跑就在沙箱里把生成相关的流程真机走一遍，不在跑就跳过并如实写明。

开始前**必须**先读完：

1. `apps/huanshu-native/README.md`——框架与「写页面的规矩」（逐条遵守）；
2. `docs/native/parity.md`（全部）；
3. `CLAUDE.md` 开头三节（代码结构、隔离数据目录、原生界面）；
4. 原生：`present.rs`、`shell.rs`（演示覆盖层怎么盖在页面上、标题栏）、`pages/` 下各页；
5. 原版：`src/views/PresentationOverlay.vue`（全部）与 `src/views/WorkflowView.vue` 里 `.pres-*` 的样式（约 2900–3100 行）、
   `src/views/` 与 `src/components/` 下全部 `.vue`（C 节逐个对照）；
6. 原版截图：`docs/native/reference/present.png`、`present-overview.png`，以及其余全部（C 节用）。

## 原则（同前几轮）

- **复刻**：布局、文案逐字、交互、规则以原版为准；可按 GPUI 特点调整控件形态，但**不能少功能**。拿不准时读原版代码。
- **不重写框架**：缺部件往 `widgets.rs` **加**；改框架接口要在汇报第 8 节说明理由。
- **数据只经 `huanshu_core::services`**；新规则放 core（带单测），界面里不写规则。

## 前几轮审查发现的问题（不要再犯）

1. **真机验证碰了真实数据**：只用 `tools/drive.ps1` / `tools/shots.ps1`。动手前备份沙箱（→ `target/sandbox-home.bak9`），然后 `seed_demo --force`。
   驱动脚本超时退出时窗口可能还开着，下一次 `cargo build` 会因 exe 被占用失败：先关掉残留的 `huanshu-native` 进程。
2. **UI 线程里的延时用了 tokio**：界面里的延时一律 GPUI 的 `background_executor().timer(..)`。
3. **「进行中」标志没复位导致永久卡住**：每个标志的每条出口都要复位，汇报里逐条列出。
4. **改 `.vue` / `.ts` 不跑类型检查**：本轮原则上不改原版前端；真要改，改完跑 `bunx vue-tsc --noEmit`。
5. **汇报要诚实**：没真机验证的写「未验证」；「原版没有这个功能」要贴出原版代码依据。

## A. 演示（`present.rs`）

原版 `PresentationOverlay.vue` 的几何与控制条，逐条照做：

- **舞台留白**：非全屏时舞台可用区 = 视口宽 − 96、视口高 − 88（原版 `stretchedViewport` 的 `padH` / `padV`）；**全屏时都是 0**——幻灯片铺满整个屏幕
  （拉伸模式铺满、等比模式按比例取最大）。现在原生版不管全屏与否都减掉控制条高度与标题栏高度，全屏时幻灯片铺不满。
- **浮动控制条**：全屏时控制条（胶片条、页码、进度、按钮）不再占位，而是叠在舞台底部（原版 `.pres-controls--floating`：半透明底色
  `bg` 72%、上边框 `border` 45%、默认透明且下移 8px、**悬停时**淡入上移，0.3s）。GPUI 没有 `backdrop-filter`，模糊省略；淡入用 `with_animation`
  或直接切换（写进第 8 节）。悬停判断用控制条自己的 hover（不是整个舞台）。非全屏时控制条照旧占位在底部。
- **标题栏**：对照 `present.png` 与原版代码确认演示时标题栏是否被盖住；原生按原版做（演示是盖在当前页上的覆盖层，见 `CLAUDE.md`）。
- **触摸滑动**：原版 `handleTouchStart` / `handleTouchEnd`——水平位移大于垂直位移且超过 50px 时翻页（左滑下一页、右滑上一页），这次触摸之后的那次
  「点击舞台下一页」不生效（`didSwipe`）。GPUI 把触摸平移转成带 `touch_phase` 的 `ScrollWheelEvent`（见 `gpui` 的 `gestures.rs`），在舞台上
  `on_scroll_wheel` 里按 `TouchPhase` 累计一次手势的位移、结束时判定；鼠标滚轮（没有触摸阶段）不要触发翻页。本机没有触摸屏的话写进人工验收清单。
- 改完后 parity「演示」一节的两行 ⬜ 结清，`tools/shots.ps1` 加一张全屏截图（悬停与不悬停各一张，用 drive 脚本移动鼠标）。

## B. 剩下的 parity 行

- 第一步「图表围栏的校验提示」（⬜）：先查原版第一步到底有没有单独的提示（`Step1Prepare.vue`、`MarkdownWorkbench.vue`、`WorkflowView.vue`）。
  据审查者粗查，原版的围栏错误只在生成时由 core 报出（`input.rs` → 生成失败）。若确实如此，把这一行改成「原版无单独提示；错误随生成失败显示」并
  标 ✅（附依据）。真机验证要 LM Studio：流水线先「验证大模型连接」再解析文稿（`run_pipeline`），没有 LM Studio 时看到的是连接错误，
  所以这条放进 D 节（写一个坏的 ```` ```chart ```` 围栏开始生成，第二步显示该围栏错误）。
- 第一步「编辑区 ↔ 预览区滚动同步」（🟡）：查 GPUI Kit 0.7.3 的编辑器有没有能拿到**真实**最大滚动量的接口、预览侧滚动条拖拽能不能挂上同步；
  有就补上改 ✅，没有就保持 🟡 并在说明里写清查过哪些接口。
- 工作流「库里停在 running 的旧运行」（🟡，有意偏离）不动。

## C. 逐页审计（`docs/native/audit.md`）

parity 表是按功能块记的，粒度粗，可能漏掉小东西（一个提示文案、一个禁用条件、一个快捷键、一个悬停效果）。本节逐个文件过一遍：

- 范围：`src/views/` 下 `HomeView.vue`、`SettingsView.vue`、`FirstRunWizard.vue`、`WorkflowView.vue`、`PresentationOverlay.vue`、
  `steps/Step1Prepare.vue`、`Step2Generate.vue`、`Step3Editor.vue`，`src/components/` 下全部 `.vue`，以及 `App.vue`；
- 做法：每个文件**按模板从上到下**列出每一个用户能看到或操作的东西（文字、按钮、输入、提示、禁用 / 显示条件、快捷键、悬停 / 拖放、确认对话框、
  轻提示），再在 `<script>` 里找出每一个副作用（调用了哪个命令、写了什么存储、何时触发）。每一项一行：原版位置（文件:行）→ 原生位置（文件:函数）→
  状态（✅ 一致 / 🔧 本轮已修 / ⚠️ 有差距未修 / ➖ 有意不做，附原因）。表按文件分节；
- **小差距当场修**（文案、禁用条件、缺一个提示、缺一个快捷键这类，一处不超过约 50 行）；大差距只记 ⚠️ 并在汇报第 8 节列出，不要在本轮硬做；
- `parity.md` 与 `audit.md` 不矛盾：发现 parity 某行其实没做完，改回 🟡 并说明；
- 主题相关的纯视觉细节（阴影、渐变、毛玻璃、动画）不逐条列，归成每个文件一行「视觉细节」，说明哪些做了、哪些按 GPUI 省略。

## D. 需要 LM Studio 的验收（有条件）

先检查 LM Studio 是否在跑：`curl -s http://127.0.0.1:1234/v1/models`。**不要**自己启动或改动 LM Studio，**不要**读写真实数据目录的设置。

- 不在跑：本节整体跳过，汇报里写「LM Studio 未运行，D 节未做」；
- 在跑：在**沙箱**里打开设置页，填上 `/v1/models` 返回的一个模型（只改沙箱的设置），然后用 3–5 节的小文稿逐项走：
  1. 生成全程：第二步看板逐页状态、实时点、整体进度；完成后 800ms 自动跳第三步，牌组可预览；
  2. 生成中回首页再进项目：进度接着走；
  3. **跨项目**：生成项目 A 途中打开项目 B，在 B 的第三步改一页并应用；A 完成后 A 的牌组正确、B 的修改还在、B 没被 A 覆盖（sqlite 只读核对两个项目的
     `blueprints`）；生成中 A 的第三步只读、B 的第三步可编辑；
  4. 单页重试成功路径（有失败页时）：重试后该页「已完成」、牌组全部通过时底栏恢复「前往编辑」；重试途中离开项目，结果仍写回原项目；
  5. 第三步「AI 推荐图标」走语义检索（向量库可用时），出现「推荐中…」，候选可点换；
  6. 坏的 ```` ```chart ```` 围栏（B 节）：开始生成后第二步显示该围栏的错误。
  每一项附截图或 sqlite 输出；做不到的写原因。

## 分支与提交

从 `feat/native-step3b` 开 `feat/native-audit`，提交按内容分：

1. `feat(native): fullscreen presentation fills the screen; floating controls; touch swipe`（A）
2. `fix(native): …`（B 与 C 里当场修的小差距，可以按页面拆成几个提交，每个提交说清修了什么）
3. `docs(native): page-by-page audit against the original`（`audit.md`、parity 更新、截图）

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo build -p huanshu-native --release
```

真机验证全部在沙箱：A 节全屏 / 非全屏 / 悬停截图与原版 `present.png` 对比；B 节坏围栏报错截图；C 节每个当场修的差距附修前修后的截图或说明；D 节见上。

## 最终汇报（按此格式，不要省略）

1. 提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）
3. parity 表本轮改动的行（改动前 → 改动后，附依据）
4. 审计结果概览：每个文件的条目数与 ✅ / 🔧 / ⚠️ / ➖ 各多少；全部 🔧 的清单
5. 截图对比清单与差异
6. 真机验证逐项结果（没做的写「未做」）；D 节结果或未做的原因；给用户的人工验收清单
7. 新增 / 修改的 core 函数与单测；有意修改的测试期望值及原因（没有就写「无」）
8. 全部 ⚠️（有差距未修）的清单，每条估计工作量；与本说明或原版不一致的实现及原因；改动了框架接口的地方；「进行中」标志的复位出口
