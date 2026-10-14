# 任务：图表改由 Typst 原生绘制，去掉 WebView 的 ECharts SVG 管道（第四步）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio，Tauri v2 + Vue 3 + Rust）里工作。
开始前先读 `CLAUDE.md` 的「图表页（`chart_focus`）」小节和「Typst 原生 PDF 导出」整节，再读本说明点名的文件。
前三步的说明在 `docs/tasks/typst-thumbnails.md`、`docs/tasks/typst-stage-and-fit.md`、`docs/tasks/typst-editor-preview.md`。

## 背景

长期目标：Typst 成为唯一的幻灯片渲染器，最终把界面换成原生 Rust 框架、不再依赖 WebView。
现在所有幻灯片画面（缩略图、演示、编辑器预览、PDF）都由 Typst 出图，**只有图表除外**：

1. 前端调用 `pdf_chart_boxes`，让 Typst 先排一遍版，报告每张图的绘图框尺寸和本页 `--type-scale`；
2. WebView 用 ECharts 按这个尺寸 SSR 出 SVG（`src/components/chartOptions.ts` 的 `buildChartOption` + `renderChartSvg`，
   入口 `src/composables/chartSvgs.ts` 的 `buildChartSvgs`）；
3. SVG 以 `chart:<页序>` 经素材管道交给 Typst，`chart-focus.typ` 用 `image()` 嵌入。

这是幻灯片渲染里**唯一必须有 JS 运行时**的地方，也让每次缩略图、导出都多一次往返。本步把五种图表直接用 Typst 画出来，
删掉这条管道。图表一共五种（`line` / `bar` / `donut` / `grouped_bar` / `stacked_area`），数据结构见
`src-tauri/src/types.rs` 的 `ChartSpec`（JSON 里类型字段叫 `type`）。

屏幕上 `KeynoteChartSlide.vue` 仍用 ECharts（它是 Typst 出图前的回退），**本步不动它**，下一步删除全部 Vue 版式时一起去掉。

## 前几轮审查发现的问题（不要再犯）

1. **没有独立基线就宣称「一致」**：基线必须在改代码**之前**用旧代码生成（见 A）。
2. **注释、提交信息与代码不一致**：写下的每一条行为都要和代码相符。
3. **Typst 陷阱**（都在 `CLAUDE.md` 里）：函数体里 `return` 会丢掉它之前拼接的内容；参数不要叫 `h` / `v`，
   局部函数不要叫 `center` / `left` / `top`；不要用 `align(horizon)`，垂直居中用 `center-v`；以 `+` 开头的续行
   会被当成新语句，多行算式加括号。
4. **模型/用户提供的字符串绝不拼成 Typst 标记**：图表标题、类目名、系列名、数据值一律作为纯文本（`str(...)` 后放进 `text`），
   不要用 `eval`，也不要经过 `rich()`（图表文字不支持富文本）。

## 范围

**要改 / 新建：**
- `crates/slide-typst/typst/lib/chart.typ`（新建：五种图表的绘制）、`crates/slide-typst/typst/layouts/chart-focus.typ`
- `crates/slide-typst/typst/themes/*.typ`（加图表 token）与 `themes/_check.typ`
- `crates/slide-typst/src/lib.rs`（`TEMPLATES` 登记新文件；删除 `ChartBox`、`chart_boxes`）、`crates/slide-typst/src/assets.rs`（删除 `chart:` 引用）
- `crates/slide-typst/tests/render.rs`、`crates/slide-typst/tests/fixtures/`、`crates/slide-typst/examples/render_fixture.rs`
- `src-tauri/src/commands/pdf.rs`、`src-tauri/src/commands/thumbnails.rs`、`src-tauri/src/lib.rs`（命令注册）
- `src/composables/usePdfExport.ts`、`src/composables/useTypstThumbnails.ts`、`src/components/chartOptions.ts`（只删 SSR 相关导出）

**删除：** `src/composables/chartSvgs.ts`、`scripts/chart-fixtures.ts`、`crates/slide-typst/tests/fixtures/charts/svg/`、
`crates/slide-typst/tests/fixtures/charts/themes/`（token 抄进 Typst 主题之后）。

**不许改：** `chart-focus.typ` 里卡片外框、左栏要点、标题、来源角标的几何与字号（只换绘图区里的内容）；其他版式；
`fit-scale` / `fit-height`；`KeynoteChartSlide.vue`、`SlideRenderer.vue`、`buildChartOption` 本身；`ChartSpec` 的结构和校验
（Rust `ChartSpec::problems()` 与 TS `chartProblems()`）；`CLAUDE.md` / `README` / `CHANGELOG`（文档由审阅方更新）。

## A. 先留基线（改任何代码之前做）

1. `cargo build --release -p slide-typst --example render_fixture`，把 `target/release/examples/render_fixture.exe`
   复制成 `target/bench-old/render_fixture.exe`。
2. 用旧程序按旧管道渲染参考图：

   ```bash
   target/bench-old/render_fixture.exe crates/slide-typst/tests/fixtures/charts.json target/charts-before --assets crates/slide-typst/tests/fixtures/charts/svg/assets.json --png
   ```

   这组 PNG 就是视觉基准（`charts/svg/` 里的 SVG 是按 light-notes 主题生成的，所以基准只有这一个主题）。

## B. 主题 token

每个 `themes/<id>.typ` 的 `theme` 字典加一个子字典 `chart`：

```typst
chart: (
  palette: (rgb("#746254"), …),   // 6 色，多系列与环形图扇区按序取色
  grid: rgb("#e8e0cf"),           // 数值轴分隔线
  axis: rgb("#c8bbaa"),           // 类目轴轴线、环形图引导线
  success: rgb("#54633d"),        // 折线图高亮气泡
),
```

- 值的来源：五个主题直接抄 `tests/fixtures/charts/themes/<id>.json`（`chart_palette` / `chart_grid` / `chart_axis` / `color_success`，
  这是在 Vue 对照页上实测读出来的）；`light-editorial` 没有 JSON，按 `src/themes/editorial.css` 的 `--chart-*` 与 `--success` 抄。
- 其余颜色用现有 token：主色 `primary`，主要文字 `ink`，次要文字 `muted`，扇区之间的描边用卡片底色 `paper`。
  ECharts 的半透明色（`hexA`）用 Typst 的 `color.transparentize`。
- `_check.typ` 的子字典循环里加上 `"chart"`，并断言每个主题的 `palette` 恰好 6 色。

## C. 绘制（`lib/chart.typ`）

导出一个函数 `chart-plot(ctx, chart, width, height)`，在给定尺寸内画完整图表（坐标轴、系列、数据标签、图例），
**结果的尺寸必须恰好是 `width × height`**，不能溢出、不能撑大。`chart-focus.typ` 里原来放 `image()` 的位置改成调用它；
`chart` 缺失、类型未知、数据画不出来（见 C6）时仍显示现有的 `placeholder`。

用 `place` + 绝对坐标作画（`rect`、`line`、`curve`、`circle`），所有尺寸写成「CSS px × `px`」，与其他版式一致。
文字一律 `txt(ctx, "body", …)`（图表文字属于「正文」字体角色，与屏幕上 `--font-body` 一致）。

### C1. 字号与几何：照搬 `buildChartOption`

`buildChartOption` 里的每个数值都是「scale = 1 时的 px」，屏幕上乘以本页 `--type-scale`。Typst 里同样乘 `ctx.scale`
（**不乘** `fit-height` 的 `f`：旧管道的 SVG 也只按 `ctx.scale` 渲染）。例如轴标签 `12 * ctx.scale * px`。
颜色、线宽、圆角、柱宽比例、平滑、环的内外半径（48% / 66%）、圆心（50% / 46%）、图例形状（12×8 圆角矩形、间距 18）、
markPoint 气泡尺寸（46）等都按 `buildChartOption` 的参数移植。坐标区的边距（`grid`）是 ECharts 的起点值：
**纵轴标签比留出的边距宽时，要量出标签宽度、把坐标区往里收**，绝不能让标签被裁掉或出界。

### C2. 数值轴

- 范围：包含 0（与 ECharts 默认一致）；数据全为非负时从 0 开始，有负数时下界也取整。全是 0 时用 [0, 1]。
- 刻度：目标约 5 个间隔，间隔取 `{1, 2, 3, 5} × 10^k` 里最接近且能覆盖数据的值，上界取整到间隔的倍数。
  对照参考图：最大值 41 → 0–50 步长 10；530 → 0–600 步长 100；170 → 0–180 步长 30；堆叠合计 119 → 0–120 步长 20。
- 数值格式（轴标签、数据标签、环形图中心合计都一样）：≥ 1000 加千分位逗号；最多 2 位小数并去掉末尾的 0；
  绝不能出现 `0.30000000000000004` 这类浮点尾巴。在 `chart.typ` 里写一个 `format-number` 统一处理。
- 数据值可能是数字，也可能是数字字符串（`"12.5"`），用 `float(...)` 统一转换。

### C3. 类目轴

- 类目名用 `str(...)` 转成文字；高亮比较用 `str(x) == h`（与 `buildChartOption` 一致）。
- 类目太多、标签放不下时，像 ECharts 一样**隔几个显示一个**（保留第一个，步长 k 取能放下的最小值）；不要旋转标签，不要缩小字号。
- 折线图 `boundaryGap: false`（点落在刻度上）；柱状图类目居中。

### C4. 五种图

- **line**：平滑曲线（cubic Bézier，Catmull-Rom 式切线、张力 0.5；控制点的 y 夹在相邻两点之间，防止曲线冲出数据范围）、
  圆点、主色 3px 线、由主色 20% 渐变到 2% 的面积（`gradient.linear`）、点上方数据标签。`emphasis.highlight` 命中的点：
  不画数据标签，改画 success 色的 pin 气泡（圆 + 下尖），气泡里白字写该点的值；找不到的类目跳过。
  超过 12 个点时不画普通数据标签（高亮气泡照画），否则文字会挤成一片（这是有意偏离屏幕版，写进汇报第 6 条）。
- **bar**：柱宽 46%、顶部圆角 6、柱顶数据标签。有 `highlight` 时只突出命中的类目，其余用主色 35% 透明度；没有 `highlight` 时全部实色。
  负值柱从 0 向下画，标签放在柱底下方。
- **grouped_bar**：按 `series` 顺序取 `palette` 颜色，组内间距 12%、类目间距 32%（照 `barGap` / `barCategoryGap` 的语义），顶部圆角 4，
  不画数据标签；底部图例。
- **stacked_area**：按 `series` 顺序自下而上堆叠，平滑曲线、2px 线、填充为该色 45% 透明度，不画数据标签；底部图例。
  负值按 0 处理（堆叠负数在幻灯片上没有可读的画法；写进汇报第 6 条）。
- **donut**：扇区按数据顺序从 12 点钟方向顺时针排列，`highlight[0]` 命中的扇区用主色、其余按序取 `palette`；扇区之间用 `paper`
  色 2px 描边分开。弧线用 `curve` 拼接，每段不超过 90°（三次 Bézier 近似圆弧）。外侧标签「名称换行百分比」，带两段引导线
  （14 + 10 px），标签宽度上限 130px、超出换行；**左右两侧各自按纵坐标排开，避免标签互相重叠**。百分比用最大余数法分配到
  两位小数、去掉末尾的 0，保证合计 100%。圆心写合计（26px、`ink`）和「合计」两字（12px、`muted`），间距 6px。
  0 和负值的扇区不画（也不出标签和图例项）；合计 ≤ 0 时整张图走 `placeholder`。底部图例。

### C5. 图例

底部居中，一行放不下就换行；色块 12×8、圆角 2，色块与文字间距 5px，项间距 18px，文字 12px `muted`。
图例占用的高度要从坐标区里扣掉（`grid.bottom` 写的 `fs(40)` 就是为它留的）。

### C6. 画不出来的情况

`ChartSpec::problems()` 不通过的数据进不了牌组，但旧数据或手改 JSON 仍可能出现：类型未知、`data` 为空、
所需列缺失或不是数字。这些情况显示 `placeholder(ctx, w, h, "图表待补充", detail: <原因>)`，**不能让编译失败**：
一页图表出错会让整份 PDF 导出和整副缩略图一起失败。

## D. 删除旧管道

- Typst：`chart-focus.typ` 不再读 `ctx.assets.at("chart:…")`，不再发出 `<chart-box>` 标记，并改写文件头注释。
- Rust：删除 `slide_typst::ChartBox`、`Rendered.chart_boxes`、`Renderer::chart_boxes`、`chart_asset_ref` 与
  `assets.rs` 里为图表页追加 `chart:<index>` 引用的逻辑（图表页从此不会出现在 `missing_assets` 里）；`render_fixture` 的
  `--chart-boxes` 参数。`src-tauri` 删除 `pdf_chart_boxes` 命令及其注册、`measure_chart_boxes`、`ChartSvgInput`，
  `export_pdf`、`render_slide_thumbnails`、`prepare_export`、`export_assets` 去掉 `chart_svgs` 参数。
  `assets.rs` 里通用的 SVG 清洗和它的测试保留（素材库图片同样会是 SVG）。
- 前端：删除 `chartSvgs.ts`；`usePdfExport` 与 `useTypstThumbnails` 不再构建、传递 `chartSvgs`。`usePdfExport` 里「首页导出时
  先 `get_project` 读出牌组」那一步只是为了 SSR，删掉后不传 `slides`、由 `export_pdf` 读数据库即可；改之前确认没有别的原因。
  `chartOptions.ts` 删掉 `renderChartSvg` 和 `ChartBox`，保留 `buildChartOption` / `themeFromCssVars` / `chartSourceLabel` /
  `chartProblems`（屏幕回退和编辑器还在用）。
- 删除后用 `grep -rn "chart_boxes\|ChartBox\|chart-box\|chartSvgs\|chart_svgs\|renderChartSvg\|chart_asset_ref\|pdf_chart_boxes" src src-tauri/src crates scripts`
  确认 0 命中。

## E. 测试

替换 `tests/render.rs` 里依赖 SVG 素材的三个测试（`chart_focus_renders_sanitized_ssr_svg_and_reports_missing`、
`every_chart_type_renders_in_every_theme_and_aspect` 及用到 `chart_boxes` 的那个），新的测试至少覆盖：

1. 五种图 × 6 个主题 × 3 种比例都能编译，每页恰好一个 PDF 页面，没有 `overflows`，`missing_assets` 为空。
2. 新建 `tests/fixtures/charts-edge.json`，覆盖：负值柱状图、负值折线图、千分位大数（如 1234567）、小数（0.1 + 0.2 这类）、
   30 个类目的柱状图、1 行数据的折线图、全 0 数据、含 0 和负值扇区的环形图、合计为 0 的环形图、12 个扇区的环形图、
   6 个系列的堆叠面积图、类型未知、缺列、数值是非数字字符串。全部能编译且不溢出；后四种显示占位（可以用 `placeholder`
   发出的某个标记或文字来断言）。
3. 注入：类目名、系列名、标题写成 `#panic("x")`、`]`、`$`、`<label>` 这类字符串时编译成功，并且原样出现在 PDF 文字里。
4. `format-number` 的格式规则：如果在 Typst 里不方便单测，就在 fixture 里放已知数值，用 PDF 文字断言（`0.3`、`1,234,567`、`-25`）。

## F. 视觉验收

渲染改后的图供审阅方目检，固定输出到这些目录：

```bash
cargo run --release -p slide-typst --example render_fixture -- crates/slide-typst/tests/fixtures/charts.json target/charts-after/light-notes --png --theme light-notes
```

主题循环 6 个（`light-notes light-terminal light-editorial dark-terminal dark-yellow dark-blue`），另外：
`light-notes` 加 `--aspect ratio_32x9` 和 `ratio_48x9` 各一份；`charts-edge.json` 用 `light-notes` 与 `dark-blue` 各一份。
你自己对照 `target/charts-before/` 逐张检查，至少确认：刻度值与参考图相同；没有文字重叠、出界或被裁；
暗色主题下文字可读；高亮点 / 高亮柱 / 高亮扇区画对了。看不了图就写「未目检」，不要写「已确认一致」。

## 分支与提交

从 `master` 新建分支 `feat/typst-charts`，按顺序提交（每步提交前跑完该步的验证）：

1. `feat(typst): chart tokens in every theme`（B）
2. `feat(typst): draw charts natively`（C + E；此时旧管道还在，`chart:` 素材存在时也忽略它、一律原生绘制）
3. `refactor: drop the webview chart SVG pipeline`（D）

## 验证

```bash
cargo fmt --all
cargo test --workspace
bun run build
```

真机验证（建议）：`.claude/launch.json` 的 `tauri-dev` 配置打开 WebView2 调试端口 9223，按 `CLAUDE.md`「真机验证」用 CDP 驱动。
找一个含图表页的项目，看 Step2 看板、编辑器预览、演示主画面里的图表都是 Typst 画的（`<img>` 而非 ECharts 的 `<svg>`/`<canvas>`），
并导出一次 PDF。`tauri dev` 是 debug 编译，耗时不能当性能结论。

## 最终汇报（按此格式，不要省略）

1. 三个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行，失败就如实贴）
3. `grep` 0 命中的输出
4. 每种图与参考图的对照：刻度、标签、高亮是否一致，有哪些可见差异
5. `charts-edge.json` 每一页的结果（正常绘制 / 占位）
6. 与本说明或屏幕版不一致的实现及原因
7. 没做完或不确定的点
8. 是否做过目检、是否做过真机验证、各验证了什么。**没做就写「未做」**，不要写「已验证」。
