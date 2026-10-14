# 任务：原生界面第一版——只读查看器（第八步）

你在仓库 `E:\huanshu`（幻述 / Auto-Slidev Studio）里工作。开始前先读：
- `CLAUDE.md` 开头的「代码结构：`huanshu-core` 与界面层」，以及「Typst 缩略图」「编辑器实时预览与 Step2 详情」两节；
- `docs/spikes/gpui-kit.md`（GPUI Kit 验证报告，尤其第 1、4、9 节）与 `spikes/gpui-kit/`（可以照抄的参考实现）；
- `src/composables/useTypstThumbnails.ts`、`src/components/SlideFrame.vue`、`src/views/PresentationOverlay.vue`
  （本步要在 Rust 里复刻它们的行为）。

## 背景

业务逻辑已经在 `crates/huanshu-core`（不依赖 Tauri），GPUI Kit 验证通过（输入法已真机验收）。现在开始真正的原生界面
`apps/huanshu-native`，与 Tauri 版**共用 core 和同一份数据目录**，逐页迁移，两套界面共存一段时间。

第一版只做**只读查看器**：项目列表 → 打开项目看板（缩略图 + 大图）→ 演示 → 导出 PDF。不做生成、不做编辑、
不做导入导出 `.keynn`。目的是把「原生界面 + core 服务 + Typst 位图」这条链路做成正式代码，后续页面都在它上面加。

## 前几轮审查发现的问题（不要再犯）

1. **真机验证改坏了用户的真实数据**（上一轮）：在用户真实项目上把封面换成图表页并保存，还用测试文稿在该项目上
   发起了一次生成，留下一条失败记录。本轮起**所有真机验证一律在隔离数据目录里做**（见 A 节）。动手前确认
   `HUANSHU_HOME` 已设置；任何时候都不许写 `%LOCALAPPDATA%\auto-slidev-studio` 与 `%APPDATA%\auto-slidev-studio`。
2. **悄悄改快照**：有意修改测试期望值（快照、fixture）必须在汇报里单独列出并说明原因。
3. **同步改异步漏了时机**：上一轮把图表校验改成异步时，失焦会取消等待中的校验，粘贴后马上点别处修改就丢了。
   凡是异步结果，都要逐个想清楚：结果回来时用户是否已换页、换项目、关窗口、切主题；迟到的结果一律作废，不能落到别处。
4. **报告与事实不符**：没做的写「未做」，没验证的写「未验证」；性能数字只认 release 构建，注明测法。
5. **重复实现**：本步允许的重复只有 B 节写明的两处，其余不许出现第二份规则。

## 范围

**要改**：`crates/huanshu-core`（A、B 节）；新建 `apps/huanshu-native/`（C 节）；根 `Cargo.toml`；`.claude/launch.json`；
`src-tauri/src/commands/pdf.rs`（只把 `reveal_exported_file` 的实现挪进 core，命令保留）。

**不许改**：Vue 前端 `src/`、Typst 模板与 `crates/slide-typst`、生成流水线、数据库表结构、`.keynn` 格式、
`spikes/gpui-kit/`（保留作参考，不删不改）、`CLAUDE.md` / `README` / `CHANGELOG`（文档由审阅方更新）。

## A. 隔离数据目录与演示数据

1. **`HUANSHU_HOME` 环境变量**（`state.rs`）：设置且非空时，`bootstrap_path()` = `$HUANSHU_HOME/bootstrap.json`，
   `default_data_dir()` = `$HUANSHU_HOME/data`（素材目录默认跟随数据目录，现有逻辑不变）；未设置时**行为与现在逐字相同**。
   为了可测，把路径计算写成接收 `Option<&Path>` 的纯函数，单测不要去改进程环境变量（测试并发会互相干扰）。
   Tauri 版自动获得这个能力，不用改它。
2. **演示数据**：`crates/huanshu-core/examples/seed_demo.rs`，用法 `cargo run -p huanshu-core --example seed_demo -- <目录>`：
   - 只能写进参数给的目录（内部把它当 `HUANSHU_HOME` 用）；目录为空字符串、或解析后等于真实数据目录 / bootstrap 所在目录时**拒绝运行**；
     目录里已有 `data/projects.db` 时拒绝，除非加 `--force`（先删除该目录下的 `data/` 与 `bootstrap.json` 再重建）。
   - 全部经 `huanshu_core::services` 写入（不写 SQL），建这些项目：
     「全部版式 16:9」（`fixtures/all-layout-blueprints.json`）、「全部版式 32:9」「全部版式 48:9」（同一份蓝图，项目比例和每页
     `aspect_ratio` 都改成对应值）、「图表」（`crates/slide-typst/tests/fixtures/charts.json`）、「图表边界」（`charts-edge.json`）、
     「压力」（`stress.json`）、「图片」（`spotlight.json`：按 `assets.json` 把图片导入素材库，引用改写成 `media:<id>`；
     坏图和缺图的用例原样保留，它们本来就该显示占位）。每个项目的 Markdown 内容写一行说明即可。
   - 打印每个项目的 id、名称、页数。
3. `.claude/launch.json` 增加 `tauri-dev-sandbox`：与 `tauri-dev` 相同，另加 `"HUANSHU_HOME": "E:\\huanshu\\target\\sandbox-home"`
   （`target/` 已被 git 忽略）。

## B. core：预览调度与像素渲染

### B1. `huanshu_core::preview`：缩略图调度（纯逻辑）

把 `useTypstThumbnails.ts` 的调度规则搬成 Rust：**纯状态机，不开线程、不调渲染器**，界面层负责执行请求。
图片类型做成泛型 `Img`（GPUI 里是 `Arc<RenderImage>`），这样 core 不依赖 GPUI。大致接口（可以调整，汇报里写清楚）：

```rust
pub struct PreviewCache<Img> { /* … */ }
impl<Img> PreviewCache<Img> {
    pub fn new(options: PreviewOptions) -> Self;          // serial、progressive、max_cached
    pub fn set_deck(&mut self, slides: &[Option<serde_json::Value>], style: PreviewStyle); // 主题、字体、宽度
    pub fn set_wanted(&mut self, indices: &[usize]);
    pub fn reset(&mut self) -> Vec<Img>;                  // generation + 1，清空并交还全部图片
    pub fn refresh(&mut self);                            // 失败后手动重试
    pub fn next_request(&mut self) -> Option<PreviewRequest>;
    pub fn complete(&mut self, request: RequestId, result: Result<Vec<RenderedSlide<Img>>, String>) -> Vec<Img>;
    pub fn image(&self, index: usize) -> Option<&Img>;
    pub fn is_pending(&self, index: usize) -> bool;
    pub fn failed(&self) -> bool;
}
```

必须保留的规则（都在 `CLAUDE.md`「Typst 缩略图」「编辑器实时预览」里，逐条写成单测）：
- 缓存键 = 蓝图 JSON + 子集位置 + 主题 id + 字体 + 宽度档位（64px 取整）；空位（`None`）不发，页序按子集位置。
- 响应**按键落图**，不按请求整批丢弃；带旧 generation 的响应整个作废（换项目、`reset` 之后）。
- `serial`：同一时刻只有一个请求在途，期间的变化合并成一个尾部请求；`progressive`：比画面新的过期图先落。
- `max_cached` 按最近使用淘汰，但不淘汰 `wanted` 里的页。
- 键变了旧图继续显示，直到新图落下（换主题不闪成加载框）；`reset` 才清空。
- 失败**不自动重试**（否则编译不过的牌组会让渲染死循环），`refresh()` 才重试；失败时旧图保留。
- **凡是被替换、被淘汰、被 `reset` 清掉的图片，都从返回值交还给调用方**——GPUI 不会回收图集，界面必须逐张 `drop_image`
  （见 spike 报告第 1 节）。单测要断言「每张进入缓存的图片最后都被交还了一次，不多不少」。

**允许的重复（一）**：`useTypstThumbnails.ts` 继续服务 Tauri 版，两份调度规则暂时并存，直到 Tauri 界面下线。
两边文件头各加一行注释指向对方，「改一边必须改另一边」。

### B2. 像素渲染服务

`services::export::render_slide_pixels(state, project_id, slides, theme, width_px, only)`：与 `render_slide_thumbnails`
走同一个 `prepare_export`（主题、比例、字体、素材解析不能分叉），调 `Renderer::render_pixels`，返回 RGBA 像素、
溢出页与占位页标记。单测：同一页同一宽度，像素与 `render_slide_thumbnails` 的 PNG 解码后逐像素相同。

### B3. 其余小件

- `reveal_exported_file` 的实现挪进 `services::export::reveal_in_file_manager`，Tauri 命令改为调用它。
- 主题清单：`huanshu_core::themes::APP_THEMES`（id、中文名、明暗），从 `src/themes/catalog.ts` 照抄；单测断言 id 集合与
  `slide_typst::SUPPORTED_THEMES` 相同。**允许的重复（二）**：`catalog.ts` 保留（Tauri 版用），两边互相注释。
- 当前主题的读写：`services::settings::get_app_theme` / `set_app_theme`，存在 `app_settings` 表的 `app_theme` 键
  （键值表，不改表结构），未设置时返回 `DEFAULT_THEME`（`dark-yellow`）。Tauri 版的主题仍在 WebView 的 localStorage 里，
  两边暂不同步（汇报里记一笔）。

## C. `apps/huanshu-native`（GPUI Kit）

### C1. 工程

- `[package] name = "huanshu-native"`，bin。依赖：`gpui-kit = "=0.7.3"`（**不要**直接依赖 `gpui`）、`rfd`、`huanshu-core`、`slide-typst`
  （只为类型；渲染一律走 core 的服务），**不依赖 `rusqlite`**——数据只经 `huanshu_core::services` 读写。
- 加入根 workspace 的 `members`，但**不放进 `default-members`**（根 `Cargo.toml` 新增 `default-members`，列出其余三个成员），
  这样 `bunx tauri dev`、裸 `cargo build` 不会编译 GPUI，`cargo test --workspace` 会。
- 如果加入根 workspace 遇到依赖冲突（例如 `links` 冲突）编不过，退回 spike 的做法（独立 `[workspace]`），并在汇报里贴出错误。
- spike 报告 A1 的 MSVC `D8050` 问题与构建目录有关，根 `target/` 下预计不会出现；若出现，用
  `[profile.dev.package.<crate>] debug = false` 只关掉出问题的 C 构建依赖，不要关整个 dev profile 的调试信息。
- `apps/huanshu-native/README.md`：怎么运行、命令行开关、页面说明。

### C2. 外壳

- 启动时 `AppState::load()`，状态用 `std::sync::Mutex<AppState>`（与 Tauri 同一类型），`HUANSHU_HOME` 自动生效；
  窗口标题栏显示数据目录（隔离目录时一眼能看出来）。
- 无边框窗口 + 自绘标题栏（照 spike：`WindowControlArea::Drag`，最小化 / 最大化 / 关闭）。
- 外壳明暗跟随当前主题的 `tone`（`APP_THEMES`），主题从 `get_app_theme` 读、切换时 `set_app_theme` 写。

### C3. 页面

1. **首页**：项目列表（`services::projects::list_projects`，顺序沿用它的 `updated_at DESC`），每行显示名称、页数、比例、更新时间。
   点击打开项目。本轮没有新建 / 删除 / 导入按钮——不要放占位按钮。
2. **看板**：左侧缩略图虚拟列表，右侧当前页大图；顶部有返回、项目名、页码 `n / N`、主题选择（`APP_THEMES` 的中文名）、
   「演示」「导出 PDF」。
   - 每页显示序号与版式中文名（来自 `editor_schema()` 的 `label`，不另写表）。
   - 加载与失败照 `SlideFrame.vue`：没有图时显示加载框（等待超过 300ms 才出现「渲染中…」）；失败时有旧图就继续显示旧图、
     角上给「渲染失败 · 重试」，没有图就显示失败与重试按钮，重试调 `refresh()`；`overflow` 页在大图下提示「超出部分被裁切」。
   - 键盘 ↑ / ↓ 换页，选中项滚动到可见。
   - 缩略图与大图各一个 `PreviewCache`：缩略图 `wanted` = 可见行 ± 4 行；大图 `wanted` = 当前页。
   - 位图宽度 = 显示宽度 × `window.scale_factor()`，按 64px 档位取整；窗口缩放后按新档位重渲。
3. **演示**：看板按钮或 F5 打开；在看板所在的显示器上无边框全屏；←/→、PageUp/PageDown、空格、鼠标左键翻页，Home/End，Esc 退出；
   位图按屏幕尺寸非等比铺满（与 `PresentationOverlay.vue` 拉伸模式一致）。预取规则照抄 Tauri 版：**当前页还没有图时只请求当前页**，
   落图后再预取前 1 页到后 3 页，`max_cached` = 10。
4. **导出 PDF**：`rfd` 保存对话框（默认文件名 = 项目名.pdf）→ 后台线程调 `services::export::export_pdf(project_id, None, path, 当前主题)`
   → 结果对话框列出页数、占位页、溢出页、缺失图片、主题回落，带「在文件夹中显示」（`reveal_in_file_manager`）。导出期间按钮禁用并显示进度。

### C4. 必须遵守的规则

- 渲染一律在后台执行器上跑，UI 线程只做 BGRA 转换后的 `RenderImage` 构造；不许阻塞 UI 线程。渲染器用 core 的 `renderer()`（进程内一个）。
- **每张不再显示的 `RenderImage` 都要 `cx.drop_image(img, Some(window))`**：`PreviewCache` 交还的、换项目、换主题后被替换的、
  关闭演示窗口、返回首页时的。
- 异步结果回来时，项目、窗口、主题都可能已经变了：靠 generation 作废，不靠「当前选中」去猜（见「不要再犯」第 3 条）。
- 只读：本轮任何代码路径都不写项目数据（`set_app_theme` 除外）。

### C5. 命令行开关（用于脚本化验证，不必手点）

`--open <项目 id>`、`--page <n>`（从 1 开始）、`--present`、`--theme <主题 id>`、`--export-pdf <路径>`（跳过保存对话框，导出完退出码 0/1）、
`--quit-after <秒>`、`--flip-soak <秒>`（看板上每 100ms 换一页，循环，期间每 30 秒在 stderr 打印工作集内存）。

## 分支与提交

从 `feat/core-crate` 新建分支 `feat/native-viewer`，按顺序提交（每步提交前跑完验证）：

1. `feat(core): HUANSHU_HOME sandbox and demo seed`（A）
2. `feat(core): preview scheduler and pixel rendering service`（B）
3. `feat(native): read-only GPUI viewer — projects, board, presentation, PDF export`（C）

## 验证

```bash
cargo fmt --all
cargo test --workspace
cargo tree -p huanshu-core | grep -i -E "tauri|gpui"     # 必须无输出
cargo build -p huanshu-native --release
bun run build                                            # 前端没改，必须仍然通过
```

真机验证（**必须做，全部在隔离目录**）：

```powershell
$env:HUANSHU_HOME = 'E:\huanshu\target\sandbox-home'
cargo run -p huanshu-core --example seed_demo -- $env:HUANSHU_HOME
cargo run -p huanshu-native --release
```

- 截图（照 `spikes/gpui-kit/tools/screenshot.ps1`，先 `SetProcessDPIAware`），存到 `docs/native/`：首页；看板的
  16:9 / 32:9 / 48:9 三个项目；「图片」项目（含缺图、坏图占位）；「图表边界」；深、浅主题各一张；演示模式。
- `--export-pdf`：每个演示项目导出一次，贴页数，并与 Tauri 版对同一项目的导出（`tauri-dev-sandbox` + CDP 调 `export_pdf`）比对页数和报告内容。
- 内存：`--flip-soak 300`，贴 0 / 60 / 120 / 180 / 240 / 300 秒的工作集，必须收敛（参考 spike 修复后 280–310 MiB 量级）。
- 冷启动（到首页出现）5 次中位数，release。
- 共存：用 `tauri-dev-sandbox` 启动 Tauri 版，确认它看到同一批演示项目（CDP `list_projects`）；两边同时开着，Tauri 版改一个项目的蓝图（只在隔离目录！），
  原生版回到首页后能看到更新时间变化。
- 人工验收清单（交给用户，你做不了的）：标题栏拖动与贴靠、多显示器下演示开在哪块屏、150% 缩放下位图清晰度、键盘翻页手感。

## 最终汇报（按此格式，不要省略）

1. 三个提交的 hash 与一句话说明
2. 每条验证命令的结果（贴输出末尾几行）；`cargo test --workspace` 的耗时（加入 GPUI 之后）
3. `PreviewCache` 的最终接口，以及与 `useTypstThumbnails.ts` 的规则对照表（规则 → 单测名）
4. 原生工程的文件清单与行数；是否加入了根 workspace（没加入就贴错误）
5. 截图清单；PDF 页数比对；内存与冷启动数字（注明 release 与测法）
6. 真机验证逐项结果（没做的写「未做」）与给用户的人工验收清单
7. 有意修改的测试期望值（快照、fixture）及原因；没有就写「无」
8. 与本说明不一致的实现及原因；没做完或不确定的点
