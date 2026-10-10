# Auto-Slidev 模板扩展说明

## 新增一个 Slide 模板的标准流程
当你要给 Auto-Slidev 增加一个新模板时，按下面顺序改（渲染由前端 `SlideRenderer.vue` 完成，Rust 侧只产出 blueprint）：

1. `src/components/`
   新建 `KeynoteXxxSlide.vue`，Props 设计需清楚。
2. `src-tauri/src/types.rs`
   新增 `SlideKind` 枚举项，以及该模板对应的数据结构。
3. `src-tauri/src/generator/planning.rs`
   在 layout plan 提示词里写清楚：
   - 什么时候应该选这个模板
   - 什么时候不能选这个模板
   - 它和相邻模板的边界是什么
4. `src-tauri/src/generator/utils.rs`
   在 `blueprint_schema_hint()` 里加入严格 JSON 示例。
5. `src-tauri/src/generator/slides.rs`
   在内容生成提示词里加入该模板的字段要求。
6. `src-tauri/src/generator/normalize.rs`
   增加默认值修复、长度收敛、tone/icon 修复。
7. `src-tauri/src/validate.rs`
   增加结构校验，避免生成结果进入非法状态。
8. 文档
   更新 `CLAUDE.md`，把组件用途、Props 和新增流程记录下来。


## 做新模板时最容易漏掉的点

- 只写了 Vue 组件，没有把它接到 `SlideKind`
- 只改了 layout 选择，没有补内容生成提示词
- schema hint 不完整，导致 LLM 输出字段漂移
- 没有 normalize/validate，偶发坏 JSON 会直接进渲染阶段
- 没有写"不要使用此模板"的规则，导致模板被误选


## 移除一个 Slide 模板（反流程）

2026-09 实例：移除 `Infographic` 模板与 `@antv/infographic` 依赖，模板数 18 → 17。
**不要重新加回 @antv/infographic**（用户明确弃用；`SplitLayers` 右侧也一并改回内置分层卡片渲染）。

反流程比加模板多两个易漏点：**LLM 提示词白名单**和**旧数据兼容**。

前端：
1. `src/components/KeynoteXxxSlide.vue` 删文件（若有专用渲染器如 `InfographicCanvas.vue` 一并删）
2. `src/components/SlideRenderer.vue`：`import`、`componentMap`、`kindPresetMap`（氛围图预设）、props 映射 `case 'xxx'`
3. `src/components/types.ts`：`SlideKind` 联合类型、`SlideBlueprint` 中该模板专属字段
4. `src/composables/useSlideEditor.ts`：`KIND_META`、`KIND_FIELDS`、`KIND_DEFAULTS`
5. `src/views/steps/Step3Editor.vue`：`FIELD_DEFS` 中该模板专属字段

Rust（变体与 `SlideBlueprint` 字段必须同时删干净，否则各处 struct 字面量编译不过）：
6. `src-tauri/src/types.rs`：`SlideKind` 变体 + `SlideBlueprint` 字段
7. `src-tauri/src/generator/planning.rs`：`layout_candidates_for_page` 的 system 提示词白名单与规则文案、`safe_fallback_kind` / `variety_fallback_kind` / `pick_different_kind` 候选列表、各 `SlideBlueprint { .. }` 字面量
8. `src-tauri/src/generator/slides.rs`：内容生成提示词的字段要求
9. `src-tauri/src/generator/utils.rs`：`slide_kind_name`（`SlideKind → snake_case`）映射、`blueprint_schema_hint`
10. `src-tauri/src/generator/normalize.rs`：默认值修复分支 + 字面量
11. `src-tauri/src/validate.rs`：结构校验
12. 文档：README 模板数、CHANGELOG（新增 `[Unreleased]`，不要改已发布版本号条目）

必查两点：
- `grep -rn "<模板名>" src src-tauri/src` 应 0 命中（仅剩迁移说明/测试里的字符串）
- **旧数据兼容**：`SlideKind` 是 `#[serde(rename_all = "snake_case")]` 外部标签枚举，删变体后 DB / `.keynn` 里带旧 kind 的牌组会**整体反序列化失败**，而 `commands/projects.rs` 用的是 `unwrap_or_default()` → 项目静默变空。所以 `SlideBlueprint.kind` 挂了
  `#[serde(deserialize_with = "de_slide_kind")]`（未知 kind → `Cover`，与前端 `SlideRenderer.vue` 的 `?? KeynoteCoverSlide` 兜底一致），并有回归测试 `legacy_infographic_kind_degrades_without_losing_slides`。


## 布局选择：候选 + 全局分配

生成流水线（`commands/generate.rs` 的 `run_pipeline`）分三段，布局**不再逐页即时决定**：

1. **2a 并发规划**（`slides.rs::plan_single_page`）：每页 enrich + `planning::layout_candidates_for_page`，
   LLM 只按「本页适配度」给 2-3 个候选打分，**提示词里不要再加多样性信息**。
   候选经 `sanitize_candidates` 过滤：Cover/Overview/Closing 永不入选，`section_intro` 只给
   `section_summary` 页（这类页不调 LLM，直接强制）；LLM 失败时用 `fallback_candidates`（两个不同 kind）。
2. **2b 全局分配**（`planning::assign_layouts`，纯 CPU、确定性）：最大化
   `Σ score − 3·相邻同款 − 1·隔页同款 − 2·超出上限的次数`，单一 kind 上限为内容页数的 30%（至少 2）。
   先 Viterbi 精确求解相邻项，再做单页交换的局部搜索处理全局项。结果只取决于候选，与并发数无关；
   分配明细写入调试目录 `02-layout-assignment.json`。权重常量在 `planning.rs` 的 `ADJACENT_SAME_PENALTY` 等。
3. **2c 并发生成**（`slides.rs::generate_single_page_pipeline`）：content → normalize → validate/repair。
   repair 最后一轮仍可能用 `pick_different_kind` 换模板，此时不会回头重排其他页（少见，可接受）。

新增模板时：提示词白名单在 `layout_candidates_for_page`；若是像 `section_intro` 那样有结构性
使用条件的模板，同步改 `is_assignable_kind`。

## 幻灯片字号体系（`--type-scale`）

所有幻灯片字号都来自一套变量，**不要**在模板里写裸 `font-size: 0.Nrem`（历史上散落过 17 处，最小 0.68rem）。

- 变量声明在 `.slide-shell`（`src/style.css`），**不是 `:root`**。原因：自定义属性的 `var()`
  替换发生在「声明它的那个元素」上，只有放在 `.slide-shell`，`SlideRenderer` 才能对该元素
  inline 覆盖 `--type-scale` 让整套字号重新求值，实现逐页自适应。想整体调力度就改这里的默认值。
- 逐页系数在 `SlideRenderer.vue` 的 `slideTypeScale`（由 `contentLoad` 估算内容量）：
  内容极少的页（封面/结尾）放大到 `1.425`，中等放大到 `1.325`，密集页保持 `1.25`。
- 模板内要在**局部**收敛字号时，同样不能只覆盖 `--type-scale`——必须像
  `KeynoteFeatureGridSlide.vue` 的 `contentVars` / `KeynoteSpotlightSlide.vue` 的 `sideVars`
  那样，在该层把整组 `--type-*` 用 `calc(<基准> * var(--type-scale) * <系数>)` 重新声明。
- 已知边界：`spotlight` 侧栏在「3 个面板 + 每面板多项」时放不下（实测 3 个面板各自最小高度
  约 210/276/158px，合计已接近侧栏可用的 ~551px），原始字号下本就溢出。`normalize` 已把
  面板条目收口到 4 项，但极端内容仍可能溢出，彻底解决需要限制面板数量或调整版式。

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

## 打包与发布（Tauri）

### 一键打包（推荐）

```bash
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
```

脚本做的事：`bun install` → `bun run build`（前端 → `src-tauri/dist-studio`）→ `bunx tauri build --bundles nsis` → 把两个产物汇总复制到 `dist-package/`（命名与 CI release 一致）：

| 产物 | 路径 | 用途 |
|---|---|---|
| `Huanshu_<version>_x64-setup.exe` | `dist-package/` | NSIS 安装包，currentUser 安装，静默 `/S`，卸载 `/S` |
| `auto-slidev-studio_windows_x64.exe` | `dist-package/` | 免安装单文件，双击即跑 |

参数：`-SkipFrontend`（只改了 Rust，复用已有 dist-studio）、`-NoInstall`（只出 exe）、`-DebugProfile`（debug 编译，编译快、冒烟用）。

### 手动等价命令

```bash
bun run build                                  # 或 bunx vite build（跳过类型检查）
bunx tauri build --bundles nsis                # 自动先跑 beforeBuildCommand
# 产物：target/release/bundle/nsis/Huanshu_<version>_x64-setup.exe
#       target/release/auto-slidev-studio.exe（免安装）
```

### 必须记住的坑

1. **没有 `cargo-tauri`**：`cargo tauri build` 会直接报 `no such command: tauri`。Tauri CLI 走 devDependency，统一用 `bunx tauri`。
2. **构建目录是 `<repo>/target`**：根 `Cargo.toml` 是 workspace（members = src-tauri），不是 `src-tauri/target`。README/SETUP 早期写的 `src-tauri/target/.../bundle/msi/` 是错的，且本项目没有 msi target。
3. **前端产物目录固定为 `src-tauri/dist-studio`**：由 `vite.config.ts` 的 `build.outDir` 决定，与 `tauri.conf.json` 的 `build.frontendDist` 对应。改了 vite outDir 就必须同步改 tauri.conf.json。
4. **Windows 只能出 nsis**：`bundle.targets` 里的 `appimage`/`deb` 需要 Linux 宿主，本机构建要显式 `--bundles nsis`。Linux/发布产物交给 `.github/workflows/release.yml`（tag `v*` 触发，tauri-action）。
5. **`beforeBuildCommand` 是 `bun run build`**：不是 `bun build`（`bun build` 是 Bun 自带 bundler，不会跑 vue-tsc + vite）。前端类型错误会直接让 tauri build 失败。
6. **PowerShell 脚本必须 ASCII-only**：Windows PowerShell 5.1 把无 BOM 的 .ps1 按 ANSI(GBK) 读，中文注释会变成乱码并引发 parse error。中文只写进 `.md` 文档。
7. **打包前必须关掉正在运行的 幻述**：Windows 上运行中的 exe 是活映像，cargo 覆盖时直接失败：
   `error: failed to remove file ... target\release\auto-slidev-studio.exe` + `拒绝访问。 (os error 5)`。
   （注意：Windows 允许重命名运行中的 exe，所以“文件能改名”不代表可以重建——别看错现象。）
   `scripts/package.ps1` 已内置前置检查，会直接报出占用进程 pid，不会白等一趟完整编译。
   优雅关闭：`(Get-Process auto-slidev-studio).CloseMainWindow()`（`decorations:false` 的窗口同样响应 WM_CLOSE），不行再 `Stop-Process -Force`。
8. **`src-tauri/Cargo.toml` 的 `[features] custom-protocol` 不能删**：Tauri v2 靠这个 feature 决定内嵌前端资源还是走 `devUrl`。删掉后 `cargo build --release` 照样编得过，但产物是 **dev 模式**（前端资源不内嵌，运行时去连 `http://localhost:5173`），双击只有 `ERR_CONNECTION_REFUSED`。用 `bunx tauri build` 就不会踩到。
   构建后自检：`grep -c "rustc-cfg=dev" target/release/build/auto-slidev-studio-*/output` 应全为 `0`，出现 `1` 说明这次是 dev 构建。

### 打包后必须验证（各步骤实测命令）

```powershell
# 1) 冒烟：直接跑未安装二进制，看窗口是否真的起来
$p = Start-Process D:\huanshu\target\release\auto-slidev-studio.exe -PassThru
Start-Sleep 12; $p.Refresh()
if ($p.HasExited) { "EXITED $($p.ExitCode)" } else { "RUNNING $($p.MainWindowTitle)" }

# 2) 安装包静默安装 + 校验落盘
Start-Process dist-package\Huanshu_0.1.0_x64-setup.exe -ArgumentList '/S' -Wait   # 落到 %LOCALAPPDATA%\Huanshu
Start-Process "$env:LOCALAPPDATA\Huanshu\auto-slidev-studio.exe" -PassThru       # 再跑一次

# 3) 卸载清理（避免留残留影响下次验证）
Start-Process "$env:LOCALAPPDATA\Huanshu\uninstall.exe" -ArgumentList '/S' -Wait
```

预期：`MainWindowTitle` 为 `幻述`。窗口 `decorations: false`，标题来自 `tauri.conf.json` 的 `app.windows[0].title`。

### 运行前置与分发注意

- 运行时依赖本机 **WebView2 运行时**（Win10+ 一般已预装；可用 `reg query "HKLM\SOFTWARE\WOW6432Node\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" /v pv` 查版本）。
- `webviewInstallMode` 当前是 `downloadBootstrapper`：目标机缺 WebView2 时需联网下载。要纯离线装机改成 `embedBootstrapper` 或 `offlineInstaller`（体积显著增大）。
- 安装包未签名，首次安装会有 SmartScreen 提示。
- LM Studio 是外部依赖，不打包；装完需在应用「设置」里填模型名，否则生成流程不可用。
- `build.rs` 依赖 `node_modules/@iconify-json/carbon`，所以打包前必须 `bun install`。


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
