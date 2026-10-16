# 幻述 · 产品发布（huanshu 格式 keynote 样本）

一副 28 页的 keynote，用**幻述自己的工程格式**交付：`SlideBlueprint[]` 牌组 + `.keynn` 项目包 +
Typst 原生排版的 PDF 与逐页位图。内容讲的正是这条流水线本身，所以每一页的版式选择都能对上它要解释的那件事。

- **主题**：报刊·钴蓝（`light-editorial`）— 暖白纸面、黑墨标题、钴蓝强调、方角细线
- **画幅**：16:9
- **版式**：渲染器支持的全部 **18 种**，一种不缺
- **图表**：`line` / `bar` / `donut` / `grouped_bar` / `stacked_area` 五种全用上
- **公式**：行内 `$…$` 与块级 `$$…$$`（目标函数、`f ∈ [0.4, 1.425]`、均值方差等）
- **图片**：单图 `image` 与多图 `images`（PDF 里等分平铺）两种写法都有
- **色调**：`neutral / blue / green / amber / red` 五套 tone，覆盖卡片、标签、面板与 SWOT 四象限
- **校验**：`.keynn` 已通过「导入沙箱 → serde 反序列化 → 真机渲染」全链路，28 页 0 溢出、0 缺图

## 文件

| 文件 | 说明 |
| --- | --- |
| [幻述产品演说.keynn](幻述产品演说.keynn) | **主交付物**：未加密的幻述项目包，首页「导入项目」直接打开 |
| [幻述产品演说.pdf](幻述产品演说.pdf) | 28 页矢量 PDF（报刊·钴蓝、16:9），Typst 原生排版 |
| [幻述产品演说.md](幻述产品演说.md) | 源稿：四章正文、五张 ```chart 围栏、行内与块级公式（`.keynn` 里的 `md_content` 就是它） |
| [blueprints.json](blueprints.json) | 牌组本体（28 条 `SlideBlueprint`，可直接写进 `projects.blueprints`） |
| [media.json](media.json) | 素材库元数据（`.keynn` 的 `content/media.json`，含宽高、caption、存储路径） |
| [assets/](assets) | 四张自绘插图 + 一张数字海报（封面视觉、工作台、流水线、版式清单、牌组数字） |
| [assets.json](assets.json) | 引用 → 文件 的映射清单，渲染与 PDF 导出共用 |
| [preview/](preview) | 逐页位图、[全场一览.png](preview/全场一览.png)、[主题对照.png](preview/主题对照.png) |
| [tools/](tools) | 生成脚本（见下），全部可重跑 |

### `.keynn` 内部结构

```
manifest.json                      { version, project_name, exported_at }
content/project.json               { name, md_content, blueprints_json, aspect_ratio, … }
content/blueprints.json            28 条 SlideBlueprint
content/media.json                 5 条素材元数据（id 1–5）
content/media/images/*.png         5 张素材原图
```

牌组里的图片引用写作 `media:1 … media:5`，导入时由 `services/storage.rs` 重映射成新的素材 id，
所以包可以在任意机器上反复导入。**主题不在包里**（它是应用设置）：导入后在设置里选「报刊·钴蓝」，
就能得到与 PDF 完全一致的画面；选别的主题，同一份蓝图会立刻换一套样式（见「主题对照」）。

## 版式覆盖

| # | 版式 | 这一页在讲什么 | 顺带覆盖的字段 |
| --- | --- | --- | --- |
| 01 | `cover` | 封面 | `badges` |
| 02 | `overview` | 本场导览 | `section`、`note`、4 条 `overview_items` |
| 03 | `section_intro` | 第一章 从文档到演示 | `badge`、4 张导览卡 |
| 04 | `feature_grid` | 四项能力 | `subtitle`、`items`、`conclusion`、`footer_tag/tone`、四套 tone |
| 05 | `issue_stack` | 手工做幻灯的四次断点 | 4 张横排问题卡 + `items` |
| 06 | `compare` | 手工排版 vs 流水线 | `pros-cons`、`highlight`、双侧 `conclusion` |
| 07 | `center_grid` | 让每一页自己找版式 | `badge`、`accent`、4 个 `center_items`、`footer` |
| 08 | `section_intro` | 第二章 流水线解剖 | 6 张卡片（3×2 栅格） |
| 09 | `process` | 三段式流水线 | 3 个 `phases`、内嵌 `steps`、`highlight` |
| 10 | `step_flow` | 从源稿到出图四步 | `direction: horizontal`、四套 tone |
| 11 | `split_layers` | 四层结构 | `left_items` + 4 条 `layers`、`footer` |
| 12 | `focus_example` | 版式分配是全局优化 | **块级公式**、`points`、`ranking`（末条 `muted`） |
| 13 | `section_list` | 交付前四道检查 | 4 条 `list_items`（`step` + `icon`） |
| 14 | `spotlight` | 三步工作流 | `image`、`label/label_tone`、`side_width`、`flow` 面板 |
| 15 | `spotlight` | 18 种版式 | 版式海报单图 + 2 个信息面板 |
| 16 | `spotlight` | 八个环节 | 流水线示意单图 |
| 17 | `spotlight` | 一份源稿四种规模 | **多图 `images`**（PDF 等分平铺） |
| 18 | `section_intro` | 第三章 实测数据 | 4 张卡片 |
| 19 | `chart_focus` | 端到端耗时 | **线图** + `emphasis.highlight/trend`、`source_note` |
| 20 | `chart_focus` | 单页首帧 | **柱状图** + 高亮 |
| 21 | `chart_focus` | 渲染耗时构成 | **环形图** + 高亮 |
| 22 | `chart_focus` | 三种画幅探测次数 | **分组柱状图**（3 个 `series`）+ 图例 |
| 23 | `chart_focus` | 缓存撑起的增量编译 | **堆叠面积图**（2 个 `series`） |
| 24 | `timeline` | 从原型到流水线 | 5 个 `timeline_events`，上下交错 |
| 25 | `section_intro` | 第四章 取舍与下一步 | 4 张卡片 |
| 26 | `swot` | 把流水线放进真实工作流 | 四象限（四套 tone）+ `strategy` |
| 27 | `outcome_grid` | 四件事已经落地 | 4 张成果卡 + `conclusion` |
| 28 | `closing` | 开始写你的第一页 | `badges`、`note` |

## 怎么打开

1. **导入项目**：幻述首页 → 导入 → 选 `幻述产品演说.keynn`（未加密，不需要密码）。
2. **选主题**：设置 → 外观与字体 → 主题选「报刊·钴蓝」。
3. 第一步能看到源稿（含五个 ```chart 围栏与公式），第二步是 28 页缩略图，第三步可改字段、换版式、重试单页。

想直接看结果，打开 [幻述产品演说.pdf](幻述产品演说.pdf)，或翻 [preview/全场一览.png](preview/全场一览.png)。

## 怎么重新生成

```bash
# 1) 画插图（Pillow，颜色取自 light-editorial 主题 token）
python3 keynote/tools/make_images.py keynote/assets

# 2) 出牌组 + 素材元数据 + .keynn 包（会读取同目录的 .md 作为 md_content）
python3 keynote/tools/build_deck.py

# 3) Typst 渲染：PDF + 逐页 PNG
cargo run --release -p slide-typst --example render_fixture -- \
  keynote/blueprints.json target/keynote-render --png \
  --theme light-editorial --assets keynote/assets.json

# 4) 全场一览（并刷新 keynote/preview/）
python3 keynote/tools/contact_sheet.py target/keynote-render keynote/preview

# 5) 主题对照：同一份蓝图换五套主题再渲染一遍
python3 keynote/tools/theme_sheet.py keynote/blueprints.json keynote/preview
```

## 校验记录

`.keynn` 不是「看起来像包」——它走的是应用自己的导入代码，再用应用自己的渲染器排版：

```bash
# 把校验程序临时放回 crate 的 examples 目录（跑完删掉即可）
cp keynote/tools/keynn_check.rs crates/huanshu-core/examples/
cargo run --release -p huanshu-core --example keynn_check -- \
  "keynote/幻述产品演说.keynn" keynote/assets.json
rm crates/huanshu-core/examples/keynn_check.rs
```

实测输出（隔离沙箱 `target/keynn-check/sandbox`，`HUANSHU_HOME` 指向它，绝不碰真实数据）：

```
imported project #1: 幻述产品演说
  md_content: 4970 字
  blueprints: 28 页（serde 校验通过）
  kinds: 18 种 → cover, overview, section_intro, feature_grid, issue_stack, compare,
         center_grid, process, step_flow, split_layers, focus_example, section_list,
         spotlight, chart_focus, timeline, swot, outcome_grid, closing
  rendered: 28 pages, 811 KB, overflows=0, missing_assets=0
OK
```

- **0 溢出**：没有任何一页被裁切（`Rendered.overflows` 为空）。
- **0 缺图**：5 张素材全部解析成功，坏图占位逻辑没被触发。
- **18/18 版式**：`SUPPORTED_KINDS` 里的每一种都在牌组里出现了。
- **6/6 主题**：同一份蓝图在 `light-notes`、`light-terminal`、`dark-terminal`、`dark-yellow`、`dark-blue`
  以及演示所用的 `light-editorial` 下都能排版出 4 页样本，见 [preview/主题对照.png](preview/主题对照.png)。

字体说明：PDF 用运行机器上的系统字体排版（本机走 Hiragino Sans GB 后备），字体子集已内嵌进 PDF，
在任何设备上打开都不会变样；换机器重新渲染时，字形可能随系统字体略有差异。
