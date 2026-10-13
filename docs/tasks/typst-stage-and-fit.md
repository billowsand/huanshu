# 任务：Typst 自适应缩放提速 + 演示主画面改用 Typst 出图（第二步）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio，Tauri v2 + Vue 3 + Rust）里工作。
开始前先读 `CLAUDE.md` 的「Typst 原生 PDF 导出」整节（尤其「约定」和新加的「Typst 缩略图」小节），
再读本说明里点名的文件。上一步的说明在 `docs/tasks/typst-thumbnails.md`，可作背景参考。

## 背景

长期目标：Typst 成为唯一的幻灯片渲染器。上一步已把缩略图（Step2 生成看板、演示全览）换成 Typst 光栅化的 PNG
（`useTypstThumbnails` → `render_slide_thumbnails` → `Renderer::render_png`）。本步做两件互相独立的事：

- **A. 自适应缩放提速**：编辑器实时预览（下一步）要求单页改动后能很快出图。实测（release，17 页测试牌组，
  每次给标题追加一个字、只编这一页）：多数版式约 9ms，但 `feature_grid` p50 68ms / p95 86ms，
  `spotlight` p50 36ms。慢在 `lib/base.typ` 的 `fit-height` / `fit-factor`：二分法每轮都要把整块内容排版、
  测量一遍（`fits(1)`、`fits(min)` 加 7 轮，共最多 9 次），`center-v` 还会再测一次。
- **B. 演示主画面**改用 Typst 出图（`PresentationOverlay.vue` 的 `.pres-slide` 里那个 `SlideRenderer`）。

## 上一轮审查发现的问题（不要再犯）

1. **下标映射**：上一轮在 Step2 里把蓝图切片后又加了一次偏移，缩略图整体错了两页。凡是「子集位置 ↔ 牌组页序」
   的换算，写之前先确认每个数组的下标含义，并在汇报里写明。
2. **过期响应整批丢弃**：后一次请求只包含它那一刻变了的页，整批丢弃会让前一批的页永远拿不到图。
   现在 `useTypstThumbnails` 是**按键落图**，扩展它时保持这个机制。
3. **条件分支顺序**：占位页（`placeholder`）曾经掉进「加载中」分支出不来。每个 `v-if / v-else-if / v-else`
   都要覆盖：有图、在路上、占位页、失败。

## 范围

**要改：**

- A：`crates/slide-typst/typst/lib/base.typ`，以及调用 `fit-height` / `fit-factor` 的版式（只许改调用方式，
  不许改版式几何）；新增两个 example（见下）。
- B：`src/composables/useTypstThumbnails.ts`（扩展参数）、`src/views/PresentationOverlay.vue`（主画面）。

**不许改：**

- Step3 编辑器实时预览、Step2 详情大图（下一步再做）
- 主题 token、`themes/*.typ`、`Keynote*.vue`、`SlideRenderer.vue`
- `fit-min = 0.4` 的下限、溢出裁切与 `<slide-overflow>` 标记的语义（一页幻灯片必须恰好一个 PDF 页面）
- `CLAUDE.md` / `README` / `CHANGELOG`（文档由审阅方更新）

## A. 自适应缩放提速

### A1. 先建测量工具（先提交，拿到基线再动算法）

1. 新增 `crates/slide-typst/examples/bench_edit.rs`，用 `fixtures/all-layout-blueprints.json` 中受支持的页：
   - 冷编译整副、什么都不改再编一次、改一页标题后整副重编（各打印耗时）；
   - **逐版式**模拟输入：取该 kind 的第一页，单独成一副牌，连续 20 次给 `title` 追加一个「字」，每次编译计时，
     打印 p50 / p95；再做一组「改正文」：改该页第一个卡片/条目的正文字段（每个 kind 自己选一个确实会显示的
     正文字段，在代码里写清楚选的哪个字段）；
   - 输出一张表：`kind | title p50 | title p95 | body p50 | body p95`。
   - 必须用 `--release` 跑；参数 `--theme <id>`，默认 `light-notes`。
2. 新增 `crates/slide-typst/examples/png_diff.rs`：比较两个目录里同名 PNG，逐张输出平均绝对差（0–255）
   和差异像素占比（任一通道差 > 16 视为差异像素），最后按平均差降序列出前 5 张。只用已有的 `image` 依赖。
3. **在改算法之前**用现有代码生成基线：
   - `render_fixture -- --png` 加 `--theme`，对 `light-notes`、`light-terminal`、`dark-blue` 三个主题，
     分别渲染 `fixtures/all-layout-blueprints.json` 和 `crates/slide-typst/tests/fixtures/stress.json`，
     输出到 `target/fit-baseline/<theme>-<fixture>/`；
   - 跑一遍 `bench_edit`，把表格原样保存，汇报里要用。

### A2. 调查：为什么跨次编译的缓存帮不上忙

Typst 用 comemo 记忆化排版。给标题追加一个字，只要标题没换行、内容量档位（`slide-type-scale` 只有三档）没变，
卡片区的 `w` / `h` 和 `build` 捕获的值都应该不变，理论上能命中缓存，但 `feature_grid` 每次仍要 68ms。
用 `bench_edit` 做对照实验（例如只改一个不影响版面的字段、固定标题长度只换字），找出导致缓存失效的原因，
在汇报里写清楚**证据**（实验设计 + 数字）。找到能安全修复的就修；修不了的写明原因，不要硬改。

### A3. 改进搜索算法

1. 在 `base.typ` 新增按**数值**搜索的版本（名字自定，例如 `fit-scale(metric, limit, min: fit-min)`）：
   `metric(f)` 返回长度（或比值），可行条件是 `metric(f) <= limit`。用**带保护的插值搜索**：
   - 先测 `f = 1`，可行就直接返回 1（与现在一致）；再测 `f = min`，不可行就返回 none（与现在一致）；
   - 维持区间 `[lo 可行, hi 不可行]`，下一个试探点按 `metric` 在两端的值做线性插值求 `metric = limit` 的 f，
     并夹到区间内侧 10%–90%，保证每轮至少缩小 10%（折行会让 metric 出现台阶，纯插值可能卡住）；
   - 区间宽度 ≤ 0.005 时停止（现在 7 轮二分的精度是 0.6/128 ≈ 0.0047）；设迭代上限 10 次；返回 `lo`。
2. `fit-height` 改用它（metric = 测出的高度）。同时让 `fit-height` 的调用方不必再为垂直居中重新测量：
   新增一个把 `center-v` 和 `fit-height` 合起来的辅助函数，把最后一次可行测量的高度直接用于居中；
   把 `center-v(…, fit-height(…))` 这种写法的调用点改用它（`grep -rn "fit-height(" typst/layouts`）。
3. 直接调用 `fit-factor(f => X(f) <= L)` 的版式（issue-stack、process、section-intro 第 82 行、swot、timeline、
   overview 等）改成用新函数（metric = `X`，limit = `L`；overview 的 `(need, have)` 可用比值 `need / have`、
   limit 1）。`fits` 不是「单个数值 ≤ 上限」形式的（split-layers、section-intro 第 215 行这种多条件的）**保留
   原来的 `fit-factor`**，不要硬套。
4. 不要改 `fit-min`、不要动裁切和 `overflow-marker` 的逻辑，不要改任何版式的几何常量。
5. 遵守 `CLAUDE.md` 里 Typst 的约定，尤其：参数不要命名成 `h` / `v`；局部函数不要命名成 `center` / `left` / `top`；
   多行算式用括号包起来（以 `+` 开头的续行会被当成新语句）。

### A4. 验收标准

- `cargo test -p slide-typst` 全部通过（包括压力用例的溢出标记与 notes 一致、各主题渲染、注入测试）。
- 用改动后的代码按 A1 第 3 条再渲染一遍到 `target/fit-after/…`，`png_diff` 对比基线：
  **每张平均绝对差 < 1.0**；超过的页在汇报里逐页列出原因（例如 f 从 0.913 变成 0.911 导致某行换行）。
- `bench_edit` 目标：`feature_grid` title p50 ≤ 30ms、`spotlight` title p50 ≤ 20ms，其余版式不比基线慢 10% 以上。
  达不到就如实报告实际数字和瓶颈，不要为了数字放宽精度或改版式。

## B. 演示主画面改用 Typst 出图

### B1. 扩展 `useTypstThumbnails`

新增两个可选参数，不改变现有调用方的行为：

```ts
wanted?: () => number[]   // 只渲染这些页（传入数组的下标）；默认全部
maxCached?: number        // thumbs 里最多保留几张；超出时淘汰不在 wanted 里、最久没用到的
```

- `wanted` 只决定**发请求时 `only` 包含哪些页**，不改变发给 Rust 的牌组（仍是整副、仍按子集位置）：
  缓存键里有子集位置，如果按 wanted 裁剪牌组，翻一页就会让所有键失效。
- 淘汰时同步删掉对应位置的「已显示键」，否则被淘汰的页再回到 wanted 时不会重新请求。
- 保持按键落图、在途键去重、`enabled()` 为假时保留缓存这些现有机制。

### B2. 接入演示主画面

- 新建一个 `useTypstThumbnails` 实例（与全览那个分开）：
  - `slides: () => props.blueprints`，`projectId: () => props.projectId ?? null`；
  - `wanted`：当前页及前后各 2 页（预取，翻页时下一页已经就绪）；`maxCached: 8`；
  - `enabled`：演示中且**不在全览模式**；
  - `widthPx`：图要覆盖舞台的实际像素尺寸。页面比例固定、拉伸模式下舞台可能非等比，所以取
    `max(舞台宽, 舞台高 × 页宽 / 页高) × devicePixelRatio`；舞台尺寸用现有的 `stretchedViewport`（拉伸）
    或 `stageSize`（等比）。Rust 侧会把宽度夹到 3840。
- 显示：当前页有可用的图（不是 placeholder、未失败）时，在 `.pres-slide-wrapper` 里放
  `<img class="pres-slide-img">`，**不要放进带 `transform: scale(...)` 的 `.pres-slide` 里**（先按 1280×720 布局
  再放大，可能被按小尺寸栅格化而变糊）：直接把 img 的 CSS 宽高设成舞台尺寸，`object-fit: fill`
  （拉伸模式本来就要非等比铺满；等比模式下舞台尺寸本身就是等比的）。
- 当前页还没有图、是 placeholder 或失败时：照旧渲染原来的 `.pres-slide` + `SlideRenderer`（主画面不能出现空白）。
- `<Transition>` 的翻页动画保持不变（图和组件都放在同一个带 `:key` 的 wrapper 里）。
- 开关：`localStorage['huanshu.typstStage'] === '0'` 时主画面整体回退到 `SlideRenderer`（读写包 `try/catch`）。
- 已知且接受的差异（不用修，汇报里列出你实际观察到的即可）：封面、结尾、SWOT 的入场动画（`SlideReveal`）没有了；
  spotlight 多图由轮播变为平铺；图表没有 ECharts 动画。

## 分支与提交

从 `feat/typst-thumbnails` 新建分支 `feat/typst-stage`，按下面顺序提交（每步提交前跑完该步的验证）：

1. `perf(typst): edit-latency benchmark and PNG diff tools`（A1，提交信息里贴基线表）
2. `perf(typst): interpolation fit search`（A2 + A3，提交信息里贴前后对比表和 png_diff 摘要）
3. `feat: wanted pages and cache cap for Typst thumbnails`（B1）
4. `feat: Typst-rendered presentation stage`（B2）

## 验证

```bash
cargo fmt --all
cargo test -p slide-typst
cargo test -p auto-slidev-studio --lib thumbnails
cargo test -p auto-slidev-studio --lib pdf
cargo run --release -p slide-typst --example bench_edit
cargo run --release -p slide-typst --example png_diff -- target/fit-baseline/<dir> target/fit-after/<dir>
bun run build
```

真机验证（可选，做了就写清楚怎么做的）：`.claude/launch.json` 里有 `tauri-dev` 配置，会打开 WebView2 调试端口 9223，
可按 `CLAUDE.md`「真机验证」用 CDP 驱动。注意：在页面里替换 `__TAURI_INTERNALS__.invoke` **拦截不到**应用自己的调用，
不要用这种办法统计请求次数。`tauri dev` 是 debug 编译，Typst 很慢（20 页缩略图约 1.6 秒），不要拿它的耗时当性能结论。

## 最终汇报（按此格式，不要省略）

1. 四个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行，失败就如实贴）
3. `bench_edit` 前后对比表（全部版式，title 与 body 两组）
4. A2 的调查结论与证据
5. `png_diff` 摘要：每个主题 × 每个 fixture 的最大平均差，以及超过 1.0 的页和原因
6. 与本说明不一致的实现及原因
7. 没做完或不确定的点
8. 是否做过真机验证。**没跑就写「未做真机验证」**，不要写「已验证 UI」。
