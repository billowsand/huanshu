# 任务：抽出 Rust 核心 crate，把前端里的业务逻辑搬进 Rust（第七步）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio，Tauri v2 + Vue 3 + Rust）里工作。
开始前先读 `CLAUDE.md`（尤其「布局选择」「素材库图片与生成链路」「Typst 原生 PDF 导出」「加密导出/导入」），
以及 `docs/spikes/gpui-kit.md`（上一步的 GPUI Kit 验证报告，第 9 节列了本步要搬的逻辑）。

## 背景

GPUI Kit 验证通过（中文输入法已由用户真机验收），界面将从 Tauri + Vue 迁到原生 GPUI。迁移是**逐页**进行的，
期间两套界面要共存一段时间。所以先做这一步：

1. 现在 `src-tauri` crate 里，业务逻辑（数据库、生成流水线、导入导出、PDF/缩略图）和 Tauri 命令混在一起，
   GPUI 界面没法复用。要把**所有与 Tauri 无关的代码**搬进一个新 crate `crates/huanshu-core`，Tauri 命令变成薄薄的一层转发。
2. 前端 TypeScript 里还有一些**不是界面而是规则**的代码（编辑器的字段表、模板默认值、图表数据校验），
   它们和 Rust 侧各维护一份，GPUI 界面也需要。把它们搬进 `huanshu-core`，Vue 界面改为从 Rust 读取。

**本步不改任何对外行为**：Tauri 命令名、参数、返回值、事件名与载荷都不变（C 节新增的两个命令除外），数据库与 `.keynn` 格式不变。

## 前几轮审查发现的问题（不要再犯）

1. **报告与事实不符**：没做的写「未做」，没验证的写「未验证」。上一轮把 spike 自己的内存泄漏报成了 GPUI 的问题——
   下结论前先排除自己代码的问题。
2. **只删不查**：每删一样东西，`grep` 确认 0 命中再提交。
3. **注释、提交信息与代码不一致**：搬代码时注释里的路径（`crate::xxx`、`src-tauri/src/...`）要跟着改。
4. **重复实现**：搬过去的逻辑在原处必须删掉，不能留两份；做不到就在汇报里写明为什么留、什么时候删。

## 范围

**要改**：新建 `crates/huanshu-core/`；`src-tauri/`（模块搬走、命令变薄）；根 `Cargo.toml` 的 workspace members；
`src/composables/useSlideEditor.ts`、`src/views/steps/Step3Editor.vue`、`src/utils/chartSpec.ts`（C 节）；删除 D 节列出的死代码。

**不许改**：Typst 模板与 `crates/slide-typst`（除非搬模块需要改 `use` 路径）、`spikes/gpui-kit/`、生成流水线的行为
（提示词、打分、分配算法、normalize/validate 规则）、数据库表结构、`.keynn` 格式、`CLAUDE.md` / `README` / `CHANGELOG`
（文档由审阅方更新）。

## A. 新建 `crates/huanshu-core`（只搬家，不改行为）

1. 新 crate：`crates/huanshu-core`，`[package] name = "huanshu-core"`，lib，加入根 workspace members。
   **不得依赖 `tauri` 或任何 `tauri-plugin-*`**（验收：`cargo tree -p huanshu-core | grep -i tauri` 无输出）。
2. 用 `git mv` 搬（保留历史）这些模块与它们的单测：`assets`、`config`、`crypto`、`db`、`embedded_icons`
   （连同 `src-tauri/build.rs` 里拷贝 carbon 图标的部分）、`generator/`（整个目录，含 `layout_probe`）、`icon`、`image_size`、
   `input`、`lmstudio`、`network`、`types`、`validate`，以及 `fonts.rs` 里枚举系统字体的函数（`#[tauri::command]` 外壳留在 `src-tauri`）。
3. 依赖随模块迁移：`src-tauri/Cargo.toml` 只留 Tauri 相关与命令层确实用到的依赖。
4. 为了少改调用点，`src-tauri/src/lib.rs` 可以用 `use huanshu_core::{assets, config, ...};` 让 `crate::db::...` 一类路径继续可用；
   但 `huanshu-core` 内部的路径要改成它自己的 `crate::...`。
5. `layout_probe`（`#[ignore]` 的真机探针）搬过去后，`CLAUDE.md` 里的命令会变成
   `cargo test -p huanshu-core --lib layout_probe -- --ignored --nocapture`；确认它能编译（不要求跑）。
6. 这一步单独提交，提交前 `cargo test --workspace` 的测试**数量与结果**和搬家前一致（贴前后对比）。

## B. 服务层：Tauri 命令变薄

把 `src-tauri/src/commands/*.rs` 和 `src-tauri/src/lib.rs` 里的业务逻辑搬进 `huanshu-core`，按领域组织模块
（建议 `projects`、`generation`、`storage`（`.keynn` 导入导出与加密）、`export`（PDF 与缩略图，包装 `slide-typst`）、`settings`、`media`）。

1. **应用状态**：`AppState`、`default_data_dir` / `bootstrap_path` / bootstrap 读写、`sync_storage_root` 等搬进 core。
   Tauri 侧用 `Mutex<huanshu_core::AppState>`（或 core 提供的句柄类型）管理，GPUI 将来用同一个类型。
2. **事件**：core 不能调用 `AppHandle::emit`。在 core 定义
   ```rust
   pub trait EventSink: Send + Sync { fn emit(&self, event: CoreEvent); }
   pub enum CoreEvent { /* 一一对应现有事件 */ }
   ```
   先 `grep` 出 `src-tauri` 现有的全部事件（前端监听的是 `gen:progress`、`gen:slide_ready`、`gen:error`、`gen:log`、`gen:page_status`，
   以 Rust 侧实际 emit 的为准），`CoreEvent` 每个变体对应一个事件名和**完全相同的 JSON 载荷**。Tauri 侧实现
   `EventSink`，把变体转回 `(事件名, 载荷)` 再 `emit`。
   **加单测锁定线上格式**：每种事件构造一个样例，断言 Tauri 适配层产出的事件名与 `serde_json` 序列化结果和改动前一致
   （改动前先用旧代码生成期望值写进测试，不要照新代码抄）。
3. **命令变薄**：每个 `#[tauri::command]` 只做「取参数 → 锁状态 / 取 `EventSink` → 调 core → 把错误转成 `String`」。
   命令体超过约 25 行、或者里面还有 SQL、文件读写、业务分支的，说明没搬干净。`spawn_blocking` / `async_runtime` 这类
   执行方式的选择留在命令层（GPUI 会有自己的线程模型）。
4. **异步运行时**：生成流水线依赖 tokio（reqwest）。core 的 async 函数可以要求「在 tokio 运行时里调用」，在模块文档里写明；
   不要把 `tauri::async_runtime` 带进 core。
5. `commands/thumbnails.rs` 与 `commands/pdf.rs` 的 `prepare_export` / `export_assets` / `font_roles` 搬进 core 的 `export`，
   保持「缩略图与 PDF 导出共用同一套主题、比例、字体、素材解析」这条不变量（`CLAUDE.md` 有说明）。
6. 现有 `src-tauri` 里的单测随逻辑一起搬进 core，测试数量不能减少。

## C. 前端规则搬进 Rust

### C1. 编辑器模式表（editor schema）

现在这些表只在 TypeScript 里：
- `src/composables/useSlideEditor.ts`：`KIND_META`（中文名、图标、说明）、`KIND_FIELDS`（每种 kind 用到的字段）、`KIND_DEFAULTS`（新建或切换 kind 时的默认内容）；
- `src/views/steps/Step3Editor.vue`：`SIMPLE_FIELDS`、`LIST_SECTIONS`、`OBJECT_SECTIONS`、`TONE_OPTIONS`、`DIRECTION_OPTIONS`
  （字段类型、标签、占位符、行数、下拉选项、列表项模板 `createItem`、`iconQueryKeys` 等）。

做法：
1. **先留快照**（改任何代码之前）：写一个一次性脚本（`bun` 运行，放 `target/` 下不提交），import 这两个文件里的表，
   把它们序列化成 JSON（`createItem` 调用一次取返回值），存为 `crates/huanshu-core/tests/fixtures/editor-schema.json`。
   Vue 单文件组件里的常量不方便 import 的话，先把它们原样挪进一个 `.ts` 模块再导出——这一步本身不改任何值。
2. 在 core 新建 `editor_schema` 模块：用 serde 结构体表达上述全部内容（`createItem` → `new_item: serde_json::Value`），
   提供 `pub fn editor_schema() -> EditorSchema`。
3. 单测：
   - 序列化结果与快照 JSON **完全相等**（证明搬家无损；之后有意修改时同步更新快照）；
   - 每个 `SlideKind` 都有 meta / fields / defaults；
   - `KIND_FIELDS` 里的每个字段名都是 `SlideBlueprint` 序列化后真实存在的键；
   - 每个 kind 的 defaults 加上 `kind` 能反序列化成 `SlideBlueprint`。
4. 新命令 `get_editor_schema`，前端启动后取一次（缓存在一个 composable 或 store 里），`useSlideEditor.ts` 与 `Step3Editor.vue`
   改为从它读取，**删掉 TypeScript 里的这些表**。`SlideKind` 联合类型、`SlideBlueprint` 接口这类 TypeScript 类型可以保留。
   schema 还没取到时编辑器显示加载状态，不能报错或渲染空表单。

### C2. 图表数据校验

现在有两份规则：Rust `ChartSpec::problems()`（英文，用于 Markdown 解析报错和 `validate.rs`）与 TypeScript
`chartProblems()`（中文，用于 Step3 的「图表数据（JSON）」框和高级 JSON）。合并成一份：

1. core 里定义 `ChartProblem` 枚举，校验逻辑只写一次；`message_en()` 产出与现在 `ChartSpec::problems()` **逐字相同**的英文
   （现有测试与 LLM 报错依赖它），`message_zh()` 产出与现在 `chartProblems()` **逐字相同**的中文。
2. TypeScript 的 `chartProblems` 接收的是任意 JSON（可能不是对象、`type` 不合法、`data` 不是数组……），而 Rust 的 `ChartSpec`
   是强类型。core 提供 `pub fn chart_problems_json(value: &serde_json::Value) -> Vec<ChartProblem>`，覆盖 TypeScript 版的全部分支
   （先把 `chartProblems` 的每个分支写成 Rust 单测用例，期望值取自现在的 TypeScript 输出）；`ChartSpec::problems()` 改为复用同一套规则。
3. 新命令 `chart_problems(chart: serde_json::Value) -> Vec<String>`（中文）。Step3 改为调用它：
   - 校验变成异步：输入后 150ms 防抖再请求；只有「最新一次输入」对应的结果为空时才写回牌组（旧请求的结果到达时，
     如果输入已经变了，丢弃）；
   - 等待结果期间不写回，也不显示「校验通过」。
4. 删掉 `src/utils/chartSpec.ts` 里的 `chartProblems`（类型定义如仍被使用可以保留）。

## D. 删除死代码

先 `grep` 确认没有调用方再删，并在汇报里贴出 `grep` 结果：
- `src/components/keynoteTheme.ts`（只被已删除的模板使用）；
- `src/composables/useTitleTokens.ts`、Rust 的 `text.rs` 与 `segment_title` 命令（标题分词只给已删除的 `SlideTitle.vue` 用），
  以及因此不再需要的 `jieba-rs` 依赖——记录删除前后 release 可执行文件大小；
- `src-tauri/src/render.rs`（旧 Slidev Markdown 渲染器，整文件 `#[allow(dead_code)]`；若 `DeckSpec` 等类型随之无人使用，一并删除）。

## 分支与提交

从 `spike/gpui-kit` 新建分支 `feat/core-crate`（需要上一步的 `render_pixels`），按顺序提交（每步提交前跑完验证）：

1. `refactor: move app logic into the huanshu-core crate`（A：只搬家）
2. `refactor: thin Tauri commands over core services`（B）
3. `feat: editor schema and chart checks served from Rust`（C）
4. `chore: remove dead title segmentation, Slidev renderer and tone helper`（D）

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo tree -p huanshu-core | grep -i tauri     # 必须无输出
bun run build
```

真机验证（**必须做**，用 `.claude/launch.json` 的 `tauri-dev`，按 `CLAUDE.md`「真机验证」用 CDP 驱动）：
- 首页：项目列表、新建、删除、`.keynn` 加密导出再导入（文件对话框需要人工点，做不到就写「未做」）；
- 设置页：读取与保存；
- Step2：打开已有项目，缩略图与详情正常；
- Step3：每种 kind 至少切换一次（默认内容来自 Rust schema）、列表加一项（`new_item`）、在图表 JSON 框里输入一个坏 JSON
  和一个类型不合法的图表，看到中文错误；改正后能写回；
- 生成：如果本机 LM Studio 在跑，用一份 3–5 节的小文档完整生成一次，确认进度、日志、逐页状态事件都正常；
  没在跑就写「未验证生成流程」。

## 最终汇报（按此格式，不要省略）

1. 四个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）；A 提交前后的测试数量对比
3. 搬进 core 的模块清单、`src-tauri` 剩下的文件与行数（目标：只剩命令转发、Tauri 初始化、`EventSink` 实现）
4. 事件线上格式的锁定测试是怎么生成期望值的
5. C1 快照是怎么生成的；C2 的 TypeScript 分支与 Rust 用例的对应表
6. D 节每项删除前的 `grep` 结果；可执行文件大小前后对比
7. 真机验证逐项结果（没做的写「未做」）
8. 与本说明不一致的实现及原因；没做完或不确定的点
