// spotlight — port of KeynoteSpotlightSlide.vue: a mounted image (or image set)
// beside a column of note panels.
//
// Differences from the screen version, on purpose:
// - `images` is a carousel on screen; a PDF page cannot rotate, so all images
//   are tiled (up to 6, then a "+N" badge).
// - The side column is fitted by measurement instead of the `sideScale`
//   character-count buckets, which is what overflowed on dense panels.

#import "../lib/components.typ": *

#let default-side = 288 // `w-72`

// Tailwind width class → CSS px: `w-80`, `w-[360px]`, `w-1/3` (of content width);
// a bare percentage (`42%`, the editor schema's format) is also of content width.
#let side-width(value, content-w) = {
  if not has(value) { return default-side }
  let m = str(value).match(regex("^(\d+(?:\.\d+)?)%$"))
  if m != none { return content-w * float(m.captures.at(0)) / 100 }
  let m = str(value).match(regex("^w-(\d+(?:\.\d+)?)$"))
  if m != none { return float(m.captures.at(0)) * 4 }
  let m = str(value).match(regex("^w-\[(\d+(?:\.\d+)?)px\]$"))
  if m != none { return float(m.captures.at(0)) }
  let m = str(value).match(regex("^w-(\d+)/(\d+)$"))
  if m != none { return content-w * int(m.captures.at(0)) / int(m.captures.at(1)) }
  default-side
}

// ── Media ─────────────────────────────────────────────────────────────────────

#let placeholder(ctx, w, h, label, detail: none) = block(
  width: w,
  height: h,
  fill: ctx.theme.paper,
  stroke: 1 * px + ctx.theme.edge,
  radius: if is-terminal(ctx) or is-glass(ctx) { ctx.theme.radius.card * px } else { 4 * px },
  {
    let body = {
      let ink = ctx.theme.ink.transparentize(60%)
      align(center, icon("i-carbon:image", ink, calc.min(ctx.ty.icon-hero, h * 0.35)))
      v(12 * px)
      align(center, txt(ctx, "caption", ctx.ty.meta, 1.45, label, fill: ink))
      if detail != none {
        align(center, txt(ctx, "code", ctx.ty.caption * 0.8, 1.4, detail, fill: ink))
      }
    }
    center-v(w, h, body)
  },
)

// `object-contain`, centered, never upscaling raster images past their pixel
// size (`max-w-full max-h-full`); rounded like the screen's `rounded-lg`.
#let contained(info, box-w, box-h) = {
  let natural-w = info.width * px
  let natural-h = info.height * px
  let scale = calc.min(box-w / natural-w, box-h / natural-h)
  let scale = if info.vector { scale } else { calc.min(scale, 1) }
  let (iw, ih) = (natural-w * scale, natural-h * scale)
  block(width: box-w, height: box-h, {
    v((box-h - ih) / 2)
    h((box-w - iw) / 2)
    box(width: iw, height: ih, radius: 8 * px, clip: true, image(info.path, width: iw, height: ih))
  })
}

#let tile-grid(n) = if n <= 1 { (1, 1) } else if n == 2 { (2, 1) } else if n <= 4 { (2, 2) } else { (3, 2) }

#let media(ctx, slide, w, h) = {
  let th = ctx.theme
  let refs = slide.at("images", default: ())
  let refs = if has(refs) { refs } else if has(slide.at("image", default: none)) { (slide.image,) } else { () }
  let label = slide.at("placeholder", default: none)
  let label = if has(label) { label } else { "图片待补充" }
  let frame = 6 // cream mat inside the hairline frame
  let inner-w = w - 2 * (frame + 1) * px
  let inner-h = h - 2 * (frame + 1) * px

  let body = if refs.len() == 0 {
    placeholder(ctx, inner-w, inner-h, label)
  } else if refs.len() == 1 {
    let info = ctx.assets.at(refs.first(), default: none)
    if info == none {
      placeholder(ctx, inner-w, inner-h, label, detail: "图片缺失：" + refs.first())
    } else { contained(info, inner-w, inner-h) }
  } else {
    let shown = refs.slice(0, calc.min(refs.len(), 6))
    let (cols, rows) = tile-grid(shown.len())
    let gap = 8 * px
    let pad = 8 * px
    let tile-w = (inner-w - 2 * pad - gap * (cols - 1)) / cols
    let tile-h = (inner-h - 2 * pad - gap * (rows - 1)) / rows
    block(inset: pad, grid(
      columns: (tile-w,) * cols,
      rows: (tile-h,) * rows,
      gutter: gap,
      ..shown
        .enumerate()
        .map(((i, reference)) => {
          let info = ctx.assets.at(reference, default: none)
          let tile = if info == none {
            placeholder(ctx, tile-w, tile-h, "图片缺失", detail: reference)
          } else { contained(info, tile-w, tile-h) }
          if i == shown.len() - 1 and refs.len() > shown.len() {
            // Images beyond the sixth are summarized, not silently dropped.
            tile
            place(bottom + right, dx: -6 * px, dy: -6 * px, box(
              fill: th.primary.transparentize(15%),
              radius: 2 * px,
              inset: (x: 8 * px, y: 2 * px),
              txt(ctx, "number", ctx.ty.caption, 1.4, "+" + str(refs.len() - shown.len()), fill: th.paper),
            ))
          } else { tile }
        }),
    ))
  }

  block(
    width: w,
    height: h,
    fill: th.paper,
    stroke: 1 * px + th.edge,
    radius: if is-glass(ctx) { th.radius.card * px } else { 2 * px },
    inset: frame * px,
    block(width: 100%, height: 100%, fill: th.media-bg, clip: true, body),
  )
}

// ── Side panels ───────────────────────────────────────────────────────────────

#let matrix-icons = (
  "i-carbon:map",
  "i-carbon:application",
  "i-carbon:run-mirror",
  "i-carbon:task-view",
  "i-carbon:lightning",
  "i-carbon:network-4",
  "i-carbon:compare",
)

#let panel(ctx, p, f) = {
  let th = ctx.theme
  let ty = ctx.ty
  let tone = tone-of(ctx, p.at("tone", default: none))
  let amber = th.tones.amber
  let meta = ty.meta * f
  let soft = th.ink.transparentize(30%)
  let mode = p.at("kind", default: "text")
  let items = p.at("items", default: ())
  let items = if items == none { () } else { items }
  let highlighted = has(p.at("highlight", default: none))

  let parts = ()
  // Heading
  let icon-name = p.at("icon", default: none)
  parts.push(grid(
    columns: if has(icon-name) { (auto, 1fr) } else { (1fr,) },
    column-gutter: 8 * px,
    align: horizon,
    ..if has(icon-name) { (icon(icon-name, tone.text, ty.icon-lg * f),) },
    txt(ctx, "title", ty.card-title * f, 1.3, p.title),
  ))
  if has(p.at("body", default: none)) {
    parts.push(txt(ctx, "body", meta, 1.45, rich(p.body, emphasis: tone.text), fill: soft))
  }
  if mode == "flow" and items.len() > 0 {
    let last = items.len() - 1
    parts.push(stack(spacing: 4 * px * f, ..items
      .enumerate()
      .map(((i, item)) => {
        let is-last = i == last
        let row = block(
          width: 100%,
          fill: if is-last { amber.bg } else { th.paper },
          stroke: (left: 2 * px + if is-last { amber.solid } else { tone.solid }, rest: 1 * px + th.edge),
          radius: 2 * px,
          inset: (x: 10 * px * f, y: 4 * px + 3 * px * f),
          grid(
            columns: (auto, 1fr),
            column-gutter: 8 * px,
            align: horizon,
            icon(
              if is-last { "i-carbon:rotate-clockwise" } else { p.at("icon", default: "i-carbon:chevron-right") },
              if is-last { th.primary } else { tone.text },
              meta,
            ),
            txt(ctx, "body", meta, 1.35, item, fill: if is-last { th.primary-strong } else { th.ink }),
          ),
        )
        if is-last { (row,) } else {
          (row, align(center, txt(ctx, "caption", ty.caption * f, 1, "↓", fill: th.muted.transparentize(50%))))
        }
      })
      .flatten()))
  } else if mode == "matrix" and items.len() > 0 {
    let last = items.len() - 1
    parts.push(grid(
      columns: (1fr, 1fr),
      gutter: 6 * px * f,
      ..items
        .enumerate()
        .map(((i, item)) => {
          let is-last = i == last
          let top = if is-last or i >= 4 { amber.solid } else { tone.solid }
          let ink = if i <= 3 { tone.text } else if is-last { th.primary-strong } else { th.primary }
          block(
            width: 100%,
            fill: if is-last { amber.bg } else { th.paper },
            stroke: (top: 2 * px + top, rest: 1 * px + th.edge),
            radius: 2 * px,
            inset: (x: 6 * px, y: 3 * px + 3 * px * f),
            grid(
              columns: (auto, 1fr),
              column-gutter: 4 * px,
              align: horizon,
              icon(matrix-icons.at(i, default: "i-carbon:checkmark-outline"), ink, ty.icon-sm * f * 0.875),
              txt(ctx, "body", meta, 1.35, item, fill: if is-last { th.primary-strong } else { th.ink }),
            ),
          )
        }),
    ))
  }
  let steps = if mode == "steps" { items } else { () }
  if has(steps) {
    parts.push(stack(spacing: 8 * px * f, ..steps
      .enumerate()
      .map(((i, step)) => {
        let badge = (8 + 12 * f) * px
        grid(
        columns: (badge, 1fr),
        column-gutter: 8 * px,
        align: horizon,
        box(width: badge, height: badge, fill: tone.solid, stroke: 1 * px + tone.border, radius: 2 * px, align(
          center + horizon,
          // The screen paints the digit in tone.text on tone.solid (invisible);
          // the contrast token is what makes the number readable.
          text(font: ctx.fonts.number, size: ty.caption * 0.85 * f, fill: tone.contrast, str(i + 1)),
        )),
        txt(ctx, "body", meta, 1.45, step, fill: soft),
      )
      })))
  }
  if highlighted {
    parts.push(txt(ctx, "body", meta, 1.45, p.highlight, fill: tone.text))
  }

  block(
    width: 100%,
    // light-notes forces the paper fill (`!important`) with a 3px tone bar on
    // the left; terminal and glass panels are plain glass cards and only pick
    // up the tone tint when the panel carries a `highlight` (see the inline
    // style guard in KeynoteSpotlightSlide.vue).
    fill: if is-terminal(ctx) or is-glass(ctx) { if highlighted { tone.bg } else { th.card } } else { th.paper },
    stroke: if is-terminal(ctx) or is-glass(ctx) { 1 * px + if highlighted { tone.border } else { th.edge } } else {
      (left: 3 * px + tone.solid, rest: 1 * px + if highlighted { tone.border } else { th.edge })
    },
    radius: if is-glass(ctx) { th.radius.card * px } else { 2 * px },
    inset: (x: 10 * px + 6 * px * f, y: 6 * px + 6 * px * f),
    stack(spacing: 8 * px * f, ..parts),
  )
}

// ── Page ──────────────────────────────────────────────────────────────────────

#let render(ctx, slide) = layout(size => {
  let th = ctx.theme
  let label-tone = tone-of(ctx, slide.at("label_tone", default: none), default: "blue")
  let label = slide.at("label", default: none)
  let label = if has(label) { label } else { "章节细化" }
  let header = block(width: size.width, grid(
    columns: (auto, 1fr),
    column-gutter: 12 * px,
    align: horizon,
    // Terminal `.tag-pill` radius is 2px, glass 999px.
    box(fill: label-tone.bg, stroke: 1 * px + label-tone.border, radius: if is-terminal(ctx) or is-glass(ctx) {
      th.radius.pill * px
    } else { 4 * px }, inset: (x: 10 * px, y: 2 * px), txt(
      ctx,
      "caption",
      ctx.ty.subtitle,
      1.5,
      label,
      fill: label-tone.text,
    )),
    txt(ctx, "title", ctx.ty.title, 1.18, slide.title, tracking: th.tracking.title),
  ))
  let gap = 16 * px
  let body-h = size.height - measure(header).height - gap
  let side-w = side-width(slide.at("side_width", default: none), size.width / px) * px
  let side-w = calc.min(side-w, size.width * 0.5)
  let media-w = size.width - side-w - 20 * px
  let panels = slide.at("panels", default: ())
  let panels = if panels == none { () } else { panels }

  header
  v(gap)
  grid(
    columns: (media-w, side-w),
    column-gutter: 20 * px,
    media(ctx, slide, media-w, body-h),
    block(width: side-w, height: body-h, fit-height(
      side-w,
      body-h,
      f => stack(spacing: 12 * px * f, ..panels.map(p => panel(ctx, p, f))),
      slide-index: ctx.index,
    )),
  )
})
