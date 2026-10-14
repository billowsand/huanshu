# 任务：新内容排版提速 + 编辑器实时预览改用 Typst 出图（第三步）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio，Tauri v2 + Vue 3 + Rust）里工作。
开始前先读 `CLAUDE.md` 的「Typst 原生 PDF 导出」整节，尤其「Typst 缩略图」和「自适应缩放（`fit-scale`）」两个小节，
再读本说明点名的文件。前两步的说明在 `docs/tasks/typst-thumbnails.md`、`docs/tasks/typst-stage-and-fit.md`。

## 背景

长期目标：Typst 成为唯一的幻灯片渲染器。缩略图（Step2 看板、演示全览）和演示主画面已经是 Typst 光栅化的 PNG
（`useTypstThumbnails` → `render_slide_thumbnails` → `Renderer::render_png`）。还剩 Step3 编辑器实时预览和 Step2 详情大图。

编辑器的难点是**边打字边出图**。`bench_edit` 末尾的「clip branch」段（持续给 `feature_grid` 卡片正文追加文字，
内容多到需要缩小）每次编辑要 **100–300ms**：每次输入都是新文字，缓存帮不上忙，`fit-scale` 的每一次探测
都要把整块内容重新排版、测量一遍。所以本步分两部分：

- **A. 新内容排版提速**：减少 `fit-scale` 的探测次数。
- **B. 编辑器实时预览**（以及 Step2 详情大图）改用 Typst 出图，并在打字时保持流畅。

## 前两轮审查发现的问题（不要再犯）

1. **没有独立基线就宣称「完全一致」**：上一轮报告「117/117 张逐字节相同」，审查方用旧代码重新渲染后发现
   7–8 页平均差到 5.5。基线必须在改代码**之前**用旧代码生成；性能对比也要用旧代码编出来的程序（见 A1）。
2. **基准测试不代表真实输入**：「替换一个等长字符」不改版面，会命中缓存，测不出打字的真实耗时。
   本机负载波动很大（同一版本单次能从 98ms 漂到 342ms），**单次结果不能下结论**，必须交替多轮、取中位数。
3. **代码与自己的说明矛盾**：上一轮提交信息写「主画面不能空白」，代码却放了透明占位。注释和提交信息写的
   每一条行为，都要和代码一致。
4. **混用下标空间**：`thumbs` / `lastUsed` 以数组下标为键，`shownKeys` / `pendingKeys` 以子集位置为键。
   新增的状态写清楚用哪种键，换算时用 `latestIndices`。
5. 更早的教训仍然有效：条件分支要覆盖「有图 / 在路上 / 占位页 / 失败」四种情况；响应按键落图，不整批丢弃。

## 范围

**要改：** `crates/slide-typst/typst/lib/base.typ`（`fit-scale`）、`crates/slide-typst/src/lib.rs`（溢出信息）、
`src-tauri/src/commands/thumbnails.rs`、`crates/slide-typst/examples/bench_edit.rs`（加参数）、
`src/composables/useTypstThumbnails.ts`、`src/views/steps/Step3Editor.vue`、`src/views/steps/Step2Generate.vue`
（详情大图）、必要时 `src/views/WorkflowView.vue`（样式、传参）。

**不许改：** 任何版式的几何常量与 `fit-min`、裁切与 `<slide-overflow>` 语义、主题、`Keynote*.vue`、`SlideRenderer.vue`、
`CLAUDE.md` / `README` / `CHANGELOG`（文档由审阅方更新）。

## A. 新内容排版提速

### A1. 先留住旧版本（改任何代码之前做）

1. `cargo build --release -p slide-typst --example bench_edit --example render_fixture`，把
   `target/release/examples/bench_edit.exe` 和 `render_fixture.exe` 复制成 `target/bench-old/bench_edit.exe`、
   `target/bench-old/render_fixture.exe`。模板是 `include_str!` 编进程序的，复制出来的就是旧版本。
2. 用旧的 `render_fixture.exe` 生成像素基线：`light-notes`、`dark-blue` 两个主题 ×
   `fixtures/all-layout-blueprints.json`、`crates/slide-typst/tests/fixtures/stress.json`，输出到 `target/fit3-baseline/<theme>-<fixture>/`。
3. 给 `bench_edit` 加参数 `--only clip`（只跑「clip branch」段）和 `--rounds <n>`（该段重复 n 遍，打印每个规模的中位数
   和全部规模的总中位数）。这个改动在 A1 里先做、先提交，然后**重新**按第 1 步复制一次旧版本
   （这样新旧两个程序有同样的参数）。

### A2. 减少探测次数

现在的 `fit-scale` 先测 `f = max`、再测 `f = min`，然后在 [min, max] 上做线性插值。问题：

- 文字在固定宽度里重排时，高度大致和 `f²` 成正比（行高和每行字数都随 `f` 变），线性插值的第一次猜测偏差很大；
- `f = min = 0.4` 那一次探测几乎总是离答案很远，却要完整排版一遍。

改成：

1. 先测 `f = max`；可行就返回（与现在一致）。
2. 用幂律模型猜第一个点：`f0 = max × (limit / metric(max))^(1/2)`，夹到 `[min, max]`。测 `f0`。
3. 之后维持 [可行 lo, 不可行 hi]，下一个点在**对数坐标**上插值（`ln metric` 对 `ln f` 线性，幂律在这里是直线）；
   只知道一端时，用已测的那一端和指数 2 外推。每一步仍夹到区间内侧 10%–90%，保证收敛。
4. **只有**在搜索确实需要往下探到 `min` 时才测 `min`；`metric(min) > limit` 时返回 none（与现在一致）。
5. 停止条件不变（区间 ≤ 0.005 或 10 轮），返回 `lo`。`metric` 可能返回长度或比值（见 `fit-metric-value`），保持兼容。
6. 不要改 `fit-factor`（二分，多条件 `fits` 的调用点还在用）。

### A3. 验收

- `cargo test -p slide-typst` 全部通过。
- 用新的 `render_fixture` 渲染到 `target/fit3-after/…`，`png_diff` 对比 A1 的基线：逐张列出平均差；平均差 > 1.0 的页
  要打开前后两张图对比，写明**有没有折行变化**（只允许子像素位移和极小的字号差）。
- 性能：交替运行 `target/bench-old/bench_edit.exe --only clip --rounds 5` 与新的 `bench_edit --only clip --rounds 5`，
  **交替 3 轮**（旧、新、旧、新、旧、新），报告每轮两者的总中位数。目标：新版本中位数比旧版本低 40% 以上。
  达不到就如实报告数字，不要改精度或版式去凑。

## B. 编辑器实时预览

### B1. 溢出信息带回前端

`Thumbnail` 增加 `overflow: bool`：该页内容即使缩到 `fit-min` 也放不下、被裁切了（来源：`render_png` 返回的
`overflows` 里 `slide` 等于这一页的条目）。`thumbnails.rs` 的单测补一个溢出页的断言（`stress.json` 里有现成的溢出页，
或者构造一页超长内容）。

### B2. 扩展 `useTypstThumbnails`（不改变现有调用方的行为）

新增可选参数：

```ts
debounceMs?: number          // 默认 200（现有行为）
serial?: boolean             // true：同一时刻最多一个请求在途；在途期间的变化合并，响应回来后立刻用最新状态再请求一次
progressive?: boolean        // true：响应即使已过期，只要比当前显示的图新，也先落下来；对应页的已显示键记成它自己的键，
                             //       这样最新内容仍会被再次请求
resetKey?: () => unknown     // 值变化时清空全部图和键（调用方立即回退到幻灯片组件），用于「换了一页」
```

并在返回值里加：

```ts
pending: Ref<boolean>        // 有请求在途或在防抖中
overflow: Ref<Record<number, boolean>>   // 以数组下标为键
```

- `progressive` 判断「比当前显示的新」要用**请求序号**，按页记录已显示图的序号（键为数组下标还是子集位置，写清楚）。
- `serial` 与 `progressive` 一起用时：打字过程中预览每隔一次渲染时间更新一次，停手后一定落在最新内容上。
  在代码注释里说明这一保证是怎么成立的。
- `resetKey` 变化时，在途响应全部作废（它们属于上一页）。

### B3. Step3 编辑器预览

- 新建一个实例：`slides: () => editorPreviewSlide ? [editorPreviewSlide] : []`（**只发这一页**：版式只依赖本页数据，
  单页编译最快；`chart:<index>` 和溢出标记的下标都是 0，换算回 `activeSlide`）、
  `resetKey: () => activeSlide`、`serial: true`、`progressive: true`、`debounceMs: 120`、
  `widthPx: editorStageW × devicePixelRatio`。
- 显示：有图时在 `.edit-preview-stage` 里放 `<img>`（CSS 尺寸 = `editorStageW × editorStageH`），**不要**放进带
  `transform: scale()` 的 `.edit-preview-canvas`；还没有图、是占位页或失败时，照旧渲染原来的 `SlideRenderer`。
- `pending` 为真时在预览栏标题旁显示一个不抢眼的「渲染中」小标记（不要遮挡画面、不要闪烁：pending 持续 < 150ms 时不显示）。
- `overflow` 为真时显示一条提示：「内容过多，已缩到最小字号仍放不下，超出部分被裁切」。
- 开关：`localStorage['huanshu.typstEditorPreview'] === '0'` 时整体回退到 `SlideRenderer`。
- 与屏幕版的已知差异（入场动画、spotlight 轮播、图表动画）照旧接受。

### B4. Step2 详情大图

`Step2Generate.vue` 右侧 `.sdp-preview-canvas` 里的 `SlideRenderer` 同样换成 Typst 图：新建一个实例，
`slides` 与看板相同（`slideItems` 的蓝图，含空位），`wanted: () => 选中页 ? [选中页] : []`、`maxCached: 4`、
宽度 = 详情舞台宽 × dpr；没有图时回退 `SlideRenderer`。

## 分支与提交

从 `feat/typst-stage` 新建分支 `feat/typst-editor`，按顺序提交（每步提交前跑完该步的验证）：

1. `perf(typst): clip-only rounds in bench_edit`（A1 第 3 条）
2. `perf(typst): power-law fit search`（A2，提交信息里贴 A3 的对比数字与 png_diff 摘要）
3. `feat: report clipped thumbnails`（B1）
4. `feat: serial, progressive and resettable Typst thumbnails`（B2）
5. `feat: Typst-rendered editor preview and generation detail`（B3 + B4）

## 验证

```bash
cargo fmt --all
cargo test -p slide-typst
cargo test -p auto-slidev-studio --lib thumbnails
cargo test -p auto-slidev-studio --lib pdf
bun run build
```

真机验证（强烈建议）：`.claude/launch.json` 的 `tauri-dev` 配置会打开 WebView2 调试端口 9223，可按 `CLAUDE.md`「真机验证」用 CDP 驱动。
注意：
- 首次启动时 Vite 预构建依赖，页面可能卡在空白，`location.reload()` 一次即可；
- 替换 `__TAURI_INTERNALS__.invoke` 拦截不到应用自己的调用；
- 演示的翻页键监听在 `window` 上，页码条不是 `<button>`；
- `tauri dev` 是 debug 编译，Typst 很慢，不要拿它的耗时当性能结论，只用它验证行为
  （例如：连续改标题时预览会陆续更新、停手后落在最新内容、切换页面时不会闪出上一页的图）。

## 最终汇报（按此格式，不要省略）

1. 五个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行，失败就如实贴）
3. A3 的性能对比：3 轮 × 新旧两个程序的总中位数，以及结论
4. `png_diff` 摘要：每个主题 × fixture 的最大平均差；平均差 > 1.0 的页逐页说明有没有折行变化
5. B2 中 `serial` + `progressive` 的保证是怎么成立的（一段话）
6. 与本说明不一致的实现及原因
7. 没做完或不确定的点
8. 是否做过真机验证、验证了哪些行为。**没跑就写「未做真机验证」**，不要写「已验证 UI」。
