# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **模型服务 SOCKS5 代理**：大语言模型 / 向量模型 / 多模态模型可各自启用 SOCKS5 代理，
  支持用户名密码认证（支持 `socks5://` / `socks5h://`），模型域名由代理解析；
  代理不可用时返回错误而不回退直连。旧 `settings.json` 无 `proxy` 字段时默认关闭。

### Fixed

- **修复生产构建产出无法启动**：`src-tauri/Cargo.toml` 补回标准 `[features]` 段
  （`custom-protocol = ["tauri/custom-protocol"]`），否则 `cargo build --release`
  编出的二进制处于 dev 模式、不内嵌前端资源，运行时去连 `http://localhost:5173`，
  双击后只有 `ERR_CONNECTION_REFUSED`。

### Changed

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
