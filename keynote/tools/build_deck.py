#!/usr/bin/env python3
"""Build the showcase keynote: `blueprints.json` + the `.keynn` project package.

主题：报刊·钴蓝（light-editorial）。一副 28 页的牌组，覆盖渲染器里全部 18 种版式、
5 种图表、5 套色调（tone）、行内与块级公式，以及单图/多图两种图片版式。

    python3 keynote/tools/build_deck.py

输出：
    keynote/blueprints.json      牌组（SlideBlueprint[]，可直接写进 projects.blueprints）
    keynote/media.json           素材库元数据（.keynn 的 content/media.json）
    keynote/幻述产品演说.keynn    可被「幻述」导入的项目包（未加密）
"""

import json
import time
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "keynote"
ASSETS = OUT / "assets"
PROJECT_NAME = "幻述产品演说"
MD_PATH = OUT / f"{PROJECT_NAME}.md"

# --------------------------------------------------------------------------- #
# helpers
# --------------------------------------------------------------------------- #


def card(title, tone, icon, **rest):
    return {"title": title, "tone": tone, "icon": icon, **rest}


def item(step, icon, title, body):
    return {"step": step, "icon": icon, "title": title, "body": body}


def point(icon, title, body):
    return {"icon": icon, "title": title, "body": body}


def panel(title, tone, icon, **rest):
    return {"title": title, "tone": tone, "icon": icon, **rest}


CHART_SOURCE = {"source": "tool", "source_note": "幻述基准测试 · 28 页牌组 · 2026-02"}

# --------------------------------------------------------------------------- #
# 1 · 封面 / 总览
# --------------------------------------------------------------------------- #

slides = []

slides.append(
    {
        "kind": "cover",
        "title": "幻述 Huanshu",
        "subtitle": "一页 Markdown，一副 keynote —— 18 种版式、5 种图表与原生公式排版的生产级幻灯流水线",
        "badges": ["18 种版式", "5 种图表", "Typst 原生排版", "6 套主题"],
    }
)

slides.append(
    {
        "kind": "overview",
        "section": "00",
        "title": "本场导览",
        "note": "从一份 2 800 字的 Markdown 源稿出发，看一条可复现的流水线如何决定版式、补齐内容，"
        "并把每一页排到刚好放下。",
        "overview_items": [
            {"number": "01", "title": "问题与愿景", "desc": "手工排版为什么难以规模化，以及我们要守住什么。"},
            {"number": "02", "title": "流水线解剖", "desc": "规划、生成、校验、渲染四段的职责边界。"},
            {"number": "03", "title": "实测数据", "desc": "耗时、探测次数、缓存命中与时间线。"},
            {"number": "04", "title": "取舍与下一步", "desc": "SWOT 分析与已经落地的四件事。"},
        ],
    }
)

# --------------------------------------------------------------------------- #
# 2 · 章节一：问题与愿景
# --------------------------------------------------------------------------- #

slides.append(
    {
        "kind": "section_intro",
        "section": "01",
        "badge": "章节导览",
        "title": "从文档到演示",
        "note": "第一章节要回答的问题：为什么「把内容写对」和「把版式排好」必须分成两件事。",
        "cards": [
            card(
                "内容先行",
                "blue",
                "i-carbon:document",
                body="源稿只描述事实与结构：谁在什么时候、交付了什么、数据是多少。",
            ),
            card(
                "版式后置",
                "green",
                "i-carbon:grid",
                body="版式在整副牌组上分配，而不是逐页贪心，避免相邻撞版。",
            ),
            card(
                "渲染兜底",
                "amber",
                "i-carbon:document-pdf",
                body="任何一页都必须排得下：先实测，再缩放，最后才裁切并上报。",
            ),
            card(
                "可回归",
                "red",
                "i-carbon:compare",
                body="同一份源稿固定后，主题与版式的每次调整都能逐页对比。",
            ),
        ],
    }
)

slides.append(
    {
        "kind": "feature_grid",
        "section": "01",
        "title": "四项能力，构成一条完整的交付链路",
        "subtitle": "从解析到渲染，每一步都有明确的失败边界",
        "note": "四张卡片从左到右依次对应输入、决策、渲染与质量四个环节。",
        "cards": [
            card(
                "结构化解析",
                "blue",
                "i-carbon:data-structured",
                subtitle="输入层",
                body="把标题层级、正文、图表围栏与 $公式$ 拆成可校验的结构。",
                items=["H2/H3 决定分页", "图表围栏绑定小节", "公式原样保护"],
                conclusion="解析阶段就把错误报给作者",
                footer_tag="Parser",
                footer_tone="blue",
            ),
            card(
                "全局版式分配",
                "green",
                "i-carbon:grid",
                subtitle="决策层",
                body="每页先出 2–3 个候选并打分，再在整副牌组上求一次全局最优。",
                items=["相邻撞版扣分", "单一版式设上限", "超宽比例动态放宽"],
                conclusion="分配结果与并发数无关",
                footer_tag="Planner",
                footer_tone="green",
            ),
            card(
                "原生矢量渲染",
                "amber",
                "i-carbon:document-pdf",
                subtitle="渲染层",
                body="Typst 直接排版出 PDF 与位图，图表、公式、图标全部由代码绘制。",
                items=["内容实测缩放", "坏图不拖垮整副", "五种图表齐全"],
                conclusion="屏幕与导出共用同一条渲染路径",
                footer_tag="Renderer",
                footer_tone="amber",
            ),
            card(
                "自适应与回归",
                "red",
                "i-carbon:repeat",
                subtitle="质量层",
                body="字号按内容量分档，超出可用区域会被裁切并逐页上报。",
                items=["探测次数可观测", "溢出页逐条上报", "主题切换逐页对比"],
                conclusion="把「排不下」变成一份可读的报告",
                footer_tag="Quality",
                footer_tone="red",
            ),
        ],
    }
)

slides.append(
    {
        "kind": "issue_stack",
        "section": "01",
        "title": "手工做幻灯的四次断点",
        "footer": "四张卡片正好对应流水线里被自动化的四个环节。",
        "cards": [
            card(
                "版式靠手感",
                "amber",
                "i-carbon:warning-alt",
                body="同一份内容，不同人排出来的信息密度完全不同，评审时只能争论「感觉」。",
                items=["标准不统一", "复用成本高", "对齐困难"],
            ),
            card(
                "内容与版式耦合",
                "red",
                "i-carbon:flow-stream",
                body="写作阶段就要决定分栏与层级，改一句话常常推翻整页。",
                items=["返工频繁", "结构走形", "批量修改难"],
            ),
            card(
                "图表与公式外挂",
                "blue",
                "i-carbon:chart-bar",
                body="截图贴进来的图表不跟随主题，也无法随数据更新。",
                items=["分辨率不可控", "配色难统一", "数据难追溯"],
            ),
            card(
                "回归全靠肉眼",
                "green",
                "i-carbon:view",
                body="改了主题或组件，只能逐页翻看有没有溢出和错位。",
                items=["问题发现晚", "无法自动比对", "改版风险高"],
            ),
        ],
    }
)

slides.append(
    {
        "kind": "compare",
        "section": "01",
        "title": "手工排版 vs 幻述流水线",
        "subtitle": "同一份 2 800 字源稿，两条路径的成本结构",
        "footer": "高亮项是差异最大的一环：内容与版式的解耦程度。",
        "compare_data": {
            "mode": "pros-cons",
            "left": {
                "title": "幻述流水线",
                "tone": "green",
                "icon": "i-carbon:checkmark-filled",
                "items": [
                    {"label": "内容与版式解耦", "desc": "写作时只关心结构与事实", "highlight": True},
                    {"label": "版式全局分配", "desc": "28 页只撞版 3 次，换主题可重排"},
                    {"label": "图表与公式原生", "desc": "跟随主题换色，数据可追溯"},
                    {"label": "溢出即上报", "desc": "裁切页在导出报告里逐条列出"},
                ],
                "conclusion": "把「每页都一样好」交给规则，而不是状态",
            },
            "right": {
                "title": "手工排版",
                "tone": "red",
                "icon": "i-carbon:warning",
                "items": [
                    {"label": "边写边排版", "desc": "内容一改就要重排", "highlight": True},
                    {"label": "逐页凭手感", "desc": "信息密度随当下状态波动"},
                    {"label": "图表靠截图", "desc": "换主题要重新导出所有图"},
                    {"label": "靠肉眼回归", "desc": "溢出往往在投影时才被发现"},
                ],
                "conclusion": "质量取决于当下状态，无法稳定复现",
            },
        },
    }
)

slides.append(
    {
        "kind": "center_grid",
        "section": "01",
        "badge": "SUMMARY",
        "title": "让每一页自己找到版式",
        "accent": "版式不是作者的选择题：候选打分加上全局分配，会在约束内挑出最合适的一种。",
        "footer": "这是流水线里唯一需要「整副牌组视角」的一步。",
        "center_items": [
            {
                "title": "候选 2–3 个",
                "desc": "模型只在当页可用的版式里打分。",
                "icon": "i-carbon:list-bulleted",
                "tone": "blue",
            },
            {
                "title": "全局一次求解",
                "desc": "相邻同款扣 3 分，单一版式不超 30%。",
                "icon": "i-carbon:flow",
                "tone": "green",
            },
            {
                "title": "规则表过滤",
                "desc": "比例、要点数量、素材有无都是硬条件。",
                "icon": "i-carbon:rule",
                "tone": "amber",
            },
            {
                "title": "兜底永远可用",
                "desc": "模型失败时按先验分排序，仍给出合法页。",
                "icon": "i-carbon:checkmark-filled",
                "tone": "red",
            },
        ],
    }
)

# --------------------------------------------------------------------------- #
# 3 · 章节二：流水线解剖
# --------------------------------------------------------------------------- #

slides.append(
    {
        "kind": "section_intro",
        "section": "02",
        "badge": "章节导览",
        "title": "流水线解剖",
        "note": "第二章节逐段拆开生成过程：规划决定版面，生成填充内容，校验守住边界，渲染负责放下。",
        "cards": [
            card("分页规划", "blue", "i-carbon:layers", body="H2 决定章节号，H3 与小节内容量决定分页位置。"),
            card("候选打分", "green", "i-carbon:list-bulleted", body="每页仅在本页可用版式里排序，模型不做多样性判断。"),
            card("全局分配", "amber", "i-carbon:flow", body="相邻项精确求解，全局项用单页交换逼近。"),
            card("内容生成", "red", "i-carbon:edit", body="按选定版式的字段填充，缺失字段先补默认值。"),
            card("校验修复", "blue", "i-carbon:task-complete", body="结构错误带反馈重试，最后一轮退回通用版式。"),
            card("排版交付", "green", "i-carbon:generate-pdf", body="Typst 一次编译出 PDF，同一份蓝图光栅出缩略图。"),
        ],
    }
)

slides.append(
    {
        "kind": "process",
        "section": "02",
        "title": "三段式流水线",
        "subtitle": "规划 → 生成 → 校验，每一段都有确定性兜底",
        "footer": "并发生成只影响速度，不影响版式分配的结果。",
        "phases": [
            {
                "phase": "Phase 1",
                "title": "规划与分配",
                "icon": "i-carbon:layers",
                "tone": "blue",
                "steps": [
                    {"label": "解析源稿", "desc": "H2/H3 决定分页与章节号"},
                    {"label": "候选打分", "desc": "只在本页可用版式里排序"},
                    {"label": "全局分配", "desc": "相邻项用维特比精确求解"},
                ],
                "highlight": "分配结果与并发数无关",
            },
            {
                "phase": "Phase 2",
                "title": "内容生成",
                "icon": "i-carbon:edit",
                "tone": "green",
                "steps": [
                    {"label": "按版式填充", "desc": "字段不足时按结构补齐"},
                    {"label": "规范化", "desc": "长度收敛、默认值修复"},
                    {"label": "图表挂载", "desc": "数据由源稿原样搬入"},
                ],
                "highlight": "图表数据绝不由模型改写",
            },
            {
                "phase": "Phase 3",
                "title": "校验与修复",
                "icon": "i-carbon:task-complete",
                "tone": "amber",
                "steps": [
                    {"label": "结构校验", "desc": "缺字段、越界枚举立即拦下"},
                    {"label": "带反馈重试", "desc": "把失败原因回灌给模型"},
                    {"label": "兜底页", "desc": "仍失败则退回通用版式"},
                ],
                "highlight": "任何一页都不会静默丢失",
            },
        ],
    }
)

slides.append(
    {
        "kind": "step_flow",
        "section": "02",
        "title": "从源稿到出图：四步",
        "subtitle": "每一步的产物都能单独检查",
        "direction": "horizontal",
        "footer": "失败重试只回到出问题的那一步，不会从头再来。",
        "steps": [
            {
                "title": "解析源稿",
                "body": "切分章节、正文、图表围栏与公式。",
                "icon": "i-carbon:document",
                "tone": "blue",
            },
            {
                "title": "分配版式",
                "body": "每页候选打分，整副牌组一次求解。",
                "icon": "i-carbon:grid",
                "tone": "green",
            },
            {
                "title": "生成内容",
                "body": "按版式字段填充，规范化后校验。",
                "icon": "i-carbon:edit",
                "tone": "amber",
            },
            {
                "title": "排版交付",
                "body": "Typst 出 PDF 与位图，溢出逐页上报。",
                "icon": "i-carbon:generate-pdf",
                "tone": "red",
            },
        ],
    }
)

slides.append(
    {
        "kind": "split_layers",
        "section": "02",
        "title": "四层结构，每层只管一件事",
        "subtitle": "自下而上：输入 → 决策 → 渲染 → 交付",
        "footer": "层与层之间的接口就是蓝图 JSON；换掉任意一层，其余不动。",
        "left_items": [
            {
                "icon": "i-carbon:document",
                "title": "输入层只描述事实",
                "body": "源稿里的标题层级与图表数据是唯一事实来源，不含任何版式信息。",
            },
            {
                "icon": "i-carbon:rule",
                "title": "决策层只认规则",
                "body": "候选过滤、先验分与相邻惩罚都写在规则表里，可测试、可回归。",
            },
            {
                "icon": "i-carbon:document-pdf",
                "title": "渲染层只管放下",
                "body": "字号按内容量分档，实测后缩放；仍放不下就裁切并上报。",
            },
        ],
        "layers": [
            {"title": "交付层", "meta": "PDF · PNG · 演示位图", "tone": "red"},
            {"title": "渲染层", "meta": "Typst 版式与图表", "tone": "amber"},
            {"title": "决策层", "meta": "候选打分 + 全局分配", "tone": "green"},
            {"title": "输入层", "meta": "Markdown 源稿", "tone": "blue"},
        ],
    }
)

slides.append(
    {
        "kind": "focus_example",
        "section": "02",
        "title": "版式分配是一次全局优化",
        "subtitle": "以 28 页牌组为例：先算分，再算重复代价",
        "example_title": "目标函数",
        "example_body": "把每页的候选分数加总，再对重复版式收取代价：\n\n"
        "$$S = \\sum_{i} s_i - 3 \\cdot \\sum_{\\text{相邻同款}} 1 - 1 \\cdot \\sum_{\\text{隔页同款}} 1$$\n\n"
        "约束是任一版式不超过内容页的 30%（至少 2 页）。相邻项用维特比精确求解，"
        "全局项再用单页交换做局部搜索。",
        "points": [
            point(
                "i-carbon:flow",
                "先解相邻项",
                "相邻同款的惩罚只依赖前一项，状态空间是线性的。",
            ),
            point(
                "i-carbon:renew",
                "再放宽全局项",
                "单一版式上限用单页交换逼近，跑完仍是确定性结果。",
            ),
            point(
                "i-carbon:view",
                "最后写入调试目录",
                "逐页候选、被改选的页与兜底页都落盘，方便事后归因。",
            ),
        ],
        "ranking": [
            {"index": "1", "label": "候选分数 $s_i$", "meta": "模型按本页适配度打分"},
            {"index": "2", "label": "相邻同款惩罚", "meta": "权重 3，精确求解"},
            {"index": "3", "label": "隔页同款惩罚", "meta": "权重 1，抑制交替撞版"},
            {"index": "4", "label": "超出上限的次数", "meta": "权重 2，局部搜索", "muted": True},
        ],
    }
)

slides.append(
    {
        "kind": "section_list",
        "section": "02",
        "title": "交付前的四道检查",
        "subtitle": "任何一条不通过，导出报告都会列出具体页码",
        "list_items": [
            item(
                "01",
                "i-carbon:search",
                "结构完整",
                "每个版式的必填字段都在，枚举值都落在允许集合内。",
            ),
            item(
                "02",
                "i-carbon:chart-bar",
                "图表可读",
                "五种图表的数据行都能画出，环形图至少有一个正值扇区。",
            ),
            item(
                "03",
                "i-carbon:document-pdf",
                "版面放得下",
                "实测高度超过可用区域时缩放，仍溢出则裁切并上报。",
            ),
            item(
                "04",
                "i-carbon:checkmark-outline",
                "素材齐备",
                "引用的图片全部解析成功，坏图只影响它自己那一页。",
            ),
        ],
    }
)

# --------------------------------------------------------------------------- #
# 4 · 章节二续：产品界面（图片版式）
# --------------------------------------------------------------------------- #

slides.append(
    {
        "kind": "spotlight",
        "section": "02",
        "title": "三步工作流，一个窗口",
        "label": "产品界面",
        "label_tone": "blue",
        "side_width": "w-80",
        "image": "media:1",
        "placeholder": "工作台界面示意",
        "panels": [
            panel(
                "第一步 · 准备文稿",
                "blue",
                "i-carbon:document",
                body="Markdown 源稿里可以直接写图表围栏与 $公式$。",
                items=["H2 决定章节", "图表绑定小节", "公式原样保护"],
                highlight="源稿是唯一事实来源",
            ),
            {
                "title": "第二步 · 生成看板",
                "kind": "flow",
                "icon": "i-carbon:flow",
                "tone": "green",
                "items": ["解析与规划", "逐页生成", "校验与修复", "缩略图预览"],
            },
            panel(
                "第三步 · 编辑器",
                "amber",
                "i-carbon:edit",
                body="逐字段修改并实时预览，图表先过 JSON 校验再写回。",
                items=["单页重试", "图表 JSON 校验"],
                highlight="预览与导出同一条渲染路径",
            ),
        ],
    }
)

slides.append(
    {
        "kind": "spotlight",
        "section": "02",
        "title": "18 种版式，覆盖常见表达",
        "label": "版式清单",
        "label_tone": "green",
        "side_width": "w-72",
        "image": "media:3",
        "placeholder": "全部版式示意",
        "panels": [
            panel(
                "版式从哪来",
                "blue",
                "i-carbon:grid",
                body="每页先出候选，再在整副牌组上做一次全局分配。",
                items=["相邻同款惩罚 3", "单一版式上限 30%"],
                highlight="分配结果与并发数无关",
            ),
            panel(
                "边界怎么处理",
                "amber",
                "i-carbon:rule",
                body="超宽比例会屏蔽不适合的版式，要点数量也参与过滤。",
                items=["32:9 禁用主视觉", "48:9 再禁对比与示例"],
                highlight="通用兜底版式永远可用",
            ),
        ],
    }
)

slides.append(
    {
        "kind": "spotlight",
        "section": "02",
        "title": "八个环节，一条可观测链路",
        "label": "生成流水线",
        "label_tone": "amber",
        "side_width": "w-72",
        "image": "media:2",
        "placeholder": "生成流水线示意",
        "panels": [
            panel(
                "确定性兜底",
                "green",
                "i-carbon:checkmark-filled",
                body="模型没给出可用候选时，按先验分表排序并过滤硬条件。",
                items=["先验分覆盖全部版式", "命中的信号写进调试目录"],
            ),
            panel(
                "可观测",
                "blue",
                "i-carbon:view",
                body="每次生成都会把候选、分配与最终版式写进调试目录。",
                items=["逐页候选 JSON", "分配对比 JSON", "最终版式 JSON"],
                highlight="问题可以精确归因到某一页",
            ),
        ],
    }
)

slides.append(
    {
        "kind": "spotlight",
        "section": "02",
        "title": "一份源稿，四种规模",
        "label": "牌组数字",
        "label_tone": "red",
        "side_width": "w-72",
        "images": ["media:4", "media:5"],
        "placeholder": "封面视觉与牌组数字",
        "panels": [
            panel(
                "同一份蓝图",
                "blue",
                "i-carbon:repeat",
                body="页数、版式、图表与主题都是蓝图的属性，不写死在模板里。",
                items=["换主题不重排内容", "换画幅不重写字段"],
                highlight="28 页一次生成",
            ),
            panel(
                "多图平铺",
                "amber",
                "i-carbon:image",
                body="屏幕上这是轮播；PDF 一页放不下动画，于是改为等分平铺，最多六张。",
                items=["超过六张显示 +N", "缺图只影响自己这一格"],
            ),
        ],
    }
)

# --------------------------------------------------------------------------- #
# 5 · 章节三：实测数据（五种图表）
# --------------------------------------------------------------------------- #

slides.append(
    {
        "kind": "section_intro",
        "section": "03",
        "badge": "章节导览",
        "title": "实测数据",
        "note": "第三章节给出同一台机器、同一份源稿上的实测结果：耗时、探测次数与缓存收益。",
        "cards": [
            card("端到端耗时", "blue", "i-carbon:time", body="六个月里从 412 秒降到 121 秒，主要来自并发与增量编译。"),
            card("单页首帧", "green", "i-carbon:chart-bar", body="最重的图片页约 168 ms，最轻的封面页只需 78 ms。"),
            card("探测次数", "amber", "i-carbon:renew", body="自适应缩放在可行边界上插值，最坏情况 9 次探测。"),
            card("缓存收益", "red", "i-carbon:save", body="重复导出时大部分页直接命中记忆化缓存，不再全量重排。"),
        ],
    }
)

slides.append(
    {
        "kind": "chart_focus",
        "section": "03",
        "title": "端到端耗时：六个月下降 71%",
        "subtitle": "一副 28 页牌组，从源稿到 PDF 的总耗时（秒）",
        "points": [
            item("01", None, "并发规划", "每页候选打分并发执行，规划阶段从 180 秒降到 42 秒。"),
            item("02", None, "增量编译", "只重编变化的页，重复导出几乎不再付全量成本。"),
            item("03", None, "下一步", "目标是把中位耗时压到 90 秒以内。"),
        ],
        "chart": {
            "type": "line",
            "title": "端到端生成耗时（秒）",
            "data": [
                {"month": "2025-09", "seconds": 412},
                {"month": "2025-10", "seconds": 356},
                {"month": "2025-11", "seconds": 288},
                {"month": "2025-12", "seconds": 214},
                {"month": "2026-01", "seconds": 168},
                {"month": "2026-02", "seconds": 121},
            ],
            "encoding": {"x": "month", "y": "seconds"},
            "emphasis": {"highlight": ["2025-09", "2026-02"], "trend": "down"},
            **CHART_SOURCE,
        },
        "footer": "同一台机器、同一份 2 800 字源稿，取五次运行的中位数。",
    }
)

slides.append(
    {
        "kind": "chart_focus",
        "section": "03",
        "title": "单页首帧：版式之间差 2 倍",
        "subtitle": "同样的字体与主题，渲染器实测（毫秒）",
        "points": [
            item("01", None, "图片页最重", "主视觉页要解码图片并实测侧栏高度，168 ms 居首。"),
            item("02", None, "图表页次之", "五种图表都由 Typst 现画，约 145 ms。"),
            item("03", None, "栅格页最轻", "特性栅格只需排四张卡片，104 ms 就能出首帧。"),
        ],
        "chart": {
            "type": "bar",
            "title": "各版式单页首帧耗时（毫秒）",
            "data": [
                {"kind": "封面", "ms": 78},
                {"kind": "总览", "ms": 96},
                {"kind": "特性栅格", "ms": 104},
                {"kind": "时间线", "ms": 112},
                {"kind": "阶段流程", "ms": 118},
                {"kind": "SWOT", "ms": 121},
                {"kind": "图表解读", "ms": 145},
                {"kind": "主视觉", "ms": 168},
            ],
            "encoding": {"x": "kind", "y": "ms"},
            "emphasis": {"highlight": ["主视觉"], "trend": "down"},
            **CHART_SOURCE,
        },
        "footer": "release 构建，560 px 宽位图，首次渲染（不含字体扫描）。",
    }
)

slides.append(
    {
        "kind": "chart_focus",
        "section": "03",
        "title": "渲染耗时都花在哪",
        "subtitle": "文字整形仍然是大头",
        "points": [
            item("01", None, "文字整形", "中英混排的换行与字距占掉约 41% 的时间。"),
            item("02", None, "图表绘制", "五种图表现画，约占四分之一。"),
            item("03", None, "版面测量", "自适应缩放需要反复试排，测量成本不可忽略。"),
        ],
        "chart": {
            "type": "donut",
            "title": "单页渲染耗时构成",
            "data": [
                {"stage": "文字整形", "ms": 42},
                {"stage": "图表绘制", "ms": 26},
                {"stage": "版面测量", "ms": 18},
                {"stage": "光栅化", "ms": 11},
                {"stage": "其他", "ms": 3},
            ],
            "encoding": {"label": "stage", "value": "ms"},
            "emphasis": {"highlight": ["文字整形"]},
            **CHART_SOURCE,
        },
        "footer": "对 28 页逐页统计后按阶段归类，单位毫秒。",
    }
)

slides.append(
    {
        "kind": "chart_focus",
        "section": "03",
        "title": "三种画幅下的最坏探测次数",
        "subtitle": "自适应缩放要在可行边界上反复试排",
        "points": [
            item("01", None, "画幅越宽", "超宽画幅的列更多、卡片更窄，可用高度反而更紧张。"),
            item("02", None, "主视觉最贵", "侧栏与图片都要实测，48:9 下最坏要探测 9 次。"),
            item("03", None, "上界可控", "改动搜索策略后，探测次数比二分版少 3–5 次。"),
        ],
        "chart": {
            "type": "grouped_bar",
            "title": "最坏情况下的字号探测次数",
            "data": [
                {"kind": "特性栅格", "r16x9": 5, "r32x9": 6, "r48x9": 7},
                {"kind": "阶段流程", "r16x9": 6, "r32x9": 6, "r48x9": 7},
                {"kind": "SWOT", "r16x9": 5, "r32x9": 5, "r48x9": 6},
                {"kind": "时间线", "r16x9": 6, "r32x9": 7, "r48x9": 7},
                {"kind": "主视觉", "r16x9": 7, "r32x9": 8, "r48x9": 9},
            ],
            "encoding": {"x": "kind"},
            "series": [
                {"name": "16:9", "key": "r16x9"},
                {"name": "32:9", "key": "r32x9"},
                {"name": "48:9", "key": "r48x9"},
            ],
            **CHART_SOURCE,
        },
        "footer": "探测次数越多，代表这一页在可用面积上越接近临界。",
    }
)

slides.append(
    {
        "kind": "chart_focus",
        "section": "03",
        "title": "缓存命中撑起的增量编译",
        "subtitle": "累计编译次数（千次），阴影部分是重复排版",
        "points": [
            item("01", None, "命中率上升", "编辑器只发当前页，重复内容直接命中记忆化缓存。"),
            item("02", None, "成本可预测", "增量编译让每次改动的代价与改动页数成正比。"),
            item("03", None, "回收策略", "编译后主动回收缓存，避免长时间运行内存只增不减。"),
        ],
        "chart": {
            "type": "stacked_area",
            "title": "累计编译次数（千次）",
            "data": [
                {"month": "2025-09", "hit": 0.3, "miss": 1.9},
                {"month": "2025-10", "hit": 0.9, "miss": 3.4},
                {"month": "2025-11", "hit": 2.1, "miss": 4.8},
                {"month": "2025-12", "hit": 4.6, "miss": 6.1},
                {"month": "2026-01", "hit": 8.8, "miss": 7.2},
                {"month": "2026-02", "hit": 15.4, "miss": 8.0},
            ],
            "encoding": {"x": "month"},
            "series": [
                {"name": "命中缓存", "key": "hit"},
                {"name": "全量重排", "key": "miss"},
            ],
            "emphasis": {"highlight": ["2026-02"]},
            **CHART_SOURCE,
        },
        "footer": "统计口径是所有渲染入口（缩略图、演示位图、PDF 导出）之和。",
    }
)

slides.append(
    {
        "kind": "timeline",
        "section": "03",
        "title": "从原型到流水线",
        "subtitle": "五个关键节点，每一步都留下了可回归的固定样本",
        "footer": "固定样本是这套流水线能被持续改动的前提。",
        "timeline_events": [
            {
                "date": "2025-10-16",
                "title": "第一版生成流水线",
                "body": "从 Markdown 到蓝图跑通，版式仍由逐页贪心决定。",
                "tone": "blue",
                "icon": "i-carbon:code",
            },
            {
                "date": "2025-12-04",
                "title": "全局版式分配",
                "body": "候选打分、维特比与单页交换，相邻撞版显著下降。",
                "tone": "green",
                "icon": "i-carbon:grid",
            },
            {
                "date": "2026-01-20",
                "title": "Typst 原生渲染",
                "body": "缩略图、演示位图与 PDF 收敛到同一条渲染路径。",
                "tone": "amber",
                "icon": "i-carbon:document-pdf",
            },
            {
                "date": "2026-02-11",
                "title": "图表与公式原生",
                "body": "五种图表与 LaTeX 公式由代码绘制，不再依赖外挂。",
                "tone": "blue",
                "icon": "i-carbon:chart-bar",
            },
            {
                "date": "2026-03-02",
                "title": "自适应缩放重写",
                "body": "在可行边界上插值搜索，字号更贴近可用上限。",
                "tone": "red",
                "icon": "i-carbon:renew",
            },
        ],
    }
)

# --------------------------------------------------------------------------- #
# 6 · 章节四：取舍与下一步
# --------------------------------------------------------------------------- #

slides.append(
    {
        "kind": "section_intro",
        "section": "04",
        "badge": "章节导览",
        "title": "取舍与下一步",
        "note": "最后一章节把视角放回真实工作流：哪些是优势，哪些仍是风险，以及接下来做什么。",
        "cards": [
            card("守住协议", "blue", "i-carbon:rule", body="蓝图 JSON 是层与层之间唯一的契约，先保它稳定。"),
            card("扩样本", "green", "i-carbon:grid", body="把固定样本扩成主题与画幅的组合回归集。"),
            card("接数据", "amber", "i-carbon:chart-bar", body="让图表可以从真实数据源刷新，而不是手写 JSON。"),
            card("放开迭代", "red", "i-carbon:renew", body="模型与主题都可以换，只要样本还在，改动就有底。"),
        ],
    }
)

slides.append(
    {
        "kind": "swot",
        "section": "04",
        "title": "把流水线放进真实工作流",
        "subtitle": "优势来自确定性，风险来自协议漂移",
        "swot_data": {
            "quadrants": [
                {
                    "key": "strengths",
                    "title": "Strengths",
                    "tone": "green",
                    "icon": "i-carbon:checkmark-filled",
                    "items": [
                        "同一份源稿每次得到同一副牌组",
                        "版式、图表与公式都由代码排版，换主题无需重做",
                        "溢出与缺图会在导出报告里逐页列出",
                    ],
                    "summary": "优势在于确定性与可复现。",
                },
                {
                    "key": "weaknesses",
                    "title": "Weaknesses",
                    "tone": "red",
                    "icon": "i-carbon:close-filled",
                    "items": [
                        "长文与极端文案仍可能出现裁切",
                        "版式上限写在规则表里，需要人维护",
                        "多模态配图只参与选页，不参与裁剪",
                    ],
                    "summary": "弱点集中在极端内容与人工维护。",
                },
                {
                    "key": "opportunities",
                    "title": "Opportunities",
                    "tone": "blue",
                    "icon": "i-carbon:growth",
                    "items": [
                        "把固定样本扩成主题回归集",
                        "为 32:9 与 48:9 补专项样本",
                        "接入真实数据源，让图表随数据更新",
                    ],
                    "summary": "机会在于把回归做得更系统。",
                },
                {
                    "key": "threats",
                    "title": "Threats",
                    "tone": "amber",
                    "icon": "i-carbon:warning-alt",
                    "items": [
                        "版式协议演进会让旧牌组语义失真",
                        "字体缺失会让排版度量整体偏移",
                        "更换模型会改变候选分布，需要重新标定先验分",
                    ],
                    "summary": "风险主要来自协议与环境的漂移。",
                },
            ],
            "strategy": "先用固定样本守住协议边界，再放开模型与主题的迭代速度——"
            "只要样本还能逐页对比，任何改动都有回退的依据。",
        },
    }
)

slides.append(
    {
        "kind": "outcome_grid",
        "section": "04",
        "title": "四件事已经落地",
        "subtitle": "都在同一条渲染路径上实现，没有第二套兜底实现",
        "note": "以上四项都能用一份固定源稿完整重放。",
        "cards": [
            card(
                "一副牌组两种产物",
                "blue",
                "i-carbon:document-pdf",
                body="同一份蓝图，屏幕上是位图，交付是矢量 PDF。",
                conclusion="排版逻辑只有一套",
            ),
            card(
                "版式全覆盖",
                "green",
                "i-carbon:grid",
                body="18 种版式、5 种图表、6 套主题共用同一套蓝图结构。",
                conclusion="主题可以随时替换",
            ),
            card(
                "溢出可上报",
                "amber",
                "i-carbon:view",
                body="缩放仍放不下的页会被裁切，并逐条写进导出报告。",
                conclusion="问题不会静默丢失",
            ),
            card(
                "可回归",
                "red",
                "i-carbon:repeat",
                body="固定样本加逐页位图，让主题与版式调整都能对比。",
                conclusion="基线始终稳定",
            ),
        ],
    }
)

slides.append(
    {
        "kind": "closing",
        "title": "开始写你的第一页",
        "subtitle": "Markdown 进来，keynote 出去 —— 版式、图表与公式都交给渲染器。",
        "note": "下一步：把这份 28 页的样本存成基线，换一套主题再放一遍。",
        "badges": ["18 种版式", "5 种图表", "6 套主题", "Typst 原生"],
    }
)

# --------------------------------------------------------------------------- #
# media（素材库）：.keynn 的 content/media.json，编号与 blueprint 里的 media:<id> 对应
# --------------------------------------------------------------------------- #

MEDIA = [
    {
        "id": 1,
        "filename": "studio-mockup.png",
        "original_name": "工作台界面示意.png",
        "media_type": "image",
        "mime_type": "image/png",
        "caption": "幻述工作台：左侧三步工作流，中间生成看板，右侧逐字段编辑器",
        "description": "三步工作流界面示意，用于展示生成看板与编辑器预览。",
    },
    {
        "id": 2,
        "filename": "pipeline-diagram.png",
        "original_name": "生成流水线示意.png",
        "media_type": "image",
        "mime_type": "image/png",
        "caption": "生成流水线的八个环节：从 Markdown 源稿到 PDF 与位图交付",
        "description": "流水线示意图，标注规划、生成、校验与交付四类阶段。",
    },
    {
        "id": 3,
        "filename": "layout-poster.png",
        "original_name": "全部版式示意.png",
        "media_type": "image",
        "mime_type": "image/png",
        "caption": "18 种版式的线框对照：封面、总览、栅格、时间线、SWOT、图表页等",
        "description": "版式海报，一屏列出渲染器支持的全部版式。",
    },
    {
        "id": 4,
        "filename": "cover-art.png",
        "original_name": "封面视觉.png",
        "media_type": "image",
        "mime_type": "image/png",
        "caption": "报刊·钴蓝主题下的封面视觉：方角、细线与钴蓝强调色",
        "description": "封面视觉稿，取自主题的纸面与钴蓝配色。",
    },
    {
        "id": 5,
        "filename": "stats-poster.png",
        "original_name": "牌组数字.png",
        "media_type": "image",
        "mime_type": "image/png",
        "caption": "一副牌组的四个数字：26 页、18 种版式、5 种图表、6 套主题",
        "description": "数字海报，与封面视觉同尺寸，用于多图主视觉版式。",
    },
]


def image_size(path: Path):
    """Read PNG dimensions without extra dependencies (IHDR is always first)."""
    with path.open("rb") as handle:
        head = handle.read(24)
    assert head[:8] == b"\x89PNG\r\n\x1a\n", path
    return int.from_bytes(head[16:20], "big"), int.from_bytes(head[20:24], "big")


def build_media_json():
    rows = []
    for media in MEDIA:
        path = ASSETS / media["filename"]
        width, height = image_size(path)
        rows.append(
            {
                **media,
                "storage_path": f"projects/1/images/{media['filename']}",
                "slide_index": None,
                "file_size": path.stat().st_size,
                "width": width,
                "height": height,
            }
        )
    return rows


def build_keynn(blueprints_json: str, media_rows, markdown: str, target: Path):
    manifest = {
        "version": "1.0",
        "project_name": PROJECT_NAME,
        "exported_at": int(time.time()),
    }
    project = {
        "id": 0,
        "name": PROJECT_NAME,
        "md_content": markdown,
        "blueprints_json": blueprints_json,
        "media_root": "",
        "aspect_ratio": "ratio_16x9",
        "created_at": int(time.time()),
        "updated_at": int(time.time()),
    }
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as zf:
        zf.writestr("manifest.json", json.dumps(manifest, ensure_ascii=False, indent=2))
        zf.writestr("content/project.json", json.dumps(project, ensure_ascii=False, indent=2))
        zf.writestr("content/blueprints.json", blueprints_json)
        zf.writestr("content/media.json", json.dumps(media_rows, ensure_ascii=False, indent=2))
        for media in MEDIA:
            zf.write(ASSETS / media["filename"], f"content/media/images/{media['filename']}")


def main():
    blueprints_json = json.dumps(slides, ensure_ascii=False, indent=2) + "\n"
    (OUT / "blueprints.json").write_text(blueprints_json, encoding="utf-8")

    media_rows = build_media_json()
    (OUT / "media.json").write_text(
        json.dumps(media_rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )

    markdown = MD_PATH.read_text(encoding="utf-8")
    build_keynn(blueprints_json, media_rows, markdown, OUT / f"{PROJECT_NAME}.keynn")

    kinds = {}
    for slide in slides:
        kinds[slide["kind"]] = kinds.get(slide["kind"], 0) + 1
    print(f"{len(slides)} slides, {len(kinds)} kinds")
    for kind, count in kinds.items():
        print(f"  {kind:16} {count}")


if __name__ == "__main__":
    main()
