# Auto-Slidev 模板扩展说明

## 新增一个 Slide 模板的标准流程

当你要给 Auto-Slidev 增加一个新模板时，按下面顺序改：

1. `src/components/`
   新建 `KeynoteXxxSlide.vue`，Props 设计需清楚。
2. `src-tauri/src/types.rs`
   新增 `SlideKind` 枚举项，以及该模板对应的数据结构。
3. `src-tauri/src/generator/render.rs`
   把 blueprint 渲染成 `<KeynoteXxxSlide ... />`。
4. `src-tauri/src/generator/planning.rs`
   在 layout plan 提示词里写清楚：
   - 什么时候应该选这个模板
   - 什么时候不能选这个模板
   - 它和相邻模板的边界是什么
5. `src-tauri/src/generator/utils.rs`
   在 `blueprint_schema_hint()` 里加入严格 JSON 示例。
6. `src-tauri/src/generator/slides.rs`
   在内容生成提示词里加入该模板的字段要求。
7. `src-tauri/src/generator/normalize.rs`
   增加默认值修复、长度收敛、tone/icon 修复。
8. `src-tauri/src/validate.rs`
   增加结构校验，避免生成结果进入非法状态。
9. 文档
   更新 `CLAUDE.md`，把组件用途、Props 和新增流程记录下来。


## 做新模板时最容易漏掉的点

- 只写了 Vue 组件，没有把它接到 `SlideKind`
- 只改了 layout 选择，没有补内容生成提示词
- schema hint 不完整，导致 LLM 输出字段漂移
- 没有 normalize/validate，偶发坏 JSON 会直接进渲染阶段
- 没有写"不要使用此模板"的规则，导致模板被误选


## 多比例幻灯片支持 (16:9 / 32:9 / 48:9)

### 功能说明
- 支持 16:9（标准）、32:9（超宽）、48:9（全景）三种幻灯片比例
- 生成时可选择比例，32:9 和 48:9 会自动调整布局（更多列、更宽的间距）
- **播放时拉伸到全屏**：无论什么比例的幻灯片，播放时都会拉伸填满屏幕
  - 32:9 幻灯片在 16:9 屏幕上播放时会横向压缩（看起来被压扁），这正是投到 32:9 大屏幕上的真实效果

### 相关文件

#### 后端 (Rust)
| 文件 | 改动 |
|------|------|
| `src-tauri/src/types.rs` | 新增 `AspectRatio` 枚举和辅助方法，`SlideBlueprint` 增加 `aspect_ratio` 字段 |
| `src-tauri/src/config.rs` | `GenerationConfig` 增加 `aspect_ratio` 字段 |
| `src-tauri/src/db.rs` | `Project` 增加 `aspect_ratio` 字段，projects 表增加 `aspect_ratio TEXT` 列 |
| `src-tauri/src/commands/projects.rs` | 新增 `update_project_aspect_ratio` 和 `get_project_aspect_ratio` 命令 |
| `src-tauri/src/commands/generate.rs` | `generate_slides` 接受 `aspect_ratio` 参数，传递给 Generator |
| `src-tauri/src/generator/planning.rs` | 布局规划提示词增加比例相关的布局规则（32:9 用 6-8 列，48:9 用 8-12 列等） |
| `src-tauri/src/generator/normalize.rs` | `apply_component_defaults` 根据比例调整 `cards.truncate` 等数量限制 |

#### 前端 (TypeScript/Vue)
| 文件 | 改动 |
|------|------|
| `src/components/types.ts` | 新增 `AspectRatio` 类型和 `ASPECT_DIMENSIONS` 常量表 |
| `src/stores/generation.ts` | `generate` 函数接受 `aspectRatio` 参数 |
| `src/views/steps/Step1Prepare.vue` | 新增比例选择器 UI（16:9 / 32:9 / 48:9 Segmented Control） |
| `src/views/PresentationOverlay.vue` | 实现拉伸到全屏播放（`scaleX` 和 `scaleY` 独立填满屏幕） |
| `src/views/steps/Step2Generate.vue` | 预览画布使用动态尺寸和缩放比例 |
| `src/composables/useSlideEditor.ts` | 编辑器预览根据实际比例计算尺寸 |
| `src/views/steps/Step3Editor.vue` | 接收 `currentSlideDims` prop，编辑器预览画布使用动态尺寸 |
| `src/style.css` | `.pres-slide, .slidev-slide` 的 width/height 改为 CSS 变量 |

### 实现要点
1. **生成时**：planning.rs 的提示词会根据比例告诉 LLM 调整布局（如 FeatureGrid 用更多列）
2. **normalize 时**：`apply_component_defaults` 根据比例调整各项数量限制（如 `cards.truncate(4 * cols_multiplier)`）
3. **播放时**：PresentationOverlay 计算 `stretchScale = { scaleX: vw/slideW, scaleY: vh/slideH }`，对幻灯片进行非等比缩放到全屏


## 加密导出/导入功能 (.keynn)

### 功能说明
- 导出时可选择加密，勾选后需设置密码
- 导入时自动检测文件是否加密，如加密则提示输入密码
- 使用 AES-256-GCM + Argon2id 加密，密码强度需至少8字符且包含大小写字母和数字

### 加密文件格式
加密后的 .keynn 文件结构：
```
header.json (非加密，可读)
├── magic: "KNN2"
├── version: "2"
├── algorithm: "AES-256-GCM"
├── kdf: "Argon2id"
├── salt: "<base64 16字节>"
├── nonce: "<base64 12字节>"
└── test_ciphertext: "<加密的验证数据>"
payload.bin (加密的原始 ZIP 内容)
```

### 相关文件

#### 后端 (Rust)
| 文件 | 改动 |
|------|------|
| `src-tauri/Cargo.toml` | 新增 `aes-gcm`, `argon2`, `rand`, `base64` 依赖 |
| `src-tauri/src/crypto.rs` | 新增加密解密模块 `CryptoService` |
| `src-tauri/src/commands/storage.rs` | `export_project` 增加 `encrypted` 和 `password` 参数，`import_project` 增加 `password` 参数 |
| `src-tauri/src/lib.rs` | 注册 `crypto` 模块和 `is_encrypted_file` 命令 |

#### 前端 (TypeScript/Vue)
| 文件 | 改动 |
|------|------|
| `src/components/PasswordDialog.vue` | 新增密码输入对话框组件（支持加密/解密模式、密码强度指示器） |
| `src/views/HomeView.vue` | 导出按钮增加加密选项弹出层，导入时自动检测加密状态 |
| `src/stores/projects.ts` | 无需改动（密码参数直接透传给 Tauri 命令） |

### 实现要点
1. **加密时**：先创建标准 .keynn ZIP，然后用 Argon2id 从密码派生 32 字节密钥，再用 AES-256-GCM 加密
2. **验证机制**：header 中包含用同一密钥加密的测试字符串，解密时先验证测试字符串是否正确
3. **自动检测**：导入时调用 `is_encrypted_file` 判断是否需要密码
4. **密码强度**：必须包含大小写字母和数字，最少 8 字符

<!-- code-review-graph MCP tools -->
## MCP Tools: code-review-graph

**IMPORTANT: This project has a knowledge graph. ALWAYS use the
code-review-graph MCP tools BEFORE using Grep/Glob/Read to explore
the codebase.** The graph is faster, cheaper (fewer tokens), and gives
you structural context (callers, dependents, test coverage) that file
scanning cannot.

### When to use graph tools FIRST

- **Exploring code**: `semantic_search_nodes` or `query_graph` instead of Grep
- **Understanding impact**: `get_impact_radius` instead of manually tracing imports
- **Code review**: `detect_changes` + `get_review_context` instead of reading entire files
- **Finding relationships**: `query_graph` with callers_of/callees_of/imports_of/tests_for
- **Architecture questions**: `get_architecture_overview` + `list_communities`

Fall back to Grep/Glob/Read **only** when the graph doesn't cover what you need.

### Key Tools

| Tool | Use when |
|------|----------|
| `detect_changes` | Reviewing code changes — gives risk-scored analysis |
| `get_review_context` | Need source snippets for review — token-efficient |
| `get_impact_radius` | Understanding blast radius of a change |
| `get_affected_flows` | Finding which execution paths are impacted |
| `query_graph` | Tracing callers, callees, imports, tests, dependencies |
| `semantic_search_nodes` | Finding functions/classes by name or keyword |
| `get_architecture_overview` | Understanding high-level codebase structure |
| `refactor_tool` | Planning renames, finding dead code |

### Workflow

1. The graph auto-updates on file changes (via hooks).
2. Use `detect_changes` for code review.
3. Use `get_affected_flows` to understand impact.
4. Use `query_graph` pattern="tests_for" to check coverage.
