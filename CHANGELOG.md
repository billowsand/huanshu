# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-10-14

### Added

- **原生 PDF 导出（内嵌 Typst）**：新增 `slide-typst` crate，在进程内用 Typst 把蓝图直接排版成矢量 PDF，不需要浏览器或外部 CLI；编辑器预览栏的「导出 PDF」与首页导出弹窗的「PDF 文档」走同一流程，导出的是当前**已应用**的牌组（与预览、演示一致），完成后可一键在文件管理器中定位文件。18 种模板 × 6 个主题全部移植，尚未移植的模板渲染为「Typst 版式尚未实现」占位页而不是丢页；结果弹窗列出占位页、内容放不下被裁切的页、缺失的图片，以及主题 id 不受支持时的回落提示。一页幻灯片恰好对应一个 PDF 页面：内容先按实测二分缩放（下限 0.4×），仍放不下才裁切并如实报告；图片沿用生成时同一套引用规则解析（项目 `figure/`、`public/` 与素材库 `media:<id>`），编译前逐张完整解码校验（BMP 转 PNG、SVG 校验），单张坏图或缺失文件只在对应位置显示占位图，不会让整份导出失败。模型输出的文本一律按纯文本渲染，绝不作为 Typst 标记求值。

- **报刊·钴蓝主题**（`light-editorial`）：取样自报刊/杂志版式——暖米白新闻纸、近黑墨色、唯一强调色钴蓝，无阴影、无圆角；卡片在屏幕上打开成「上方一条细线」的报刊分栏，标题落在 2px 墨色线上，副标题为浅蓝带，封面/结尾徽章为实心钴蓝标签，页面顶部一条钴蓝色带。封面与结尾标题在第一个全角逗号/冒号（或破折号、竖线）后断开，后半句换行并改用钴蓝（双色大标题，屏幕 `src/utils/titleTail.ts` 与 PDF `title-tail` 同一规则，其他主题无视觉变化）。PDF 导出同步支持（第一期沿用终端主题几何，卡片保留细线全框），应用界面配色随主题切换。

- **数据图表页（ChartFocus）**：Markdown 正文新增 ` ```chart ` 围栏块携带图表数据（line / bar / donut / grouped_bar / stacked_area），归属所在 H2/H3 小节；正文较短时每小节首图与文字同页生成「要点解读 + 图表」版式，正文较长时文字页与图表页拆成两页，多图自动各追加一页；带图表的页固定使用 `chart_focus`（不经 LLM 选版式，也不会在 repair 中被换掉），图表数据由代码从文档原样挂载、模型只写解读。图表渲染单一实现（`buildChartOption`，ECharts）：webview 直接挂载，导出 PDF 时在 webview 内 SSR 出 SVG、经 asset 管道（`chart:<页码>`）嵌入 typst，双端一致；导出前先由 typst 测量每张图的绘图框与字号比例（`pdf_chart_boxes`），SVG 按实测尺寸渲染、在 PDF 中 1:1 落位，图表字号随 `--type-scale` 与其他文字一起缩放；Rust 侧修正 SSR SVG 里 font-family 的裸引号后经 usvg 校验。5 套主题新增 `--chart-*` token（palette/grid/axis），配色随主题切换（屏幕上切换主题/字体会即时重绘）。Step3 编辑器可直接编辑图表数据 JSON（与生成端同一套校验）。
- **第 18 种幻灯片模板** `chart_focus`：布局规则、评分、schema 提示词、校验（chart 必填、type/encoding 合法、数值列必须为数字；数据错误在解析 Markdown 时直接报给用户，不进入 LLM 修复）、Step3 编辑器字段（要点列表 + 图表标题）全部接入。
- **行内数学公式**：蓝图文本中的 `$...$` / `$$...$$` 现在会真正渲染——屏幕与预览由 `src/utils/richText.ts` 用 KaTeX 输出 HTML（白名单标签只输出裸标签，属性一律丢弃，结果进 `v-html`），PDF 由 `crates/slide-typst/src/math.rs` 经 mitex 转成 Typst 数学后在 `rich()` 里 `eval`。三处定界规则（前端、生成侧 `generator/utils.rs` 的公式保护、PDF 侧）保持一致：`$$` 块可跨行，`$` 内联不跨行且首尾不得为空白，`$` 后紧跟数字不算闭合，`\$` 为字面美元号；输入里的哨兵字符会先清除，防止伪造公式段注入 Typst 代码（有单测）。Typst 的 `eval` 出错无法捕获，导出前先用 `math-check.typ` 一次性试编译全部公式，失败的公式退回原文显示；内嵌的 mitex spec 在 `standard.typ` 打了本地补丁（上游 xarrow 依赖 Typst 已删除的 `style()`，且为 GPL-3.0，故不 vendor）。

### Fixed

- **PDF 终端主题章节号位置**：终端主题下的小号章节号保持 CSS 的 `top: 16px; right: 32px` 盒位（内容角上方 21px、右侧内缩 5px），不再与页面标题齐平，也不再被 `section_intro` 的卡片压住。
- **PDF split_layers 标题被长副标题挤扁**：改为按实测宽度等比收缩（等价 CSS `flex-shrink`），不再用会让右侧先取走整个自然宽度的 `(1fr, auto)` 网格，长 meta 不会把标题挤成一字一行。
- **PDF 主题回落提示**：主题 id 不受支持而回落到便签·稿纸时按警告列出，不再显示为正常的绿色对勾。

## [0.3.0] - 2026-10-11

### Added

- **素材库图片参与生成**：素材库图片（`media:<id>`）现在与项目 `figure/`、`public/` 文件一起构成生成时的素材目录，enrich 与内容生成提示词会带上图片的图注与宽高比，`preferred_assets` 也接受素材库引用，`spotlight` 布局不再因为看不到素材而被整体跳过；幻灯片缺少图片时会用页面规划选中的素材兜底填充。
- **图片尺寸元数据**：导入素材时从文件头读取像素尺寸（新增零依赖 `image_size` 模块，支持 PNG / JPEG / GIF / BMP / WebP / SVG）写入 `media_assets.width`/`height`，素材列表显示尺寸；`.keynn` 导出/导入携带宽高，旧包回退到解析还原后的字节。
- **图文配对**：新增「图 → 文稿」配对阶段，把分页大纲与每张素材图片一起交给多模态模型，判断这张图适合放在哪一页、在该页起什么作用，并产出带文稿语境的图注；结果作为该页 `preferred_assets` 的种子和提示词中的「Matched to this page」推荐，图注回写素材库，配对明细输出到调试目录 `00-media-match.json` 并记入生成日志。未配置多模态模型时自动跳过，行为与旧版一致。

- **失败页手动重试**：生成详情新增“重试这一页”，弹框选择可用布局后使用原文重新生成、校验并保存单页；其他页仍在生成时先排队，保留人工所选布局，支持再次失败后继续重试。保留兜底页和错误状态，可手动前往编辑；存在失败页时不再自动跳转。

- **便签·稿纸主题**：新增「便签·稿纸」，应用界面取样为暖棕控制件 + 浅木色底 + 米色稿纸卡片（木纹纤维来自新增的 `src/themes/notes-wood.svg`），幻灯片统一渲染为带竖向装订线的单横线稿纸；标题、标签、步骤点等复用既有配色变量，预览、缩略图与播放共用样式。

### Changed

- **设置页重构**：改为左侧分区导航（外观与字体 / 模型服务 / 生成设置 / 存储与缓存 / 关于与帮助），所有改动先落在草稿上，
  与基线逐项对比后才会保存，未保存时给出提示；三个模型服务合并为页签切换，可逐个「测试连接」并显示耗时与状态徽标，
  模型名支持从服务拉取或手动输入；主题改为色块选择并按深浅分组；字体面板新增幻灯片 / 界面两套预览与「统一字体」开关。
  样式表拆分为 `src/views/settings.css`。
- **系统字体选择器**：改为受控弹层，打开时才懒加载系统字体；支持关键词过滤、常用字体置顶、方向键与 Home/End 导航、
  焦点陷阱，并可直接「应用到全部角色」。`font-sample` 样式收回到 `.font-preview`（`notes-studio.css`）。
- **设置页跟随应用主题与界面字体**：设置页与字体选择弹层不再写死一套浅绿配色（也不再强改标题栏）和微软雅黑，
  颜色全部取自各主题的 `--studio-*` 变量、文字使用「界面」字体，点主题或改界面字体即整页实时预览；主题选择改为平铺全部 5 个主题，缩略图用各自主题的真实配色绘制；
  主题清单收敛到 `src/themes/catalog.ts`，`main.ts`、`ThemeToggle` 与设置页共用。
- **设置页交互**：有未保存更改时离开会弹窗询问（保存并离开 / 放弃 / 继续编辑），不再静默丢弃草稿并回滚预览；
  支持 Ctrl+S 保存；服务状态徽标区分已连接 / 失败 / 测试中；最小字号由 9–11px 提到 12px，文字链接改为按钮。
- **幻灯片字体角色修正**：字体角色规划表（`src/themes/typography.css`）此前只覆盖一部分类名，并把「标题」
  错归到标签组（`issue_stack` 卡片标题），有的文字干脆没有任何角色——`feature_grid` 卡片标题、`swot` 象限标题、
  `process` 阶段标题等退化成界面字体，时间轴日期与步骤序号借用代码字体、卡片正文借用标签字号类。
  现在 6 类角色覆盖全部 17 个模板：默认角色改为正文（漏归类只会退化成正文，不再掉进界面字体），
  新增 `.slide-font-{title,subtitle,body,caption,number,code}` 逐元素钩子，`src/style.css` 里标题、封面副标题、
  章节号、脚注的 `--font-sans`/`--font-mono` 改为对应角色变量，模板中已被覆盖成死代码的 `font-mono`/`text-mono` 一并清除。
  各角色分组统一限定在幻灯片容器内、特异性一致，Studio 页面列表（`SlideCard`）里的 `.slide-title` 不再被换成标题字体；
  `terminal.css` 对幻灯片标签的 `font-mono`（恒被覆盖）改为只调字距；Process「关键动作」计数归入数字、SWOT 策略结论归入正文。

### Fixed

- **单页重试卡死**：单页重试要求整副幻灯片同时通过校验才肯保存，两页以上有问题时任何一页都重试不过去，
  修好的那页也被丢弃；而只在最终校验阶段出错的页面根本没被标成失败，界面上无从重试。现在最终校验失败时，
  逐页记录 `page_error` 并推送失败状态（`ValidationIssue` 新增 `slide_index`）；单页重试只要该页自身通过就写入，
  整副通过校验后才把本次生成记为完成，否则返回剩余问题（`RetryPageResult`）并把仍有问题的页标为可重试。
  封面、目录、结尾的问题会单独提示需整副重新生成或手动编辑。
- **放映残留任务栏黑边**：窗口处于最大化时开始放映，tao 会把无边框窗口的客户区按显示器工作区裁剪（`WM_NCCALCSIZE`），
  即使 WebView 已进入全屏，屏幕底部仍留出任务栏高度的黑条。现在进入放映前先记录并取消最大化，退出放映
  （含直接卸载组件、用户按 Esc 或拒绝全屏）时先退出全屏再恢复最大化。配套补充 `core:window:allow-set-fullscreen` 窗口权限。
- **流程页步骤说明被截断**：`WorkflowView` 的侧栏描述类名 `.step-desc` 与 `Process` 幻灯片的步骤说明同名，全局样式让幻灯片文案被限宽省略号截断；改为 `.steps-nav .step-desc` 只约束侧栏，幻灯片内步骤说明恢复完整换行。新增 `scripts/check-notes-theme.mjs` 与 `check-slide-layout.mjs` 的 Process 换行断言守护该回归。

### Removed

- **四套亮色主题**：删除「亮·蓝图」「亮·翠绿」「亮·晨曦」「亮·多巴胺」及其 slide/studio 变量块；
  `localStorage` 中残留的旧主题 id 启动时自动回退到「暗·琥珀」。剩余主题：暗·琥珀、暗·星图、暗·极光、终端·墨、终端·纸。
- **「暗·极光」主题**：删除「暗·极光」（`dark-glass-blue`）及其幻灯片、界面两组变量块；
  `localStorage` 中残留的 `dark-glass-blue` 启动时自动回退到「暗·琥珀」。剩余主题：暗·琥珀、暗·星图、终端·墨、终端·纸、便签·稿纸。

## [0.2.0] - 2026-10-10

### Added

- **字体与个性化**：设置页新增字体面板，界面 / 标题 / 副标题 / 正文 / 注释 / 数字 / 代码 七类角色可各自指定字体，
  支持按中文名或英文名搜索本机字体（Rust 侧用 DirectWrite 枚举系统字体族）、一键应用到全部角色与恢复默认，
  修改后实时预览，保存失败会回滚。字体偏好持久化到 `settings.json` 与数据库，旧配置缺 `fonts` 字段时默认微软雅黑。
  幻灯片组件同步补充 `slide-subtitle` / `slide-number` / `slide-footnote` 等语义类，使主题与字体角色解耦。

- **终端明暗主题**：新增「终端·墨」与「终端·纸」，应用界面与全部 17 种幻灯片模板统一使用暖黑 / 纸白底色、细线边框、近直角卡片和等宽标签；预览、缩略图与播放共用样式。

- **模型服务 SOCKS5 代理**：大语言模型 / 向量模型 / 多模态模型可各自启用 SOCKS5 代理，
  支持用户名密码认证（支持 `socks5://` / `socks5h://`），模型域名由代理解析；
  代理不可用时返回错误而不回退直连。旧 `settings.json` 无 `proxy` 字段时默认关闭。

### Fixed

- **Spotlight 面板条目上限**：`normalize` 对每个面板的 `items` / `steps` 收口为最多 4 项（原先无上限），
  与 FeatureGrid / IssueStack 既有的 `card.items.truncate(4)` 保持同一内容规范。

- **标题按词换行**：全部幻灯片主标题使用 Rust jieba 分词，保持词语与标点完整，均衡多行长度，并保留手动换行，避免长标题末尾单字孤行。

- **卡片跨卡对齐**：FeatureGrid / OutcomeGrid / StepFlow（水平）改用 CSS subgrid，同一行的图标、标题、正文、标签
  按行轨对齐，可选字段不再把后续区块顶位移；首行元素去掉多余的顶部间距。

- **修复生产构建产出无法启动**：`src-tauri/Cargo.toml` 补回标准 `[features]` 段
  （`custom-protocol = ["tauri/custom-protocol"]`），否则 `cargo build --release`
  编出的二进制处于 dev 模式、不内嵌前端资源，运行时去连 `http://localhost:5173`，
  双击后只有 `ERR_CONNECTION_REFUSED`。

### Changed

- **幻灯片字号整体放大并逐页自适应**：全部幻灯片字号改由 `.slide-shell` 上的 `--type-scale` 统一驱动（默认 `1.25`），
  正文由 18px 提升至 22.5px（占画布高度 2.5% → 3.1%），标题 32px → 40px、封面主标题 52px → 65px；
  模板内 17 处硬编码字号（最小 0.68rem = 10.9px）与 `.section-num` 水印一并收编进该体系，
  外框固定留白同步收紧（垂直方向约 146px → 100px，占画布高 20% → 14%）。
  `SlideRenderer` 按每页内容量给出系数：内容偏少的页面（封面、结尾页）自动放大到 `1.425` 让版面更饱满，
  内容密集的页面保持基准，并由 FeatureGrid / Spotlight 在模板内按字段数与文本量自动降档以防溢出。

- **构建方式统一为 Tauri CLI**：`@tauri-apps/cli` 已作为开发依赖，请使用 `bunx tauri build`
  （不要再直接 `cargo build --release` / `cargo tauri build`）。

### Removed

- **信息图幻灯片（`Infographic` / `@antv/infographic`）**：移除 `@antv/infographic` 依赖与 `Infographic` 模板（模板数 18 → 17），`SplitLayers` 恢复为内置的分层卡片渲染（去掉右侧 @antv 语法可视化）。前端产物 JS 由 1820.92 kB 降至 978.61 kB。
- 旧牌组兼容：`SlideBlueprint.kind` 反序列化改为容错（未知 kind 回退为 `Cover`），已存在于数据库或 `.keynn` 中的 `infographic` 页面不会导致整个项目加载失败。

- **旧批量生成流水线死代码**：并发生成架构（25db08c）上线后遗留的整段旧流水线已删除，包括
  `run_page_plan` / `run_layout_plan` / `generate_content_slides_with_progress` /
  `repair_until_valid` / `normalize_blueprints` 及 `generator/audit.rs` 布局审计模块、
  `PagePlanResponse` / `LayoutPlanResponse`、布局规划缓存与修复函数；`PageStage` 移除从未构造的
  `Pending` / `Error` 变体（前端保留自有兜底状态）。同时删除无调用方的
  `GenerationConfig::multimodal_client`、`CryptoService::validate_password`、
  `db::update_project_media_root`。`cargo check` 警告 28 → 0。

## [0.1.0] - 2024-07-26

### Added

- **18 种幻灯片模板**：Cover、Closing、Overview、SectionIntro、FeatureGrid、Spotlight、SplitLayers、SectionList、FocusExample、OutcomeGrid、CenterGrid、Timeline、StepFlow、Process、Compare、IssueStack、Swot、Infographic
- **三步生成流程**：素材准备 → AI 生成 → 编辑完善
- **多比例幻灯片支持**：16:9、32:9、48:9，播放时自动拉伸至全屏
- **并发生成**：多页并行处理，实时跟踪每页状态
- **LM Studio 本地推理**：完全离线可用，支持自定义模型
- **加密导出/导入**：AES-256-GCM + Argon2id 加密的 `.keynn` 文件格式
- **首次运行向导**：引导用户配置 LM Studio 连接和模型选择
- **幻灯片编辑器**：网格视图 + 单页 JSON 编辑，支持单页修复
- **蓝图驱动的渲染架构**：PagePlan → SlideBlueprint → Vue 组件层层递进
- **AI 布局规划**：LLM 根据内容信号自动选择最佳模板
- **AI 图标匹配**：Embedding 模型语义匹配图标库
- **规范化与校验**：3 轮修复循环确保输出质量

### Technical

- **前端**：Vue 3 + Vite + TypeScript + Pinia + Vue Router
- **后端**：Rust + Tauri v2 + SQLite
- **LLM 集成**：OpenAI 兼容 API（LM Studio 本地推理）
- **加密**：aes-gcm、argon2、rand、base64
