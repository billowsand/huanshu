# 任务：幻灯片缩略图改用 Typst 出图（第一步）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio，Tauri v2 + Vue 3 + Rust）里工作。
开始前先读 `CLAUDE.md` 的「Typst 原生 PDF 导出」与「图表页（`chart_focus`）」两节，再读本说明里点名的文件。

## 背景

幻灯片现在有两套版式实现：屏幕用 `src/components/Keynote*.vue`（经 `SlideRenderer.vue`），
PDF 用 `crates/slide-typst`（嵌入式 Typst）。长期目标是让 Typst 成为唯一的幻灯片渲染器。
第一步只换**缩略图**：缩略图一次展示很多页，每页都挂一个完整 Vue 幻灯片组件（玻璃主题还带
`backdrop-filter`）很重；换成 Typst 光栅化出的 PNG，用 `<img>` 显示。

实测参考（release，17 页测试牌组）：冷编译整副约 220ms，改一页后整副重编约 20ms，
`typst-render` 光栅化在 2px/pt 时每页约 18ms（缩略图分辨率更低，会更快）。

## 范围

**要改的只有两处缩略图：**

1. `src/views/steps/Step2Generate.vue` 生成进度网格里每张卡片的预览（`.spc-preview-frame` 内的 `SlideRenderer`，约第 473 行）
2. `src/views/PresentationOverlay.vue` 全览模式的缩略图（`.pres-overview-thumb` 内的 `SlideRenderer`，约第 364 行）

**不许改：**

- Step2 右侧详情大图（约 510 行）、演示主画面（约 341 行）、Step3 编辑器实时预览——继续用 `SlideRenderer`
- 任何 `.typ` 版式/主题文件、任何 `Keynote*.vue`、`SlideRenderer.vue`
- PDF 导出的行为与产物（只允许做下面说的「抽函数」重构，行为必须不变）
- `CLAUDE.md`、`README`、`CHANGELOG`（文档由审阅方更新）

## 设计（照此实现，不要自行换方案；与代码事实冲突时以代码为准，并在汇报里说明）

### R1. `crates/slide-typst`：整副编译，按需光栅化成 PNG

- `Cargo.toml`：把 `typst-render = "0.15.1"` 从 `[dev-dependencies]` 移到 `[dependencies]`，版本不变。
- `src/lib.rs` 新增：

  ```rust
  pub struct RenderedPng { pub index: usize, pub png: Vec<u8>, pub width: u32, pub height: u32 }
  pub struct RenderedPages { pub pages: Vec<RenderedPng>, pub overflows: Vec<Overflow>, pub missing_assets: Vec<MissingAsset> }

  impl Renderer {
      /// Compiles the whole deck once and rasterizes the requested pages (all when `only` is None).
      pub fn render_png(&self, slides: &[serde_json::Value], options: &ExportOptions,
                        width_px: u32, only: Option<&[usize]>) -> Result<RenderedPages, RenderError>
  }
  ```

  - **必须整副编译**（复用现有 `self.compile`），不能单页编译再拼：`ctx.index` 被 `chart:<index>`
    素材键和溢出标记使用（见 `typst/main.typ` 与 `layouts/chart-focus.typ`）。
  - `width_px` 夹到 `64..=3840`；`pixel_per_pt = width_px / 页面宽度(pt)`；
    `typst_render::render(page, &RenderOptions { pixel_per_pt: ..., render_bleed: false })`，
    `pixmap.encode_png()`。`Scalar` 的导入路径以编译器为准。
  - 页数多于 `slides.len()` 时（版式溢出产生的额外页）忽略多出的页；`only` 里越界的下标忽略。
- `tests/render.rs` 加测试：对 `fixtures/all-layout-blueprints.json` 中受支持的页，`only = [0, 3]`
  返回恰好 2 张；PNG 以 `\x89PNG` 开头；宽度等于请求值；16:9 高度 ≈ 宽 × 9/16（±1px）；
  `aspect_ratio: "ratio_32x9"` 时高度 ≈ 宽 × 9/32。
- `examples/render_fixture.rs` 加参数 `--thumbs <width_px>`：把每页 PNG 写到输出目录
  `thumb-NN-<kind>.png`，并打印整副耗时与每页平均耗时（汇报里要用这个数字）。

### R2. `src-tauri`：`render_slide_thumbnails` 命令

- 新建 `src-tauri/src/commands/thumbnails.rs`，在 `commands/mod.rs` 和 `lib.rs` 的 `generate_handler!` 里登记。
- **复用** `commands/pdf.rs` 的 `renderer()`、`prepare_export()`、`ChartSvgInput`（改成 `pub(crate)`），
  **不要复制它们的逻辑**：素材引用写法只能有一套（`export_assets` / `assets::project_file_refs`），
  否则生成时合法的图片会在缩略图里找不到。
- 命令签名：

  ```rust
  #[tauri::command]
  pub async fn render_slide_thumbnails(
      project_id: Option<i64>,
      slides: Vec<SlideBlueprint>,          // 必填：缩略图永远渲染屏幕上的那份牌组
      theme: Option<String>,
      chart_svgs: Option<Vec<ChartSvgInput>>,
      width_px: u32,
      only: Option<Vec<usize>>,
      state: State<'_, Mutex<AppState>>,
  ) -> Result<ThumbnailBatch, String>
  ```

  `ThumbnailBatch { pages: Vec<Thumbnail>, theme_fallback: Option<String>, elapsed_ms: u128 }`，
  `Thumbnail { index: usize, data_url: String /* data:image/png;base64,... */, placeholder: bool }`。
  `placeholder` = 该页 kind 不在 `slide_typst::SUPPORTED_KINDS`。base64 用已有的 `base64 = "0.22"`。
  渲染放进 `tauri::async_runtime::spawn_blocking`。面向用户的错误信息用中文（与 `pdf.rs` 一致）。
- 把同步部分拆成一个纯函数（类似 `pdf.rs` 的 `render_to_file`），给它写单测：3 页（含一个不存在的
  kind）→ `only = [0, 2]` 只返回 2 张、`data_url` 前缀正确、未知 kind 的 `placeholder == true`。
- 预热：`lib.rs` 的 `tauri::Builder` 目前没有 `.setup`，加一个，在后台线程里调用一次
  `commands::pdf::renderer()`，让首张缩略图不用等约 0.5 秒的系统字体扫描。

### F1. 抽出图表 SVG 构建（纯重构）

`src/composables/usePdfExport.ts` 约第 56–80 行在导出前构建 `chartSvgs`（`pdf_chart_boxes` →
`renderChartSvg`）。把它抽到 `src/composables/chartSvgs.ts`：

```ts
export async function buildChartSvgs(
  slides: SlideBlueprint[],
  opts: { projectId: number | null, themeId: string | null },
): Promise<Array<{ index: number, svg: string }> | null>
```

`usePdfExport` 改为调用它，传给 `pdf_chart_boxes` 的参数与现在完全一致。注释保持原有的中文说明。

### F2. `src/composables/useTypstThumbnails.ts`

```ts
export function useTypstThumbnails(opts: {
  slides: () => Array<SlideBlueprint | null | undefined>  // 下标 = 牌组页序；null 表示该页还没生成
  projectId: () => number | null
  widthPx: () => number                                   // 显示宽度 × devicePixelRatio
  enabled?: () => boolean
}): {
  thumbs: Ref<Record<number, { url: string, placeholder: boolean }>>  // 以牌组页序为键
  failed: Ref<boolean>
}
```

要点（每条都要做到）：

1. **子集与页序映射**：Step2 生成中会有空位。只把已有蓝图组成子集数组发给 Rust，同时保存
   `deckIndex[subsetPos]`。Typst 的 `ctx.index`、返回的 `index`、`chart:<index>` 都是**子集位置**；
   `buildChartSvgs` 必须用**同一个子集数组**调用；结果再映射回牌组页序。
2. **增量**：每页的缓存键 = `JSON.stringify(slide)` + 子集位置 + 主题 id + 字体设置 + 宽度档位。
   只把键变了的页放进 `only`；没有变化就不调用命令。
3. **主题**：`document.documentElement.getAttribute('data-theme')`；用 `MutationObserver`
   （`attributeFilter: ['data-theme']`）监听，变了全部失效；卸载时 `disconnect`。
4. **字体**：监听 `src/stores/appSettings.ts` 里的 `settings.fonts`（deep），变了全部失效。
5. **宽度档位**：`widthPx` 向上取整到 64 的倍数，避免窗口每变 1px 就重渲。
6. **防抖与竞态**：200ms 防抖；请求带递增序号，丢弃过期响应；新图到达前保留旧图（不闪烁）。
7. **失败**：命令报错时 `failed = true`，`console.warn` 一次（带错误信息），视图回退到 `SlideRenderer`。
8. **开关**：`localStorage['huanshu.typstThumbnails'] === '0'` 时整体禁用（读写包 `try/catch`），便于 A/B 对比。
9. 类型用 `src/components/types.ts` 里的，不要新增 `any`。

### F3. 接入两个视图

- **Step2Generate**：`.spc-preview-frame` 内，有缩略图且不是 `placeholder`、未 `failed` 时显示
  `<img class="spc-preview-img" :src draggable="false" alt="">`（`width/height: 100%`、
  `object-fit: contain`）；还没拿到图时显示一个简单的加载底（沿用 frame 的比例，浅色背景即可，
  不要挂 `SlideRenderer`）；`placeholder` 或 `failed` 时走原来的 `SlideRenderer` 分支（保留原代码）。
  `widthPx = ceil(slideDims.w * previewScale * devicePixelRatio)`。
- **PresentationOverlay**：新增 prop `projectId?: number | null`，在 `WorkflowView.vue`（约 561 行）
  传 `projectId`；`enabled` 只在全览模式可见时为真（不要在每次播放时渲染）。图片放进现有的
  `.pres-overview-thumb`（256×144）并 `object-fit: contain`——这也顺带修掉现有缩略图写死
  `1280×720 scale(0.2)`、不随 32:9/48:9 变化的问题。回退分支保留原代码。

## 约束

- 除了把 `typst-render` 挪到正式依赖，不新增任何 cargo / npm 依赖。
- 代码风格贴合所在文件：Rust 注释用英文、面向用户的报错用中文；TS/Vue 注释跟随所在文件。
- 不要运行 `bunx tauri build` 或 `scripts/package.ps1`；不要 push。
- 不要改动与本任务无关的文件；发现别的问题写进汇报，不要顺手修。

## 分支与提交

从 `master` 新建分支 `feat/typst-thumbnails`，按下面四步各提交一次（每步提交前跑完该步的验证）：

1. `feat(typst): rasterize deck pages to PNG`（R1）
2. `feat: render_slide_thumbnails command`（R2）
3. `refactor: share chart SVG building between PDF export and thumbnails`（F1）
4. `feat: Typst-rendered thumbnails in generation grid and presentation overview`（F2 + F3）

## 验证（全部通过才算完成）

```bash
cargo fmt --all
cargo test -p slide-typst
cargo test -p auto-slidev-studio --lib thumbnails
cargo test -p auto-slidev-studio --lib pdf
cargo run --release -p slide-typst --example render_fixture -- --thumbs 560
bun run build
```

`bun run build` 会跑 `vue-tsc`，类型错误必须清零。

## 最终汇报（按此格式，不要省略）

1. 四个提交的 hash 与一句话说明
2. 上面每条验证命令的结果（贴输出末尾几行，失败就如实贴失败）
3. `--thumbs 560` 的整副耗时与每页平均耗时
4. 与本说明不一致的实现及原因
5. 没做完或不确定的点
6. 是否做过真机验证（`bunx tauri dev` 实际看到缩略图）。**没跑就写「未做真机验证」**，不要写「已验证 UI」。
