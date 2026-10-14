# 幻述 · 原生界面（只读查看器）

GPUI Kit 写的原生界面，与 Tauri + Vue 版**共用 `huanshu-core` 与同一份数据目录**，逐页迁移。
第一版只做查看：项目列表 → 项目看板（缩略图 + 大图）→ 演示 → 导出 PDF。
不做生成、不做编辑、不做 `.keynn` 导入导出。

幻灯片由 `crates/slide-typst` 的进程内 Typst 编译器排版，界面只负责把像素画出来；
所有数据经 `huanshu_core::services` 读写，本工程**不依赖 `rusqlite`**。

## 运行

```powershell
# 隔离数据目录（推荐：不会碰到真实项目）
$env:HUANSHU_HOME = 'E:\huanshu\target\sandbox-home'
cargo run -p huanshu-core --example seed_demo -- $env:HUANSHU_HOME --force   # 首次先播种演示项目
cargo run -p huanshu-native --release

# 真实数据目录（不加 HUANSHU_HOME 即是）
cargo run -p huanshu-native --release
```

### 命令行开关

用于脚本化验证，不必手点。`--theme` 取 `huanshu_core::themes::APP_THEMES` 里的 id。

| 开关 | 作用 |
|---|---|
| `--open <项目 id>` | 直接打开某个项目，跳过首页 |
| `--page <n>` | 从 1 开始指定打开的页 |
| `--present` | 打开后直接进演示模式 |
| `--theme <主题 id>` | 启动时用的主题（并写入设置） |
| `--export-pdf <路径>` | 跳过保存对话框，导出完退出码 0/1 |
| `--quit-after <秒>` | 到点自动退出 |
| `--flip-soak <秒>` | 看板上每 100ms 换一页，循环；每 30 秒在 stderr 打工作集内存 |

示例：

```powershell
cargo run -p huanshu-native --release -- --open 2 --page 3 --theme dark-blue --quit-after 20
cargo run -p huanshu-native --release -- --open 2 --flip-soak 300 2>&1 | Select-String '\[mem\]'
cargo run -p huanshu-native --release -- --open 5 --export-pdf target\charts.pdf
```

## 页面

| 页面 | 说明 |
|---|---|
| 首页 | 项目列表：名称、页数、比例、更新时间。点击打开 |
| 看板 | 左侧缩略图（±4 行预取），右侧当前页大图；顶部返回、项目名、`n / N`、主题选择、「演示」「导出 PDF」。↑/↓ 换页 |
| 演示 | 无边框全屏；←/→、PageUp/PageDown、空格、鼠标左键翻页，Home/End，Esc 退出。位图按屏幕尺寸非等比铺满 |

## 两条编码规则

1. **每张不再显示的 `RenderImage` 都要 `cx.drop_image(img, Some(window))`。**
   GPUI 不会自动回收图集里的 tile（见 `docs/spikes/gpui-kit.md` 第 4 节第 2 条）。
   `PreviewCache` 把每张交还的图都从返回值或 `take_released()` 里拿出来，调用方逐张释放。
2. **渲染一律在 `background_executor` 上跑**，UI 线程只做 BGRA 转换后的 `RenderImage` 构造。

## 结构

```
src/
  main.rs        启动、命令行、外壳（无边框窗口 + 自绘标题栏）
  cli.rs         命令行开关
  images.rs      像素 → RenderImage，以及 drop_image 的统一出口
  app.rs         应用状态（core 的 AppState + 主题）
  projects.rs    首页
  board.rs       看板（两个 PreviewCache：缩略图 + 大图）
  present.rs     演示窗口
  export.rs      导出 PDF（rfd + 后台渲染 + 结果对话框）
```

## 构建说明

`apps/huanshu-native` 在 workspace 的 `members` 里，但**不在 `default-members`**：
裸 `cargo build` / `cargo test` 不编译 GPUI，`cargo test --workspace` 会。
