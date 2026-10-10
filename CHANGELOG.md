# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **失败页手动重试**：生成详情新增“重试这一页”，弹框选择可用布局后使用原文重新生成、校验并保存单页；其他页仍在生成时先排队，保留人工所选布局，支持再次失败后继续重试。保留兜底页和错误状态，可手动前往编辑；存在失败页时不再自动跳转。

- **便签·稿纸主题**：新增「便签·稿纸」，应用界面取样为暖棕控制件 + 浅木色底 + 米色稿纸卡片（木纹纤维来自新增的 `src/themes/notes-wood.svg`），幻灯片统一渲染为带竖向装订线的单横线稿纸；标题、标签、步骤点等复用既有配色变量，预览、缩略图与播放共用样式。

### Changed

- **设置页重构**：改为左侧分区导航（外观与字体 / 模型服务 / 生成设置 / 存储与缓存 / 关于与帮助），所有改动先落在草稿上，
  与基线逐项对比后才会保存，未保存时给出提示；三个模型服务合并为页签切换，可逐个「测试连接」并显示耗时与状态徽标，
  模型名支持从服务拉取或手动输入；主题改为色块选择并按深浅分组；字体面板新增幻灯片 / 界面两套预览与「统一字体」开关。
  样式表拆分为 `src/views/settings.css`。
- **系统字体选择器**：改为受控弹层，打开时才懒加载系统字体；支持关键词过滤、常用字体置顶、方向键与 Home/End 导航、
  焦点陷阱，并可直接「应用到全部角色」。`font-sample` 样式收回到 `.font-preview`（`notes-studio.css`）。

### Fixed

- **流程页步骤说明被截断**：`WorkflowView` 的侧栏描述类名 `.step-desc` 与 `Process` 幻灯片的步骤说明同名，全局样式让幻灯片文案被限宽省略号截断；改为 `.steps-nav .step-desc` 只约束侧栏，幻灯片内步骤说明恢复完整换行。新增 `scripts/check-notes-theme.mjs` 与 `check-slide-layout.mjs` 的 Process 换行断言守护该回归。

### Removed

- **四套亮色主题**：删除「亮·蓝图」「亮·翠绿」「亮·晨曦」「亮·多巴胺」及其 slide/studio 变量块；
  `localStorage` 中残留的旧主题 id 启动时自动回退到「暗·琥珀」。剩余主题：暗·琥珀、暗·星图、暗·极光、终端·墨、终端·纸。

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
