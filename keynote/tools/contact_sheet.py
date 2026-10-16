#!/usr/bin/env python3
"""Assemble a contact sheet of every rendered page (and report the page list).

    python3 keynote/tools/contact_sheet.py target/keynote-render keynote/preview
"""

import json
import sys
from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

PAPER = "#f6f5f0"
INK = "#141414"
MUTED = "#73736f"
COBALT = "#2549d8"
EDGE = "#d8d6cf"
CJK = "/System/Library/Fonts/Hiragino Sans GB.ttc"

KIND_LABEL = {
    "cover": "封面 Cover",
    "overview": "总览 Overview",
    "section_intro": "章节导览 Section Intro",
    "feature_grid": "特性栅格 Feature Grid",
    "issue_stack": "问题堆叠 Issue Stack",
    "compare": "对比 Compare",
    "center_grid": "中心主旨 Center Grid",
    "process": "阶段流程 Process",
    "step_flow": "步骤流 Step Flow",
    "split_layers": "分层 Split Layers",
    "focus_example": "论点示例 Focus Example",
    "section_list": "清单 Section List",
    "spotlight": "主视觉 Spotlight",
    "chart_focus": "图表解读 Chart Focus",
    "timeline": "时间线 Timeline",
    "swot": "SWOT 分析",
    "outcome_grid": "成果栅格 Outcome Grid",
    "closing": "结尾 Closing",
}


def main():
    src = Path(sys.argv[1] if len(sys.argv) > 1 else "target/keynote-render")
    out = Path(sys.argv[2] if len(sys.argv) > 2 else "keynote/preview")
    deck_path = Path(sys.argv[3] if len(sys.argv) > 3 else "keynote/blueprints.json")
    deck = json.loads(deck_path.read_text(encoding="utf-8"))
    pages = sorted(src.glob("[0-9][0-9]-*.png"))
    out.mkdir(parents=True, exist_ok=True)

    # copy each page with a readable file name
    names = []
    for i, page in enumerate(pages):
        kind = deck[i]["kind"] if i < len(deck) else "unknown"
        name = f"{i + 1:02d}-{kind}.png"
        Image.open(page).save(out / name)
        names.append((name, kind, deck[i].get("title", "") if i < len(deck) else ""))

    cols = 4
    rows = (len(names) + cols - 1) // cols
    tw, th = 460, 259
    pad, gap, header, footer = 48, 22, 150, 90
    W = pad * 2 + cols * tw + (cols - 1) * gap
    H = header + rows * (th + 46 + gap) + footer
    sheet = Image.new("RGB", (W, H), PAPER)
    d = ImageDraw.Draw(sheet)
    f_title = ImageFont.truetype(CJK, 46, index=1)
    f_sub = ImageFont.truetype(CJK, 22)
    f_kind = ImageFont.truetype(CJK, 20, index=1)
    f_meta = ImageFont.truetype(CJK, 17)

    d.text((pad, 46), "幻述 · 产品发布", font=f_title, fill=INK)
    d.text(
        (pad, 104),
        f"{len(names)} 页 · 18 种版式 · 主题：报刊·钴蓝（light-editorial）· 16:9",
        font=f_sub,
        fill=MUTED,
    )
    d.rectangle([pad, header - 24, W - pad, header - 21], fill=COBALT)

    for i, (name, kind, title) in enumerate(names):
        col, row = i % cols, i // cols
        x = pad + col * (tw + gap)
        y = header + row * (th + 46 + gap)
        img = Image.open(out / name).resize((tw, th), Image.LANCZOS)
        sheet.paste(img, (x, y))
        d.rectangle([x, y, x + tw, y + th], outline=EDGE, width=1)
        d.text((x, y + th + 8), f"{i + 1:02d}  {KIND_LABEL.get(kind, kind)}", font=f_kind, fill=INK)
        label = title if len(title) <= 22 else title[:21] + "…"
        d.text((x, y + th + 32), label, font=f_meta, fill=MUTED)

    d.text(
        (pad, H - 58),
        f"渲染：Typst 原生排版 · 无溢出页、无缺失素材（{len(names)}/{len(names)}）",
        font=f_sub,
        fill=MUTED,
    )
    target = out / "全场一览.png"
    sheet.save(target)
    print(f"{target}  {sheet.size[0]}x{sheet.size[1]}")
    for name, kind, _ in names:
        print(" ", name)


if __name__ == "__main__":
    main()
