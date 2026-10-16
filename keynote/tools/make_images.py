#!/usr/bin/env python3
"""Draw the four assets used by the keynote deck (报刊·钴蓝 / editorial palette).

Everything is drawn with Pillow so the deck ships self-contained PNGs that match
the `light-editorial` Typst theme tokens (paper/ink/cobalt/edge).

    python3 keynote/tools/make_images.py keynote/assets
"""

import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

PAPER = "#f6f5f0"
CARD = "#ffffff"
INK = "#141414"
MUTED = "#73736f"
EDGE = "#d8d6cf"
COBALT = "#2549d8"
COBALT_BG = "#e9edfc"
COBALT_BORDER = "#bdc9f1"
GREEN = "#2e6e4f"
GREEN_BG = "#e2ede5"
AMBER = "#9a6212"
AMBER_BG = "#f6ecd9"
RED = "#a8322a"
RED_BG = "#f6e3e1"
SLATE = "#5b6f86"
VIOLET = "#7a5c99"

CJK = "/System/Library/Fonts/Hiragino Sans GB.ttc"
ARIAL = "/System/Library/Fonts/Supplemental/Arial.ttf"
ARIAL_BOLD = "/System/Library/Fonts/Supplemental/Arial Bold.ttf"
MENLO = "/System/Library/Fonts/Menlo.ttc"

_cache = {}


def cjk(size, w6=False):
    key = ("cjk", size, w6)
    if key not in _cache:
        _cache[key] = ImageFont.truetype(CJK, size, index=1 if w6 else 0)
    return _cache[key]


def latin(size, bold=False, mono=False):
    key = ("latin", size, bold, mono)
    if key not in _cache:
        path = MENLO if mono else (ARIAL_BOLD if bold else ARIAL)
        _cache[key] = ImageFont.truetype(path, size)
    return _cache[key]


def canvas(w, h, bg=PAPER):
    img = Image.new("RGB", (w, h), bg)
    return img, ImageDraw.Draw(img)


def text(d, xy, s, font, fill=INK, anchor="la"):
    d.text(xy, s, font=font, fill=fill, anchor=anchor)


def spaced(d, xy, s, font=None, fill=MUTED, tracking=4):
    """Letter-spaced kicker. Defaults to the CJK face, which also carries Latin."""
    """Letter-spaced small caps label, the editorial kicker look."""
    x, y = xy
    font = font or cjk(15, True)
    for ch in s:
        d.text((x, y), ch, font=font, fill=fill)
        x += d.textlength(ch, font=font) + tracking
    return x


def rule(d, box, fill=EDGE, width=1):
    x0, y0, x1, y1 = box
    d.rectangle([x0, y0, x1, y1], fill=fill)


def card(d, box, fill=CARD, border=EDGE, top=None, top_h=3):
    x0, y0, x1, y1 = box
    d.rectangle([x0, y0, x1, y1], fill=fill, outline=border, width=1)
    if top:
        d.rectangle([x0, y0, x1, y0 + top_h], fill=top)


def bar(d, x, y, w, h, fill=EDGE):
    d.rectangle([x, y, x + w, y + h], fill=fill)


def lines(d, x, y, widths, *, h=9, gap=16, fill=EDGE):
    cy = y
    for w in widths:
        bar(d, x, cy, w, h, fill)
        cy += h + gap - h
    return cy


def arrow(d, x0, y0, x1, y1, color=COBALT, width=3):
    d.line([x0, y0, x1, y1], fill=color, width=width)
    if y0 == y1:
        s = 12 if x1 > x0 else -12
        d.polygon([(x1, y1), (x1 - s, y1 - 7), (x1 - s, y1 + 7)], fill=color)
    else:
        s = 12 if y1 > y0 else -12
        d.polygon([(x1, y1), (x1 - 7, y1 - s), (x1 + 7, y1 - s)], fill=color)


# --------------------------------------------------------------------------- #
# 1. Studio mockup: the three-step workflow as one window.
# --------------------------------------------------------------------------- #
def studio_mockup(path):
    W, H = 1760, 1100
    img, d = canvas(W, H)
    card(d, (40, 40, W - 40, H - 40))
    rule(d, (40, 40, W - 41, 43), COBALT, 3)

    # title bar
    rule(d, (40, 104, W - 41, 105), EDGE)
    for i, c in enumerate((COBALT, AMBER, EDGE)):
        d.ellipse([70 + i * 26, 66, 82 + i * 26, 78], fill=c)
    text(d, (168, 60), "幻述 Huanshu", cjk(24, True))
    text(d, (312, 64), "— 产品发布.key", cjk(21), MUTED)
    text(d, (W - 80, 66), "报刊 · 钴蓝", cjk(19), MUTED, anchor="ra")

    # left rail
    rule(d, (260, 105, 261, H - 40), EDGE)
    rail = [("01", "准备文稿", True), ("02", "生成幻灯片", False), ("03", "编辑与导出", False)]
    y = 150
    for num, label, active in rail:
        if active:
            d.rectangle([56, y - 14, 244, y + 34], fill=COBALT_BG)
            d.rectangle([56, y - 14, 59, y + 34], fill=COBALT)
        text(d, (76, y - 6), num, latin(19, True), COBALT if active else MUTED)
        text(d, (112, y - 8), label, cjk(22, active), INK if active else MUTED)
        y += 76
    spaced(d, (72, H - 120), "TYPST", latin(14, True))

    # three panes
    panes = [
        (300, 660, "STEP 1 · MARKDOWN 源稿"),
        (990, 340, "STEP 2 · 生成看板"),
        (1360, 360, "STEP 3 · 编辑器"),
    ]
    for x, w, label in panes:
        spaced(d, (x, 140), label, cjk(16, True))
        rule(d, (x, 168, x + w, 169), COBALT, 2)

    # pane 1: markdown source
    x = 300
    text(d, (x, 190), "# 幻述 · 产品发布", cjk(23, True), COBALT)
    lines(d, x, 240, [420, 520, 480, 300], h=10, gap=26)
    card(d, (x, 380, x + 620, 470), fill="#efeee8", border=EDGE)
    text(d, (x + 18, 396), "```chart", latin(17, mono=True), GREEN)
    text(d, (x + 18, 420), '{ "type": "bar", ... }', latin(17, mono=True), INK)
    text(d, (x + 18, 442), "```", latin(17, mono=True), GREEN)
    lines(d, x, 500, [560, 460, 380], h=10, gap=26)
    text(d, (x, 610), "行内公式：", cjk(19), MUTED)
    text(d, (x + 110, 606), r"$T = \frac{W}{C}$", latin(21, mono=True), INK)
    d.rectangle([x, 660, x + 620, 664], fill=EDGE)
    text(d, (x, 680), "2 800 字 · 12 个小节 · 5 张图表", cjk(19), MUTED)

    # pane 2: generation board with four thumbnails
    x = 990
    thumbs = [(200, 112), (200, 112), (200, 112), (200, 112)]
    positions = [(x, 200), (x + 230, 200), (x, 370), (x + 230, 370)]
    for (tx, ty), (tw, th) in zip(positions, thumbs):
        card(d, (tx, ty, tx + tw, ty + th))
        rule(d, (tx, ty, tx + tw, ty + 2), COBALT, 2)
        bar(d, tx + 14, ty + 18, tw * 0.55, 8, INK)
        bar(d, tx + 14, ty + 34, tw * 0.8, 6, EDGE)
        bar(d, tx + 14, ty + 46, tw * 0.7, 6, EDGE)
        bar(d, tx + 14, ty + 62, tw * 0.4, 6, COBALT)
        bar(d, tx + 14, ty + 78, tw * 0.6, 6, EDGE)
    # one thumbnail gets a mini chart
    tx, ty = positions[1]
    for i, h in enumerate((22, 38, 30, 50)):
        bar(d, tx + 30 + i * 26, ty + 100 - h, 16, h, COBALT)
    # one gets tone chips
    tx, ty = positions[3]
    for i, c in enumerate((COBALT, GREEN, AMBER, RED)):
        bar(d, tx + 16 + i * 30, ty + 86, 24, 12, c)
    text(d, (x, 512), "28 页 · 生成中 20/28", cjk(20), MUTED)
    for i, (c, w) in enumerate(((COBALT, 190), (GREEN, 120), (EDGE, 60))):
        pass
    # progress bar
    d.rectangle([x, 546, x + 430, 556], fill="#ecebe5")
    bar(d, x, 546, 330, 10, COBALT)

    # pane 3: editor preview + form
    x = 1360
    card(d, (x, 190, x + 330, 376))
    rule(d, (x, 190, x + 330, 193), COBALT, 3)
    bar(d, x + 20, 214, 190, 12, INK)
    bar(d, x + 20, 236, 130, 8, EDGE)
    lines(d, x + 20, 266, [120, 150, 110], h=7, gap=20, fill=EDGE)
    # donut on the right of the preview
    cx, cy, r = x + 262, 292, 52
    d.ellipse([cx - r, cy - r, cx + r, cy + r], outline=COBALT, width=22)
    d.pieslice([cx - r, cy - r, cx + r, cy + r], -90, -90 + 92, fill=GREEN)
    d.pieslice([cx - r, cy - r, cx + r, cy + r], -90 + 92, -90 + 170, fill=EDGE)
    d.ellipse([cx - 26, cy - 26, cx + 26, cy + 26], fill=CARD)
    text(d, (cx, cy - 8), "28", latin(22, True), INK, anchor="ma")

    text(d, (x, 400), "版式", cjk(19), MUTED)
    card(d, (x, 426, x + 330, 468), border=EDGE)
    text(d, (x + 14, 434), "chart_focus", cjk(20))
    d.polygon([(x + 140, 442), (x + 156, 442), (x + 148, 452)], fill=MUTED)
    text(d, (x, 490), "标题", cjk(19), MUTED)
    card(d, (x, 516, x + 330, 558))
    text(d, (x + 14, 524), "交付量为什么在 Q3 跳升", cjk(20))
    text(d, (x, 580), "图表数据（JSON）", cjk(19), MUTED)
    card(d, (x, 606, x + 330, 700))
    text(d, (x + 14, 614), "{", latin(17, mono=True), GREEN)
    text(d, (x + 24, 638), '"type": "bar",', latin(17, mono=True), INK)
    text(d, (x + 24, 662), '"data": [ … ]', latin(17, mono=True), INK)
    text(d, (x + 14, 686), "}", latin(17, mono=True), GREEN)

    # status bar
    rule(d, (40, H - 78, W - 41, H - 77), EDGE)
    text(d, (72, H - 62), "第 7 / 28 页", cjk(19), MUTED)
    text(d, (W - 72, H - 62), "Typst 原生排版 · 0.30 s / 页", cjk(19), MUTED, anchor="ra")
    img.save(path)


# --------------------------------------------------------------------------- #
# 2. Pipeline diagram: eight stages in a serpentine.
# --------------------------------------------------------------------------- #
def pipeline(path):
    W, H = 1760, 900
    img, d = canvas(W, H)
    spaced(d, (80, 60), "生成流水线", cjk(17, True))
    text(d, (80, 92), "从一页 Markdown 到一副 keynote", cjk(40, True))
    rule(d, (80, 158, W - 80, 161), INK, 2)

    stages = [
        ("01", "Markdown 源稿", "标题层级、正文、图表围栏与公式", COBALT),
        ("02", "分页规划", "每页目标、要点、内容量与信号", COBALT),
        ("03", "版式候选", "LLM 逐页打分，规则表过滤边界", COBALT),
        ("04", "全局分配", "最大化适配度，抑制相邻撞版", COBALT),
        ("05", "内容生成", "按选定版式填充字段与文案", GREEN),
        ("06", "校验与修复", "结构校验，失败页带反馈重试", GREEN),
        ("07", "Typst 排版", "矢量页、真实测量、自适应字号", AMBER),
        ("08", "交付", "PDF · 缩略图 · 演示位图", AMBER),
    ]
    bw, bh, gap = 360, 200, 30
    x0, y0 = 80, 210
    cells = []
    for i, stage in enumerate(stages):
        row, col = divmod(i, 4)
        if row == 0:
            cx = x0 + col * (bw + gap)
        else:
            cx = x0 + (3 - col) * (bw + gap)
        cy = y0 + row * (bh + 90)
        cells.append((cx, cy))

    for (cx, cy), (num, title, desc, accent) in zip(cells, stages):
        card(d, (cx, cy, cx + bw, cy + bh), top=accent, top_h=6)
        text(d, (cx + 24, cy + 28), num, latin(30, True), accent)
        text(d, (cx + 24, cy + 74), title, cjk(29, True))
        text(d, (cx + 24, cy + 122), desc, cjk(19), MUTED)
        d.rectangle([cx + 24, cy + 158, cx + 64, cy + 161], fill=accent)

    for i in range(3):
        cx, cy = cells[i]
        arrow(d, cx + bw + 6, cy + bh / 2, cx + bw + gap - 6, cy + bh / 2)
    # elbow from row 1 right edge down to row 2 right edge
    xr = x0 + 3 * (bw + gap) + bw
    arrow(d, xr + 14, y0 + bh / 2, xr + 14, y0 + bh + 60)
    arrow(d, xr + 14, y0 + bh + 60, xr - bw - gap + bw + 14, y0 + bh + 60)
    for i in range(4, 7):
        cx, cy = cells[i]
        arrow(d, cx - gap + 6, cy + bh / 2, cx - 6, cy + bh / 2)

    text(
        d,
        (80, H - 80),
        "确定性兜底：LLM 未给出可用候选时，按先验分表排序并过滤边界条件，仍能给出合法版式。",
        cjk(21),
        MUTED,
    )
    img.save(path)


# --------------------------------------------------------------------------- #
# 3. Layout poster: all 18 layouts as wireframes.
# --------------------------------------------------------------------------- #
def _wire(d, x, y, w, h, kind):
    """A 260x150-ish wireframe tile body."""
    e = EDGE
    if kind == "cover":
        bar(d, x + 14, y + 34, 130, 16, INK)
        bar(d, x + 14, y + 58, 96, 16, INK)
        bar(d, x + 14, y + 86, 70, 7, e)
        d.rectangle([x + 168, y + 30, x + 232, y + 116], outline=COBALT, width=3)
        bar(d, x + 186, y + 56, 28, 34, None) if False else None
        text(d, (x + 200, y + 72), "01", latin(30, True), COBALT, anchor="mm")
    elif kind == "overview":
        bar(d, x + 14, y + 22, 110, 12, INK)
        for i in range(4):
            cx = x + 14 + (i % 2) * 120
            cy = y + 50 + (i // 2) * 46
            card(d, (cx, cy, cx + 108, cy + 38))
            bar(d, cx + 8, cy + 8, 14, 8, COBALT)
            bar(d, cx + 30, cy + 8, 60, 7, e)
            bar(d, cx + 8, cy + 24, 80, 6, e)
    elif kind == "feature_grid":
        for i in range(4):
            cx = x + 10 + i * 60
            card(d, (cx, y + 24, cx + 52, y + 126), top=COBALT, top_h=3)
            bar(d, cx + 8, y + 38, 22, 8, COBALT)
            bar(d, cx + 8, y + 56, 36, 6, INK)
            lines(d, cx + 8, y + 72, [36, 30, 34], h=5, gap=13, fill=e)
    elif kind == "spotlight":
        card(d, (x + 12, y + 22, x + 152, y + 128), fill=COBALT_BG, border=COBALT_BORDER)
        d.line([x + 12, y + 22, x + 152, y + 128], fill=COBALT_BORDER)
        d.line([x + 12, y + 128, x + 152, y + 22], fill=COBALT_BORDER)
        for i in range(3):
            card(d, (x + 164, y + 22 + i * 36, x + 248, y + 52 + i * 36))
            bar(d, x + 172, y + 30 + i * 36, 40, 6, COBALT if i == 0 else e)
            bar(d, x + 172, y + 42 + i * 36, 64, 5, e)
    elif kind == "split_layers":
        lines(d, x + 14, y + 30, [96, 88, 74], h=7, gap=22, fill=e)
        for i in range(4):
            bar(d, x + 140, y + 24 + i * 28, 106, 20, [COBALT, AMBER, GREEN, RED][i])
    elif kind == "section_list":
        for i in range(4):
            cy = y + 22 + i * 28
            bar(d, x + 14, cy, 20, 16, COBALT)
            bar(d, x + 44, cy + 2, 90, 9, INK)
            bar(d, x + 150, cy + 3, 100, 6, e)
    elif kind == "focus_example":
        for i in range(3):
            card(d, (x + 12, y + 22 + i * 36, x + 118, y + 52 + i * 36))
            bar(d, x + 22, y + 31 + i * 36, 44, 6, INK)
            bar(d, x + 22, y + 42 + i * 36, 70, 5, e)
        for i in range(4):
            bar(d, x + 130, y + 26 + i * 26, 14, 10, COBALT if i == 0 else e)
            bar(d, x + 152, y + 27 + i * 26, 76, 8, e)
    elif kind == "outcome_grid":
        for i in range(3):
            cx = x + 12 + i * 80
            card(d, (cx, y + 24, cx + 72, y + 118))
            bar(d, cx + 8, y + 34, 20, 20, [COBALT, GREEN, AMBER][i])
            bar(d, cx + 8, y + 62, 44, 7, INK)
            bar(d, cx + 8, y + 76, 54, 5, e)
            bar(d, cx + 8, y + 92, 40, 5, COBALT)
    elif kind == "center_grid":
        d.rectangle([x + 96, y + 56, x + 164, y + 94], fill=INK)
        text(d, (x + 130, y + 75), "结论", cjk(17), CARD, anchor="mm")
        for i, (dx, dy) in enumerate(((-92, -34), (78, -34), (-92, 30), (78, 30))):
            cx, cy = x + 130 + dx, y + 75 + dy
            card(d, (cx, cy, cx + 92, cy + 30))
            bar(d, cx + 8, cy + 10, 44, 6, [COBALT, GREEN, AMBER, RED][i])
    elif kind == "timeline":
        d.line([x + 16, y + 76, x + 244, y + 76], fill=EDGE, width=3)
        for i in range(4):
            cx = x + 40 + i * 56
            d.ellipse([cx - 6, y + 70, cx + 6, y + 82], fill=COBALT)
            top = i % 2 == 0
            cy = y + 26 if top else y + 92
            card(d, (cx - 24, cy, cx + 24, cy + 34))
            bar(d, cx - 16, cy + 8, 26, 5, INK)
            bar(d, cx - 16, cy + 20, 34, 4, e)
    elif kind == "step_flow":
        for i in range(4):
            cx = x + 12 + i * 60
            d.polygon(
                [
                    (cx, y + 56),
                    (cx + 44, y + 56),
                    (cx + 56, y + 74),
                    (cx + 44, y + 92),
                    (cx, y + 92),
                    (cx + 12, y + 74),
                ],
                fill=[COBALT, GREEN, AMBER, RED][i],
            )
            text(d, (cx + 26, y + 74), str(i + 1), latin(18, True), CARD, anchor="mm")
    elif kind == "process":
        for i in range(3):
            cx = x + 12 + i * 82
            card(d, (cx, y + 22, cx + 72, y + 128))
            bar(d, cx, y + 22, 72, 4, [COBALT, GREEN, AMBER][i])
            bar(d, cx + 8, y + 36, 34, 7, INK)
            for j in range(3):
                bar(d, cx + 8, y + 54 + j * 20, 56, 12, "#f1f0ea")
                bar(d, cx + 8, y + 58 + j * 20, 30, 4, e)
    elif kind == "compare":
        card(d, (x + 12, y + 24, x + 122, y + 126), top=GREEN, top_h=4)
        card(d, (x + 138, y + 24, x + 248, y + 126), top=RED, top_h=4)
        for i in range(3):
            bar(d, x + 22, y + 40 + i * 26, 70, 7, e)
            bar(d, x + 148, y + 40 + i * 26, 70, 7, e)
        text(d, (x + 130, y + 74), "VS", latin(15, True), MUTED, anchor="mm")
    elif kind == "swot":
        for i in range(4):
            cx = x + 12 + (i % 2) * 122
            cy = y + 22 + (i // 2) * 56
            card(d, (cx, cy, cx + 110, cy + 46), top=[GREEN, RED, COBALT, AMBER][i], top_h=4)
            bar(d, cx + 8, cy + 16, 34, 7, INK)
            bar(d, cx + 8, cy + 30, 84, 5, e)
    elif kind == "section_intro":
        d.rectangle([x + 12, y + 22, x + 108, y + 128], fill=COBALT_BG)
        bar(d, x + 22, y + 36, 56, 9, INK)
        lines(d, x + 22, y + 56, [70, 60, 46], h=5, gap=13, fill=COBALT_BORDER)
        for i in range(4):
            cx = x + 120 + (i % 2) * 66
            cy = y + 22 + (i // 2) * 56
            card(d, (cx, cy, cx + 58, cy + 46))
            bar(d, cx + 8, cy + 10, 16, 16, [GREEN, COBALT, AMBER, RED][i])
            bar(d, cx + 8, cy + 32, 40, 5, e)
    elif kind == "issue_stack":
        for i in range(4):
            cy = y + 22 + i * 28
            card(d, (x + 12, cy, x + 248, cy + 22))
            bar(d, x + 12, cy, 4, 22, [AMBER, RED, COBALT, GREEN][i])
            bar(d, x + 26, cy + 7, 62, 8, INK)
            bar(d, x + 110, cy + 8, 128, 6, e)
    elif kind == "chart_focus":
        for i in range(3):
            bar(d, x + 14, y + 30 + i * 30, 14, 10, COBALT)
            bar(d, x + 34, y + 31 + i * 30, 56, 8, INK)
            bar(d, x + 34, y + 44 + i * 30, 74, 5, e)
        d.line([x + 126, y + 120, x + 248, y + 120], fill=EDGE, width=2)
        d.line([x + 126, y + 120, x + 126, y + 26], fill=EDGE, width=2)
        for i, hh in enumerate((22, 40, 32, 58, 46)):
            bar(d, x + 138 + i * 22, y + 120 - hh, 14, hh, COBALT)
    elif kind == "closing":
        bar(d, x + 76, y + 52, 108, 14, INK)
        bar(d, x + 96, y + 76, 68, 7, e)
        d.line([x + 90, y + 96, x + 170, y + 96], fill=COBALT, width=3)
        text(d, (x + 130, y + 112), "END", latin(15, True), MUTED, anchor="mm")


def layout_poster(path):
    kinds = [
        ("cover", "封面 Cover"),
        ("overview", "总览 Overview"),
        ("feature_grid", "特性栅格 Feature Grid"),
        ("spotlight", "主视觉 Spotlight"),
        ("split_layers", "分层 Split Layers"),
        ("section_list", "清单 Section List"),
        ("focus_example", "论点示例 Focus Example"),
        ("outcome_grid", "成果栅格 Outcome Grid"),
        ("center_grid", "中心主旨 Center Grid"),
        ("timeline", "时间线 Timeline"),
        ("step_flow", "步骤流 Step Flow"),
        ("process", "阶段流程 Process"),
        ("compare", "对比 Compare"),
        ("swot", "SWOT 分析"),
        ("section_intro", "章节导览 Section Intro"),
        ("issue_stack", "问题堆叠 Issue Stack"),
        ("chart_focus", "图表解读 Chart Focus"),
        ("closing", "结尾 Closing"),
    ]
    cols, rows = 6, 3
    tw, th, gx, gy = 260, 190, 16, 26
    W = 80 * 2 + cols * tw + (cols - 1) * gx
    H = 240 + rows * (th + gy) + 40
    img, d = canvas(W, H)
    spaced(d, (80, 56), "版式清单", cjk(17, True))
    text(d, (80, 86), "18 种版式，一次生成", cjk(40, True))
    text(d, (W - 80, 104), "版式由渲染器原生排版 · 尺寸实测 · 溢出即裁切并上报", cjk(20), MUTED, anchor="ra")
    rule(d, (80, 152, W - 80, 155), INK, 2)

    for i, (kind, label) in enumerate(kinds):
        col, row = i % cols, i // cols
        x = 80 + col * (tw + gx)
        y = 200 + row * (th + gy)
        card(d, (x, y, x + tw, y + 150))
        rule(d, (x, y, x + tw, y + 3), COBALT, 3)
        _wire(d, x, y, tw, 150, kind)
        text(d, (x, y + 160), label, cjk(19, True), INK)
        text(d, (x, y + 182), f"kind: {kind}", latin(14, mono=True), MUTED)
    img.save(path)


# --------------------------------------------------------------------------- #
# 4. Cover art: editorial geometric hero.
# --------------------------------------------------------------------------- #
def cover_art(path):
    """Landscape hero: editorial wordmark left, geometric mark right (≈16:9)."""
    W, H = 1760, 900
    img, d = canvas(W, H)
    for i in range(1, 6):
        d.line([i * W / 6, 0, i * W / 6, H], fill="#eceae4", width=1)
    for i in range(1, 4):
        d.line([0, i * H / 4, W, i * H / 4], fill="#eceae4", width=1)

    d.rectangle([860, 110, 1420, 670], outline=COBALT, width=4)
    d.rectangle([960, 190, 1260, 490], fill=INK)
    d.ellipse([1170, 370, 1670, 870], fill=COBALT)
    d.ellipse([1230, 430, 1610, 810], fill=PAPER)
    d.ellipse([1290, 490, 1550, 750], fill=COBALT)

    for i in range(6):
        bar(d, 860 + i * 62, 726, 38, 8, INK if i % 2 == 0 else EDGE)

    spaced(d, (140, 170), "编者按 · EDITORIAL", cjk(22, True), MUTED, tracking=6)
    text(d, (140, 300), "幻述", cjk(190, True), INK)
    rule(d, (142, 580, 780, 584), COBALT, 4)
    text(d, (140, 616), "HUANSHU  ·  MARKDOWN → KEYNOTE", latin(26, True), MUTED)
    text(d, (140, 676), "一页源稿，一副 keynote", cjk(30), MUTED)
    img.save(path)


# --------------------------------------------------------------------------- #
# 5. Stats poster: four big numbers, built to sit next to the cover art.
# --------------------------------------------------------------------------- #
def stats_poster(path):
    W, H = 1760, 900
    img, d = canvas(W, H)
    spaced(d, (140, 96), "一副牌组的四个数字", cjk(22, True), MUTED, tracking=6)
    text(d, (140, 140), "一份源稿，四种规模", cjk(44, True))
    d.rectangle([140, 226, W - 140, 230], fill=INK)

    stats = [
        ("28", "页", "一副完整牌组的页数", COBALT),
        ("18", "种版式", "渲染器支持的全部版式", GREEN),
        ("5", "种图表", "折线 / 柱状 / 环形 / 分组柱状 / 堆叠面积", AMBER),
        ("6", "套主题", "同一份蓝图，换主题不重排内容", RED),
    ]
    col = (W - 280) / 4
    for i, (num, unit, desc, accent) in enumerate(stats):
        x = 140 + i * col
        d.rectangle([x, 300, x + col - 46, 304], fill=accent)
        text(d, (x, 340), num, latin(150, True), INK)
        nw = d.textlength(num, font=latin(150, True))
        text(d, (x + nw + 14, 412), unit, cjk(30, True), accent)
        text(d, (x, 540), desc, cjk(22), MUTED)
        if i < 3:
            d.line([x + col - 26, 330, x + col - 26, 660], fill=EDGE, width=1)

    for i in range(6):
        bar(d, 140 + i * 62, 740, 38, 8, INK if i % 2 == 0 else EDGE)
    text(d, (140, 786), "HUANSHU · 2026", latin(24, True), MUTED)
    img.save(path)


def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "keynote/assets")
    out.mkdir(parents=True, exist_ok=True)
    studio_mockup(out / "studio-mockup.png")
    pipeline(out / "pipeline-diagram.png")
    layout_poster(out / "layout-poster.png")
    cover_art(out / "cover-art.png")
    stats_poster(out / "stats-poster.png")
    for f in sorted(out.iterdir()):
        print(f"{f}  {f.stat().st_size // 1024} KB")


if __name__ == "__main__":
    main()
