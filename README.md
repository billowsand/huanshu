# 幻述 - AI 演示文稿生成工作室

将 Markdown 文档自动转化为精美的 Slidev 演示文稿。

## 功能特点

- **三步生成**：素材准备 → AI 生成 → 编辑完善
- **智能布局**：AI 根据内容自动选择最佳幻灯片模板
- **18 种模板**：封面、目录、分隔页、网格、对比、时间线、SWOT、信息图等
- **多比例支持**：16:9 标准、32:9 超宽、48:9 全景
- **并发生成**：多页并行处理，实时跟踪每页状态
- **本地运行**：基于 LM Studio，完全离线可用
- **实时预览**：生成过程实时展示，进度一目了然
- **加密导出**：AES-256-GCM 加密的项目文件导出与导入

## 环境要求

- [LM Studio](https://lmstudio.ai/)（需加载聊天模型和 Embedding 模型）
- Node.js 18+
- Rust 1.70+
- Bun

## 安装运行

```bash
# 安装依赖
bun install

# 运行开发版本
bun dev
```

## LM Studio 配置

1. 下载并启动 [LM Studio](https://lmstudio.ai/)
2. 在 **Local Server** 界面加载一个聊天模型（如 Qwen、Llama）
3. 加载一个 Embedding 模型（用于图标语义匹配）
4. 点击 "Start Server"，默认地址为 `http://localhost:1234`
5. 在幻述设置中填入模型名称

## 使用步骤

### 第一步：准备素材

1. 输入项目名称
2. 粘贴或上传 Markdown 内容
3. 选择生成粒度：
   - **H2**：每个 `##` 标题生成一页
   - **H3**：每个 `###` 标题生成一页
4. 选择幻灯片比例：16:9 / 32:9 / 48:9
5. 可选上传配图
6. 点击 **开始生成**

### 第二步：AI 生成

等待生成完成，实时查看：
- 当前阶段（初始化、页面规划、布局规划、内容生成、修复验证）
- 每页的独立生成状态与进度
- 每个阶段的具体日志
- 点击日志可查看详细 Prompt 和输出

### 第三步：编辑完善

1. 网格视图查看所有幻灯片
2. 点击卡片进入编辑模式
3. 修改 JSON 内容、模板类型
4. 插入或替换媒体引用
5. 保存后进入演示模式

## 幻灯片生成机理

### 整体流程

```
Markdown 文档
     │
     ▼
┌─────────────────┐
│  解析文档        │  提取标题层级结构
│  parse_markdown │  title / intro / sections / subsections
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  页面规划        │  根据标题生成 PagePlan
│  run_page_plan  │  补充 objective / key_points /
│                 │  content_shape / layout_intent 等
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  布局规划        │  LLM 选择最佳 SlideKind
│  run_layout_plan│  验证布局是否适配内容
│                 │  最多 3 轮修复 + 回退机制
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  内容生成        │  每页独立生成 SlideBlueprint
│  generate_      │  - 调用 LLM 生成 JSON
│  content_slides │  - Embedding 选择匹配图标
│                 │  - 并发生成，结果缓存
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  规范化修复      │  长度修复 / 语气调整 /
│  normalize      │  资源验证 / 结构校验
│  + validate     │  3 轮修复循环
└────────┬────────┘
         │
         ▼
┌─────────────────┐
│  装配输出        │  追加封面 / 目录 / 结尾
│  assemble_slides│  最终输出
└─────────────────┘
```

### 核心数据结构

**PagePlan** - 页面规划单元
```json
{
  "page_id": "p1",
  "section_title": "背景介绍",
  "subsection_title": "市场现状",
  "page_title": "市场规模",
  "objective": "让观众了解市场规模",
  "key_points": ["全球市场 5000 亿", "年增长率 15%"],
  "content_shape": "overview|summary|comparison|architecture|timeline|workflow|matrix",
  "layout_intent": "左图右文",
  "visual_need": "text_only|image_optional|image_required",
  "object_count": "single|pair|multi",
  "argument_mode": "parallel|sequential|layered|summary|evidence|causal|warning",
  "density": "low|medium|high",
  "preferred_assets": ["图表", "数据"],
  "page_role": "content|section_summary"
}
```

**SlideBlueprint** - 幻灯片蓝图
```json
{
  "id": "uuid",
  "kind": "FeatureGrid",
  "aspect_ratio": "ratio_16x9",
  "cards": [...],
  "panels": [...],
  "layers": [...],
  "timeline_events": [...],
  "steps": [...],
  "compare_data": {...},
  "swot_data": {...},
  "infographic_data": {...}
}
```

### 18 种幻灯片模板

| 模板 | 用途 | 关键字段 |
|------|------|---------|
| `Cover` | 封面 | badges, title, subtitle |
| `Closing` | 结尾 | badges, title, subtitle, note |
| `Overview` | 目录 | items |
| `SectionIntro` | 章节预览 | cards |
| `FeatureGrid` | 特性网格 | cards (3+ 项) |
| `Spotlight` | 焦点展示 | image, panels, label |
| `SplitLayers` | 分层架构 | left_items, layers |
| `SectionList` | 有序列表 | list_items |
| `FocusExample` | 论点+案例 | points, example_title, example_body |
| `OutcomeGrid` | 成果展示 | cards |
| `CenterGrid` | 中心网格 | center_items, badge |
| `Timeline` | 时间线 | timeline_events |
| `StepFlow` | 步骤流 | steps |
| `Process` | 多阶段流程 | phases |
| `Compare` | 双栏对比 | compare_data |
| `IssueStack` | 问题堆叠 | cards, panels |
| `Swot` | SWOT 分析 | swot_data |
| `Infographic` | 信息图 | infographic_data |

### 布局选择策略

LLM 根据 PagePlan 的以下信号选择布局：

1. **content_shape**：内容形态（概览/总结/对比/架构/时间线/流程/矩阵）
2. **object_count**：对象数量（单/双/多）
3. **argument_mode**：论证模式（并行/顺序/分层/总结/证据/因果/警示）
4. **density**：信息密度（低/中/高）
5. **visual_need**：视觉需求（纯文本/可选图/需配图）

模板选择后，Audit 模块验证布局是否真正适配内容信号，不适配则触发修复。

## 多比例幻灯片

支持三种幻灯片比例，适应不同展示场景：

| 比例 | 分辨率 | 适用场景 |
|------|--------|---------|
| 16:9 | 1280×720 | 标准屏幕、投影仪 |
| 32:9 | 2560×720 | 超宽显示器、双屏拼接 |
| 48:9 | 3840×720 | 全景展示、LED 长屏 |

- 生成时 LLM 会根据比例自动调整布局（如 FeatureGrid 使用更多列）
- 播放时幻灯片拉伸至全屏，确保在对应屏幕上完美呈现

## 加密导出 / 导入

支持将项目导出为加密的 `.keynn` 文件：

- **加密算法**：AES-256-GCM
- **密钥派生**：Argon2id
- **密码要求**：至少 8 字符，包含大小写字母和数字
- 导入时自动检测文件是否加密，加密文件需输入密码解锁

## 项目结构

```
auto-slidev/
├── src/                          # Vue 前端
│   ├── components/               # 幻灯片 Vue 组件
│   │   ├── SlideRenderer.vue     # 模板渲染器
│   │   ├── Keynote*.vue          # 18 种模板组件
│   │   ├── PasswordDialog.vue    # 加密密码对话框
│   │   ├── ExportDialog.vue      # 导出对话框
│   │   ├── PipelineProgress.vue  # 生成进度条
│   │   └── ...                   # 其他 UI 组件
│   ├── composables/              # Vue 组合式函数
│   │   ├── useSlideEditor.ts     # 幻灯片编辑器
│   │   └── useMediaLibrary.ts    # 媒体库管理
│   ├── stores/                   # Pinia 状态管理
│   │   ├── generation.ts         # 生成流程状态
│   │   ├── projects.ts           # 项目管理
│   │   ├── config.ts             # 配置
│   │   └── appSettings.ts        # 应用设置
│   └── views/                    # 页面视图
│       ├── HomeView.vue          # 首页（项目管理）
│       ├── WorkflowView.vue      # 三步工作流
│       ├── PresentationOverlay.vue # 全屏演示
│       ├── SettingsView.vue      # 设置页
│       └── steps/                # 工作流步骤
│           ├── Step1Prepare.vue  # 素材准备
│           ├── Step2Generate.vue # AI 生成
│           └── Step3Editor.vue   # 编辑完善
├── src-tauri/                    # Rust 后端
│   └── src/
│       ├── commands/             # Tauri 命令
│       │   ├── generate.rs       # 生成入口
│       │   ├── projects.rs       # 项目管理
│       │   ├── storage.rs        # 导出/导入
│       │   └── settings.rs       # 设置
│       ├── generator/            # 生成器核心
│       │   ├── planning.rs       # 页面/布局规划
│       │   ├── slides.rs         # 内容生成
│       │   ├── normalize.rs      # 规范化修复
│       │   ├── audit.rs          # 布局审计
│       │   ├── render.rs         # 蓝图渲染
│       │   ├── icons.rs          # 图标匹配
│       │   └── utils.rs          # 工具函数
│       ├── crypto.rs             # AES-256-GCM 加解密
│       ├── db.rs                 # SQLite 数据库
│       ├── input.rs              # Markdown 解析
│       ├── lmstudio.rs           # LM Studio API
│       ├── config.rs             # 生成配置
│       ├── types.rs              # 类型定义
│       └── validate.rs           # 结构校验
└── package.json
```

## 配置说明

### 生成设置

| 参数 | 说明 | 默认值 |
|------|------|--------|
| `concurrency` | 并行生成数量 | 3 |
| `repair_rounds` | 修复轮次 | 3 |
| `granularity` | H2 或 H3 | H2 |
| `aspect_ratio` | 16:9 / 32:9 / 48:9 | 16:9 |

### LM Studio 设置

| 参数 | 说明 | 默认值 |
|------|------|--------|
| base_url | API 地址 | http://localhost:1234 |
| chat_model | 聊天模型名称 | - |
| embedding_model | Embedding 模型 | - |
| api_key | API 密钥（可选） | - |

## License

MIT
