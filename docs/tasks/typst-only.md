# 任务：删除 Vue 幻灯片模板，Typst 成为唯一的幻灯片渲染器（第五步）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio，Tauri v2 + Vue 3 + Rust）里工作。
开始前先读 `CLAUDE.md` 的「Typst 原生 PDF 导出」整节（尤其「Typst 缩略图」「编辑器实时预览与 Step2 详情」两小节）
和「图表页」小节，再读本说明点名的文件。前四步的说明在 `docs/tasks/` 下（`typst-thumbnails.md`、`typst-stage-and-fit.md`、
`typst-editor-preview.md`、`typst-charts.md`）。

## 背景

所有幻灯片画面已经由 Typst 出图：Step2 看板缩略图与详情大图、演示全览与主画面、Step3 编辑器预览、PDF 导出，图表也由 Typst 原生绘制。
Vue 模板（18 个 `Keynote*Slide.vue` + `SlideRenderer.vue`）现在只剩一个用途：**图还没出来或出图失败时的回退**。
本步删除它们，Typst 成为唯一的渲染器。这是迁移到原生 Rust 界面之前，前端里最大的一块与幻灯片相关的代码。

已确定的产品决定（不要改）：
1. 封面、结尾、SWOT 的入场动画，spotlight 的轮播（PDF 与 Typst 图里是平铺），一律不再有。
2. 三个 localStorage 开关 `huanshu.typstThumbnails` / `huanshu.typstStage` / `huanshu.typstEditorPreview` 删除：已经没有可以回退的东西了。
3. 没有图时显示**加载框**（见 B1），不再有幻灯片组件兜底。

## 前几轮审查发现的问题（不要再犯）

1. **注释、提交信息与代码不一致**：写下的每一条行为都要和代码相符；删掉的东西，相关注释也要一起改掉（比如「回退到幻灯片组件」）。
2. **混用下标空间**：`useTypstThumbnails` 里 `thumbs` / `overflow` / `lastUsed` 以数组下标为键，`shownKeys` / `pendingKeys` 以子集位置为键。
3. **报告与事实不符**：没做的就写没做，没验证的就写没验证。上一轮的汇报说刻度「逐例核对通过」，实际输出却是错的。
4. **只删不查**：每删一类东西，用 `grep` 确认 0 命中再提交；动态拼接的类名（`` `tone--${x}` ``、`:class` 里的字符串）`grep` 搜不到完整名字，删 CSS 前要专门查。

## 范围

### 删除

- `src/components/Keynote*Slide.vue`（18 个）、`SlideRenderer.vue`
- 只被模板使用的组件：`GlassCard.vue`、`TagBadge.vue`、`CardGrid.vue`、`ImageCarousel.vue`、`StepNumber.vue`、`SlideReveal.vue`、`SlideTitle.vue`
- 已经没有任何地方使用的组件：`SlideCard.vue`、`PipelineProgress.vue`（删之前再 `grep` 一次确认）
- `src/utils/richText.ts`、`src/utils/titleTail.ts`（只被模板和上面的组件使用）
- `experiments/typst/preview/`（Vue 对照页，依赖模板）与 `experiments/chart-spike/`（ECharts 试验）
- `src/main.ts` 里这些组件的 import 与 `app.component(...)` 全局注册
- `package.json` 的 `echarts` 依赖（删完模板后只有 `chartOptions.ts` 用它），并 `bun install` 更新锁文件

### 保留并改造

- `src/components/chartOptions.ts` → 改名 `src/utils/chartSpec.ts`，只留 `ChartSpec` 等类型、`chartProblems()`（Step3 编辑器校验图表 JSON 用）
  和 `chartSourceLabel()`（如果删完后没人用就一起删）；删掉 `buildChartOption`、`themeFromCssVars`、`echarts` 导出、`TYPE_SCALE_BASE`。
  `src/components/types.ts`、`Step3Editor.vue` 的 import 跟着改。`chartProblems` 与 Rust `ChartSpec::problems()` 逐条对应的注释保留。
- `katex`、`markdown-it`、`@vscode/markdown-it-katex` 保留：Step1 的 `MarkdownWorkbench` 还在用（`main.ts` 里的 `katex.min.css` 也保留）。

### 不许改

Rust 代码（`src-tauri/`、`crates/`）、Typst 模板、`useSlideEditor.ts` 的 `KIND_META` / `KIND_FIELDS` / `KIND_DEFAULTS`、Step3 的 `FIELD_DEFS`、
Studio 外壳的样式与行为、`CLAUDE.md` / `README` / `CHANGELOG`（文档由审阅方更新）。

## A. 改造三个视图（先做，此时模板还在，便于对照）

现在三个视图都是「有图显示 `<img>`，否则 `SlideRenderer`」。逐一改成只用 Typst 图：

| 位置 | 现在的回退 | 改成 |
|---|---|---|
| `Step2Generate.vue` 看板卡片（`thumbnailFor`） | `SlideRenderer` | 加载框 |
| `Step2Generate.vue` 详情大图（`detailThumbnails`） | `SlideRenderer` | 加载框 |
| `PresentationOverlay.vue` 全览（`thumbnails`） | `SlideRenderer` | 加载框 |
| `PresentationOverlay.vue` 主画面（`stage`，`stageImageEnabled`） | `SlideRenderer` | 加载框 |
| `Step3Editor.vue` 预览（`TYPST_EDITOR_PREVIEW_KEY`） | `SlideRenderer` | 加载框 |

- 删掉三个开关及其判断函数；`enabled` 只保留与开关无关的条件（例如全览是否打开）。
- 判断「有没有图」时**不再**因为 `failed` 或 `placeholder` 把已有的图藏起来：
  - `failed` 为真但该页已有（旧的）图：继续显示旧图，同时显示错误提示（见 B1）；
  - `placeholder` 为真：Typst 返回的是「版式尚未实现」的占位页图片，**照常显示这张图**（现在 18 个 kind 都有版式，这种情况只出现在未来新增 kind 时）。
- 只为 `SlideRenderer` 服务的包装层一并删掉：带 `transform: scale()` 的画布（`.sdp-preview-canvas`、`.edit-preview-canvas`、`.pres-slide`）、
  计算缩放的函数和只被它们用到的 props（例如传给 `SlideRenderer` 的 `mediaMap`，删之前 `grep` 确认没有别的用途；`WorkflowView.vue` 里构造它的代码也随之删除）。
  `<img>` 的尺寸逻辑（铺满舞台、`object-fit: fill`、宽度档位 × dpr）不变。
- 主画面的预取范围从「当前 ±2」改为「当前页前 1 页到后 3 页」（往后翻比往前翻多得多），`maxCached` 从 8 改为 10。

## B. 加载框与错误状态

### B1. 新组件 `src/components/SlideFrame.vue`

一个按幻灯片比例占位的框，三个视图共用：

```ts
props: {
  src?: string | null          // 有图时显示 <img>，铺满本组件
  pending?: boolean            // 正在出图
  error?: boolean              // 命令失败
  alt?: string
}
emits: ['retry']
```

- 有 `src`：显示图片；`error` 为真时右下角叠一个小的「渲染失败 · 重试」按钮（不遮挡画面主体）。
- 没有 `src`：显示主题纸色（`var(--bg-elevated)` 一类的 Studio token，挑现有的、在 6 个主题下都合适的）的空框；
  `pending` 持续超过 300ms 才淡入一个不抢眼的「渲染中…」文字（避免每次翻页都闪）；`error` 时显示「渲染失败」和「重试」按钮。
- 「重试」调用 `useTypstThumbnails` 返回的 `refresh()`（已存在），不要另写请求逻辑。
- 组件本身不关心比例：外层容器决定尺寸（三个视图现在都已经有按比例算好的舞台尺寸）。

### B2. `useTypstThumbnails` 的注释与行为

- 删掉所有「回退到幻灯片组件 / SlideRenderer」的说法，改成「调用方显示加载框 / 错误」。
- `failed` 的语义不变（最近一次请求失败）；成功一次后复位（现在已经如此）。
- 不要改键、下标、`serial` / `progressive` / `resetKey` / generation 的逻辑。

## C. 删除模板与清理

按「范围 / 删除」逐项删除。删完后以下命令都必须 0 命中（贴出输出）：

```bash
grep -rn "SlideRenderer\|Keynote[A-Za-z]*Slide\|GlassCard\|TagBadge\|CardGrid\|ImageCarousel\|StepNumber\|SlideReveal\|SlideTitle\|SlideCard\|PipelineProgress" src --include=*.vue --include=*.ts
grep -rn "richText\|titleTail\|buildChartOption\|themeFromCssVars\|echarts\|huanshu.typst" src --include=*.vue --include=*.ts
grep -n "echarts" package.json
```

（`src/themes/catalog.ts` 等处如果有说明性文字提到这些名字，按实际情况改写，不要留下指向已删除文件的说明。）

## D. 只服务于幻灯片模板的 CSS

模板删掉后，大量 CSS 不再命中任何元素。**单独一个提交**清理，规则：

1. 先写一个一次性脚本（放 `scripts/` 之外，例如 `target/css-audit.mjs`，不提交）：解析 `src/style.css` 和 `src/themes/*.css` 的每条规则，
   取出其中的类名，在 `src/**/*.vue`、`src/**/*.ts`、`index.html` 里搜索；输出「所有类名都搜不到的规则」清单到 `target/css-unused.txt`。
2. 删除清单里的规则。以下几类**整体**属于幻灯片，可以按块删：
   - `src/themes/notes-layouts.css`：先确认它的选择器全部是幻灯片模板的类名，是的话整个文件删掉并去掉 `main.ts` 里的 import；
   - `src/style.css` 里 `.slide-shell` 的 `--type-*` / `--type-scale` 字号体系、`.pres-slide, .slidev-slide` 的尺寸变量、`.slide-shell__ambient` 等；
   - `src/themes/typography.css` 里 `.slide-shell` / `.slidev-slide` 的字体角色分组。**`.md-preview` 和 `code`/`pre` 的部分保留**（Step1 Markdown 预览在用），
     只删去选择器里的 `.slide-shell, .slidev-slide`；
   - `terminal.css` / `editorial.css` / `notes.css` 里针对幻灯片元素的规则（如 `.glass-card`、`.section-num`、`.semantic-title__tail`）。
3. **主题 token（`--primary`、`--bg`、`--chart-*` 等 CSS 变量）一个都不删**：Studio 外壳在用，而且 Typst 主题的取值以它们为参照。
   `--chart-*` 删完 ECharts 后屏幕上已无人读取，但保留，作为 Typst 主题 `chart` 字典的对照来源。
4. 动态类名：`grep` 一下 `` ` `` 模板字符串和 `:class` 里拼出来的类名前缀（例如 `tone--`、`status--`），确认它们属于 Studio 还是幻灯片，再决定删不删。
5. 删完后 6 个主题下逐一打开首页、设置页、Step1/2/3、演示，确认 Studio 外壳样式没有变化（见「验证」）。

## 分支与提交

从 `feat/typst-charts` 新建分支 `feat/typst-only`，按顺序提交（每步提交前跑完验证）：

1. `feat: slide frames instead of the Vue fallback`（A + B：三个视图只用 Typst 图，`SlideFrame` 加载框与重试，删除三个开关；模板此时还在但已无人引用）
2. `refactor: remove the Vue slide templates`（C）
3. `refactor(css): drop slide-only styles`（D，提交信息里写删了哪些文件、多少条规则）

## 验证

```bash
bun install
bun run build
cargo test --workspace
```

真机验证（**必须做**，这一步改的全是界面）：`.claude/launch.json` 的 `tauri-dev` 配置打开 WebView2 调试端口 9223，按 `CLAUDE.md`「真机验证」用 CDP 驱动
（首次加载可能卡在空白，`location.reload()` 一次；替换 `__TAURI_INTERNALS__.invoke` 拦截不到应用自己的调用；演示的翻页键监听在 `window` 上）。
用一个有图表页的项目，至少确认并各截一张图：
- Step2 看板缩略图与详情大图；
- Step3 编辑器：切换页面时先出加载框再出图、连续改标题时预览跟着更新；
- 演示：主画面、连续翻页、用页码跳到远处（超出预取范围）时出现加载框而不是空白、全览；
- 切换主题时，旧图保持到新图出来（不闪成加载框）；
- 6 个主题下首页与设置页的外观与改动前一致（改动前先截一组基线图，存到 `target/ui-baseline/`）。

`tauri dev` 是 debug 编译，Typst 出图慢（每页几百毫秒），加载框出现得会比发布版频繁，这是预期的。

## 最终汇报（按此格式，不要省略）

1. 三个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行，失败就如实贴）
3. C 节三条 `grep` 的输出
4. D 节：删除的文件、删除的规则数、保留下来但不确定是否还在用的规则（列出来交给审阅方判断）
5. 真机验证：逐条列出做了什么、看到什么，附截图路径。**没做的写「未做」**
6. 与本说明不一致的实现及原因
7. 没做完或不确定的点
