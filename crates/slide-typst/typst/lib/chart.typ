// Native chart drawing for the `chart_focus` layout.
//
// A port of `buildChartOption` (src/components/chartOptions.ts), checked against
// the ECharts SSR output this replaces (the SVGs the app used to render in the
// webview): grid margins, line smoothing, the pin bubble, donut radii and the
// legend geometry are the numbers ECharts wrote into those files. As in the
// option builder, only the `fs(n)` sizes follow the slide's --type-scale
// (`ctx.scale`); grid margins, line widths, symbol sizes and radii are fixed
// CSS px.
//
// Geometry is plain floats in CSS px, turned into lengths only when drawn.
// Everything is placed absolutely inside a `width × height` box that clips.
//
// Chart strings (categories, series names) are data: they reach `text()` as
// plain strings, never as markup and never through `rich()`.

#import "base.typ": px, line-box

// ── Values ────────────────────────────────────────────────────────────────────

#let number-pattern = regex("^\s*[-+]?(\d+\.?\d*|\.\d+)([eE][-+]?\d+)?\s*$")

// A data cell as a finite float, or none. Numeric strings ("12.5") count, as in
// `ChartSpec::problems()`; `float()` is only called on values known to parse,
// since a failing conversion would fail the whole export.
#let as-number(value) = {
  let v = if type(value) == int or type(value) == float {
    float(value)
  } else if type(value) == str and value.match(number-pattern) != none {
    float(value.trim().trim("+", at: start))
  } else {
    none
  }
  if v == none or v.is-nan() or v.is-infinite() { none } else { v }
}

// Category / series names as display text (JSON may hold numbers there).
#let as-text(value) = {
  if type(value) == str { value } else if value == none { "" } else if type(value) == int or type(value) == float {
    str(value)
  } else { repr(value) }
}

// Thousands separators, at most `digits` decimals with trailing zeros dropped,
// no negative zero and no float tails (`0.1 + 0.2` prints 0.3).
#let format-number(value, digits: 2) = {
  let v = calc.round(float(value), digits: digits)
  if v == 0 { return "0" }
  let a = calc.abs(v)
  if a >= 1e15 { return str(v) }
  let whole = calc.floor(a)
  let unit = calc.pow(10, digits)
  let frac = int(calc.round((a - whole) * unit))
  if frac >= unit {
    whole += 1
    frac = 0
  }
  let digits-text = str(whole)
  let grouped = ""
  for (i, c) in digits-text.clusters().enumerate() {
    if i > 0 and calc.rem(digits-text.len() - i, 3) == 0 { grouped += "," }
    grouped += c
  }
  let frac-text = if frac == 0 { "" } else {
    let f = str(frac)
    "." + ("0" * (digits - f.len()) + f).trim("0", at: end)
  }
  (if v < 0 { "-" } else { "" }) + grouped + frac-text
}

// ECharts' `nice(x, round = true)`: the interval for `span / splitNumber (5)`.
#let nice-interval(value) = {
  let exponent = calc.floor(calc.log(value, base: 10))
  let base = calc.pow(10.0, exponent)
  let f = value / base
  let nice = if f < 1.5 { 1 } else if f < 2.5 { 2 } else if f < 4 { 3 } else if f < 7 { 5 } else { 10 }
  (step: nice * base, exponent: exponent)
}

// Value axis: always contains 0, ends on whole intervals. ECharts gives
// 41 → 0..50 by 10, 530 → 0..600 by 100, 170 → 0..180 by 30, 119 → 0..120 by 20.
#let value-axis(values) = {
  let lo = values.fold(0.0, calc.min)
  let hi = values.fold(0.0, calc.max)
  if hi - lo <= 0 { hi = 1.0 }
  let nice = nice-interval((hi - lo) / 5)
  let step = nice.step
  let first = calc.floor(lo / step + 1e-9)
  let last = calc.ceil(hi / step - 1e-9)
  (
    min: first * step,
    max: last * step,
    ticks: range(first, last + 1).map(k => k * step),
    digits: calc.min(6, calc.max(0, -nice.exponent)),
  )
}

// ── Reading the spec ──────────────────────────────────────────────────────────
// `ChartSpec::problems()` keeps malformed charts out of new decks, but old data
// and hand-edited JSON still reach the renderer. Returns `(ok: false, reason)`
// for anything that cannot be drawn: the layout shows its placeholder instead
// of failing the whole export.

#let chart-kinds = ("line", "bar", "donut", "grouped_bar", "stacked_area")

#let chart-read(spec) = {
  if type(spec) != dictionary { return (ok: false, reason: "chart 不是对象") }
  let kind = as-text(spec.at("type", default: ""))
  if kind not in chart-kinds { return (ok: false, reason: "未知图表类型：" + kind) }
  let rows = spec.at("data", default: none)
  if type(rows) != array or rows.len() == 0 { return (ok: false, reason: "图表数据为空") }
  let encoding = spec.at("encoding", default: none)
  if type(encoding) != dictionary { encoding = (:) }
  let key(name) = as-text(encoding.at(name, default: ""))
  let (name-key, value-keys) = if kind == "donut" {
    (key("label"), (key("value"),))
  } else if kind == "line" or kind == "bar" {
    (key("x"), (key("y"),))
  } else {
    let series = spec.at("series", default: none)
    if type(series) != array or series.len() == 0 { return (ok: false, reason: "缺少 series") }
    if series.any(e => type(e) != dictionary) { return (ok: false, reason: "series 条目不是对象") }
    (key("x"), series.map(e => as-text(e.at("key", default: ""))))
  }
  if name-key == "" or value-keys.any(k => k == "") {
    return (ok: false, reason: "encoding 或 series 缺少列名")
  }
  let names = ()
  let values = ()
  for (i, row) in rows.enumerate() {
    let at-row = "第 " + str(i + 1) + " 行"
    if type(row) != dictionary { return (ok: false, reason: at-row + "不是对象") }
    if name-key not in row { return (ok: false, reason: at-row + "缺少「" + name-key + "」") }
    names.push(as-text(row.at(name-key)))
    let cells = ()
    for k in value-keys {
      let v = as-number(row.at(k, default: none))
      if v == none { return (ok: false, reason: at-row + "的「" + k + "」不是数字") }
      cells.push(v)
    }
    values.push(if kind == "grouped_bar" or kind == "stacked_area" { cells } else { cells.at(0) })
  }
  if kind == "donut" and values.filter(v => v > 0).len() == 0 {
    return (ok: false, reason: "环形图没有大于 0 的数值")
  }
  let emphasis = spec.at("emphasis", default: none)
  let highlight = if type(emphasis) == dictionary { emphasis.at("highlight", default: ()) } else { () }
  let series = spec.at("series", default: none)
  (
    ok: true,
    kind: kind,
    names: names,
    values: values,
    series: if type(series) == array {
      series.filter(e => type(e) == dictionary).map(e => as-text(e.at("name", default: e.at("key", default: ""))))
    } else { () },
    highlight: if type(highlight) == array { highlight.map(as-text) } else { () },
  )
}

// ── Drawing primitives ────────────────────────────────────────────────────────

#let at-px(p) = (p.at(0) * px, p.at(1) * px)
#let put(x, y, body) = place(top + left, dx: x * px, dy: y * px, body)
#let dir(angle) = (calc.cos(angle * 1deg), calc.sin(angle * 1deg))
#let on-circle(cx, cy, r, angle) = (cx + r * dir(angle).at(0), cy + r * dir(angle).at(1))

// Cubic segments of the arc from `a0` to `a1` (degrees, y down, either
// direction), the pen being at the `a0` point. At most 90° per segment.
#let arc-to(cx, cy, r, a0, a1) = {
  let sweep = a1 - a0
  let count = calc.max(1, calc.ceil(calc.abs(sweep) / 90))
  let step = sweep / count
  let k = 4 / 3 * calc.tan(step * 1deg / 4)
  range(count).map(i => {
    let t0 = a0 + i * step
    let t1 = a0 + (i + 1) * step
    let (p0, p3) = (on-circle(cx, cy, r, t0), on-circle(cx, cy, r, t1))
    let (d0, d1) = (dir(t0), dir(t1))
    curve.cubic(
      at-px((p0.at(0) - k * r * d0.at(1), p0.at(1) + k * r * d0.at(0))),
      at-px((p3.at(0) + k * r * d1.at(1), p3.at(1) - k * r * d1.at(0))),
      at-px(p3),
    )
  })
}

// ECharts' `smooth: true` (0.5) control points, including its clamps that keep
// the curve from overshooting a local extreme. Returns `(prev, next)` handles
// per point; the end points get none.
#let smooth-handles(points, smooth: 0.5) = {
  let n = points.len()
  let clamp(v, a, b) = calc.max(calc.min(v, calc.max(a, b)), calc.min(a, b))
  range(n).map(i => {
    let p = points.at(i)
    if i == 0 or i == n - 1 { return (p, p) }
    let (a, b) = (points.at(i - 1), points.at(i + 1))
    let d0 = calc.sqrt(calc.pow(p.at(0) - a.at(0), 2) + calc.pow(p.at(1) - a.at(1), 2))
    let d1 = calc.sqrt(calc.pow(b.at(0) - p.at(0), 2) + calc.pow(b.at(1) - p.at(1), 2))
    if d0 == 0 or d1 == 0 { return (p, p) }
    let ratio = d1 / (d0 + d1)
    let next = range(2).map(c => clamp(p.at(c) + (b.at(c) - a.at(c)) * smooth * ratio, b.at(c), p.at(c)))
    let prev = range(2).map(c => clamp(p.at(c) - (next.at(c) - p.at(c)) * d0 / d1, a.at(c), p.at(c)))
    let next = range(2).map(c => p.at(c) + (p.at(c) - prev.at(c)) * d1 / d0)
    (prev, next)
  })
}

// Curve components through `points`, forwards or (for the underside of a
// stacked area) backwards along the same curve. Starts with a move when `move`.
#let smooth-path(points, reverse: false, move: true) = {
  let handles = smooth-handles(points)
  let order = if reverse { range(points.len()).rev() } else { range(points.len()) }
  let out = if move { (curve.move(at-px(points.at(order.at(0)))),) } else { (curve.line(at-px(points.at(order.at(0)))),) }
  for j in range(1, order.len()) {
    let (from, to) = (order.at(j - 1), order.at(j))
    // Leaving `from` uses its handle on the side of `to`, and vice versa.
    let (c1, c2) = if reverse { (handles.at(from).at(0), handles.at(to).at(1)) } else {
      (handles.at(from).at(1), handles.at(to).at(0))
    }
    out.push(curve.cubic(at-px(c1), at-px(c2), at-px(points.at(to))))
  }
  out
}

// A bar from the zero side `y0` to the value end `y1`, the value end's two
// corners rounded (`borderRadius: [r, r, 0, 0]`; a negative bar rounds its
// bottom, the end away from zero).
#let bar-shape(x, w, y0, y1, r, fill) = {
  let r = calc.max(0, calc.min(r, w / 2, calc.abs(y1 - y0)))
  let down = y1 > y0
  let s = if down { -1 } else { 1 }
  curve(
    fill: fill,
    curve.move(at-px((x, y0))),
    curve.line(at-px((x, y1 + s * r))),
    ..arc-to(x + r, y1 + s * r, r, 180, if down { 90 } else { 270 }),
    curve.line(at-px((x + w - r, y1))),
    ..arc-to(x + w - r, y1 + s * r, r, if down { 90 } else { 270 }, if down { 0 } else { 360 }),
    curve.line(at-px((x + w, y0))),
    curve.close(),
  )
}

// ECharts' `pin` symbol (from the SSR path, in units of half its size) with the
// tip on the data point: a 0.6 circle on a chord at y = -0.89, closed by two
// Béziers down to the tip.
#let pin-shape(x, y, size, fill) = {
  let u = size / 2
  let p(a, b) = at-px((x + a * u, y + b * u))
  let cy = -0.89 - calc.sqrt(0.36 - 0.54 * 0.54)
  let half = calc.atan2(0.54, -0.89 - cy) / 1deg // angle of the chord ends (Typst's atan2 takes x, y)
  curve(
    fill: fill,
    curve.move(p(-0.54, -0.89)),
    ..arc-to(x, y + cy * u, 0.6 * u, 180 - half, 360 + half),
    curve.cubic(p(0.39, -0.56), p(0, -0.42), p(0, 0)),
    curve.cubic(p(0, -0.42), p(-0.39, -0.56), p(-0.54, -0.89)),
    curve.close(),
  )
}

// Ring sector from `start` to `end` (degrees, y down, clockwise) with every
// corner rounded to `r` (`itemStyle.borderRadius: 6`), shrunk for thin sectors.
#let ring-sector(cx, cy, inner, outer, start, end, r, fill) = {
  let sweep = end - start
  if sweep >= 359.99 {
    return curve(
      fill: fill,
      curve.move(at-px(on-circle(cx, cy, outer, -90))),
      ..arc-to(cx, cy, outer, -90, 270),
      curve.close(),
      curve.move(at-px(on-circle(cx, cy, inner, -90))),
      ..arc-to(cx, cy, inner, -90, 270),
      curve.close(),
    )
  }
  let k = if sweep >= 180 { 1 } else { calc.sin(sweep / 2 * 1deg) }
  let r = calc.max(0, calc.min(r, (outer - inner) / 2, outer * k / (1 + k), if k < 1 { inner * k / (1 - k) } else { r }))
  let d-out = if r > 0 { calc.asin(r / (outer - r)) / 1deg } else { 0 }
  let d-in = if r > 0 { calc.asin(r / (inner + r)) / 1deg } else { 0 }
  let (co-s, co-e) = (on-circle(cx, cy, outer - r, start + d-out), on-circle(cx, cy, outer - r, end - d-out))
  let (ci-s, ci-e) = (on-circle(cx, cy, inner + r, start + d-in), on-circle(cx, cy, inner + r, end - d-in))
  curve(
    fill: fill,
    curve.move(at-px(on-circle(cx, cy, outer, start + d-out))),
    ..arc-to(cx, cy, outer, start + d-out, end - d-out),
    ..arc-to(co-e.at(0), co-e.at(1), r, end - d-out, end + 90),
    curve.line(at-px(on-circle(ci-e.at(0), ci-e.at(1), r, end + 90))),
    ..arc-to(ci-e.at(0), ci-e.at(1), r, end + 90, end - d-in + 180),
    ..arc-to(cx, cy, inner, end - d-in, start + d-in),
    ..arc-to(ci-s.at(0), ci-s.at(1), r, start + d-in + 180, start + 270),
    curve.line(at-px(on-circle(co-s.at(0), co-s.at(1), r, start - 90))),
    ..arc-to(co-s.at(0), co-s.at(1), r, start - 90, start + d-out),
    curve.close(),
  )
}

// ── Text ──────────────────────────────────────────────────────────────────────
// Chart text is the body font role, like `--font-body` on screen. ECharts does
// not space CJK from Latin, so neither does this (`1月`, not `1 月`). The
// measuring helpers need context; everything below runs inside `chart-plot`.

#let chart-text(ctx, body, size, fill, weight: "regular") = text(
  font: ctx.fonts.body,
  size: size * px,
  fill: fill,
  weight: weight,
  cjk-latin-spacing: none,
  body,
)

#let text-width(ctx, body, size, weight: "regular") = measure(chart-text(ctx, body, size, black, weight: weight)).width / px

// One line of text whose box (1em tall, see main.typ) is vertically centred on
// `y`, like ECharts' `dominant-baseline: central`, and anchored at `x`.
#let put-text(ctx, body, size, fill, x, y, anchor: "middle", weight: "regular") = {
  let t = chart-text(ctx, body, size, fill, weight: weight)
  let w = measure(t).width / px
  let left = if anchor == "end" { x - w } else if anchor == "start" { x } else { x - w / 2 }
  put(left, y - size / 2, box(width: (w + 1) * px, t))
}

#let series-color(ctx, index) = {
  let palette = ctx.theme.chart.palette
  palette.at(calc.rem(index, palette.len()))
}

// ── Legend ────────────────────────────────────────────────────────────────────
// ECharts' plain legend: fs(12)×fs(8) rounded swatches, 5px to the label,
// fs(18) between items and between wrapped rows, centred on the whole chart
// width and sitting 5px above the bottom edge.

#let legend-layout(ctx, items, w) = {
  let s = ctx.scale
  let (size, swatch, gap) = (12 * s, 12 * s, 18 * s)
  let rows = ()
  let row = ()
  let used = 0
  for item in items {
    let item-w = swatch + 5 + text-width(ctx, item.name, size)
    if row.len() > 0 and used + gap + item-w > w - 10 {
      rows.push((items: row, width: used))
      row = ()
      used = 0
    }
    used += (if row.len() > 0 { gap } else { 0 }) + item-w
    row.push((..item, width: item-w))
  }
  if row.len() > 0 { rows.push((items: row, width: used)) }
  (rows: rows, height: if rows.len() == 0 { 0 } else { rows.len() * size + (rows.len() - 1) * gap })
}

#let draw-legend(ctx, legend, w, h) = {
  let s = ctx.scale
  let (size, swatch, swatch-h, gap) = (12 * s, 12 * s, 8 * s, 18 * s)
  let top = h - 5 - legend.height
  for (r, row) in legend.rows.enumerate() {
    let y = top + r * (size + gap)
    let x = (w - row.width) / 2
    for item in row.items {
      put(x, y + (size - swatch-h) / 2, rect(
        width: swatch * px,
        height: swatch-h * px,
        radius: swatch-h / 4 * px,
        fill: item.color,
      ))
      put-text(ctx, item.name, size, ctx.theme.muted, x + swatch + 5, y + size / 2, anchor: "start")
      x += item.width + gap
    }
  }
}

// ── Cartesian charts ──────────────────────────────────────────────────────────

#let cartesian(ctx, spec, w, h) = {
  let th = ctx.theme
  let s = ctx.scale
  let size = 12 * s
  let n = spec.names.len()
  let multi = spec.kind == "grouped_bar" or spec.kind == "stacked_area"
  let axis-values = if spec.kind == "grouped_bar" {
    spec.values.flatten()
  } else if spec.kind == "stacked_area" {
    spec.values.map(row => row.map(v => calc.max(0, v)).sum())
  } else { spec.values }
  let axis = value-axis(axis-values)
  let tick-text = axis.ticks.map(v => format-number(v, digits: calc.max(2, axis.digits)))
  let legend = if multi {
    legend-layout(ctx, spec.series.enumerate().map(((j, name)) => (name: name, color: series-color(ctx, j))), w)
  } else { none }
  // `grid` from the option builder. The left margin grows when a tick label is
  // wider than it (ECharts 6 keeps axis labels inside the chart); the bottom
  // grows when the legend wraps onto more rows.
  let grid = if spec.kind == "line" {
    (left: 48, right: 40, top: 40, bottom: 40)
  } else if multi { (left: 48, right: 24, top: 36, bottom: 40 * s) } else {
    (left: 48, right: 24, top: 36, bottom: 40)
  }
  let left = calc.max(grid.left, tick-text.map(t => text-width(ctx, t, size)).fold(0, calc.max) + 10)
  let bottom = if legend == none { grid.bottom } else {
    calc.max(grid.bottom, 8 + size + 8 + legend.height + 5)
  }
  let plot = (x: left, y: grid.top, w: calc.max(1, w - left - grid.right), h: calc.max(1, h - grid.top - bottom))
  let y-of(v) = plot.y + (axis.max - v) / (axis.max - axis.min) * plot.h
  let zero = y-of(calc.min(calc.max(0, axis.min), axis.max))
  // `boundaryGap: false` puts line points on the ticks; everything else is centred in its band.
  let band = if spec.kind == "line" { plot.w / calc.max(1, n - 1) } else { plot.w / n }
  let x-of(i) = if spec.kind != "line" { plot.x + (i + 0.5) * band } else if n == 1 {
    plot.x + plot.w / 2
  } else { plot.x + i * band }

  // Split lines, then the category axis line on the zero value (`onZero`).
  for v in axis.ticks {
    put(plot.x, y-of(v), line(length: plot.w * px, stroke: 1 * px + th.chart.grid))
  }
  put(plot.x, zero, line(length: plot.w * px, stroke: 1 * px + th.chart.axis))

  // Plain value labels are dropped when neighbours would touch (ECharts would
  // print them on top of each other); highlight pins always stay.
  let value-labels = if multi { () } else { spec.values.map(v => format-number(v)) }
  let show-labels = (
    not multi and value-labels.map(t => text-width(ctx, t, size)).fold(0, calc.max) + 4 <= (if n > 1 { band } else { plot.w })
  )
  let hits = range(n).filter(i => spec.highlight.contains(spec.names.at(i)))

  if spec.kind == "line" {
    let points = range(n).map(i => (x-of(i), y-of(spec.values.at(i))))
    if n > 1 {
      // Area to the zero line: 20% → 2% primary, times ECharts' default area
      // opacity 0.7. The curve never overshoots its points (see smooth-handles),
      // so the points bound the gradient box; it is drawn shifted to the box top.
      let top = calc.min(zero, ..points.map(p => p.at(1)))
      let shifted = points.map(p => (p.at(0), p.at(1) - top))
      put(0, top, curve(
        fill: gradient.linear(
          (th.primary.transparentize(86%), 0%),
          (th.primary.transparentize(98.6%), 100%),
          angle: 90deg,
        ),
        ..smooth-path(shifted),
        curve.line(at-px((shifted.last().at(0), zero - top))),
        curve.line(at-px((shifted.first().at(0), zero - top))),
        curve.close(),
      ))
      put(0, 0, curve(stroke: (paint: th.primary, thickness: 3 * px, join: "round"), ..smooth-path(points)))
    }
    for (i, p) in points.enumerate() {
      put(p.at(0) - 4, p.at(1) - 4, circle(radius: 4 * px, fill: th.primary))
      // Label on top: 4px symbol radius + 5px distance.
      if show-labels and i not in hits {
        put-text(ctx, value-labels.at(i), size, th.muted, p.at(0), p.at(1) - 9 - size / 2)
      }
    }
    // Highlighted points: the success pin with the value inside (white, fs(11)).
    for i in hits {
      let p = points.at(i)
      put(0, 0, pin-shape(p.at(0), p.at(1), 46 * s, th.chart.success))
      put-text(ctx, value-labels.at(i), 11 * s, white, p.at(0), p.at(1) - 1.046 * 23 * s)
    }
  } else if spec.kind == "bar" {
    let bar-w = band * 0.46
    for (i, v) in spec.values.enumerate() {
      let x = x-of(i)
      let fill = if hits.len() == 0 or i in hits { th.primary } else { th.primary.transparentize(65%) }
      if v != 0 { put(0, 0, bar-shape(x - bar-w / 2, bar-w, zero, y-of(v), 6, fill)) }
      if show-labels {
        let y = if v >= 0 { y-of(v) - 5 - size / 2 } else { y-of(v) + 5 + size / 2 }
        put-text(ctx, value-labels.at(i), size, th.muted, x, y)
      }
    }
  } else if spec.kind == "grouped_bar" {
    // `barCategoryGap: 32%` of the band, `barGap: 12%` of a bar between bars.
    let count = spec.series.len()
    let bar-w = band * 0.68 / (count + 0.12 * (count - 1))
    for (i, row) in spec.values.enumerate() {
      for (j, v) in row.enumerate() {
        let x = plot.x + i * band + band * 0.16 + j * bar-w * 1.12
        if v != 0 { put(0, 0, bar-shape(x, bar-w, zero, y-of(v), 4, series-color(ctx, j))) }
      }
    }
  } else {
    // Stacked areas, bottom-up: each fill runs along its top curve and back along
    // the previous layer's curve; areas first, then the 2px lines over them.
    // Fill: the series colour at 45%, times the 0.7 area opacity. Negative
    // values count as 0: a stacked negative has no readable form on a slide.
    let lower = range(n).map(_ => 0.0)
    let layers = ()
    for (j, _) in spec.series.enumerate() {
      let upper = range(n).map(i => lower.at(i) + calc.max(0, spec.values.at(i).at(j)))
      layers.push((
        top: range(n).map(i => (x-of(i), y-of(upper.at(i)))),
        bottom: range(n).map(i => (x-of(i), y-of(lower.at(i)))),
        color: series-color(ctx, j),
      ))
      lower = upper
    }
    for layer in layers {
      put(0, 0, curve(
        fill: layer.color.transparentize(68.5%),
        ..smooth-path(layer.top),
        ..smooth-path(layer.bottom, reverse: true, move: false),
        curve.close(),
      ))
    }
    for layer in layers {
      put(0, 0, curve(stroke: (paint: layer.color, thickness: 2 * px, join: "round"), ..smooth-path(layer.top)))
    }
  }

  // Axis labels: values right-aligned 8px left of the plot, categories 8px
  // below it. Crowded categories show every k-th label, never rotated or shrunk.
  for (v, t) in axis.ticks.zip(tick-text) {
    put-text(ctx, t, size, th.muted, plot.x - 8, y-of(v), anchor: "end")
  }
  let widest = spec.names.map(t => text-width(ctx, t, size)).fold(0, calc.max)
  let every = calc.max(1, calc.ceil((widest + 4) / calc.max(band, 1)))
  for i in range(n) {
    if calc.rem(i, every) == 0 {
      put-text(ctx, spec.names.at(i), size, th.muted, x-of(i), plot.y + plot.h + 8 + size / 2)
    }
  }
  if legend != none { draw-legend(ctx, legend, w, h) }
}

// ── Donut ─────────────────────────────────────────────────────────────────────
// Radii 48% / 66% of half the shorter side, centre at (50%, 46%). Sectors run
// clockwise from 12 o'clock (zero and negative values get no sector, label or
// legend item); `highlight[0]` takes the primary colour. Outside labels
// `name / percent` on fs(16) lines, at most fs(130) wide, joined by a 14 + 10px
// leader; each side is pushed apart so labels never overlap.

// Percentages by largest remainder to two decimals: they always add up to 100.
#let percent-labels(values) = {
  let total = values.sum()
  let exact = values.map(v => v * 100 / total)
  let floors = exact.map(v => calc.floor(v * 100) / 100)
  let missing = int(calc.round((100 - floors.sum()) * 100))
  let order = range(values.len()).sorted(key: i => -(exact.at(i) - floors.at(i)))
  for i in order.slice(0, calc.min(calc.max(0, missing), order.len())) {
    floors.at(i) = floors.at(i) + 0.01
  }
  floors
}

#let donut(ctx, spec, w, h) = {
  let th = ctx.theme
  let s = ctx.scale
  let size = 12 * s
  let sectors = range(spec.names.len()).filter(i => spec.values.at(i) > 0)
  let values = sectors.map(i => spec.values.at(i))
  let total = values.sum()
  let percents = percent-labels(values)
  let colors = sectors.map(i => {
    if spec.highlight.len() > 0 and spec.names.at(i) == spec.highlight.at(0) { th.primary } else { series-color(ctx, i) }
  })
  let legend = legend-layout(ctx, sectors.enumerate().map(((k, i)) => (name: spec.names.at(i), color: colors.at(k))), w)
  let (cx, cy) = (w / 2, h * 0.46)
  let half = calc.min(w, h) / 2
  let (inner, outer) = (half * 0.48, half * 0.66)

  // ECharts separates sectors with a 2px stroke in the page colour. Themes have
  // no opaque page token (glass cards are translucent), so the gap is cut out
  // instead: 1px of arc at the mid radius off each side, showing the card.
  let pad = if values.len() > 1 { 1 / ((inner + outer) / 2) * 180 / calc.pi } else { 0 }
  let angle = -90.0
  let labels = ()
  for (k, v) in values.enumerate() {
    let sweep = v / total * 360
    let cut = calc.min(pad, sweep / 4)
    put(0, 0, ring-sector(cx, cy, inner, outer, angle + cut, angle + sweep - cut, 6, colors.at(k)))
    let mid = angle + sweep / 2
    let surface = on-circle(cx, cy, outer, mid)
    let elbow = on-circle(cx, cy, outer + 14, mid)
    let side = if dir(mid).at(0) >= 0 { 1 } else { -1 }
    let name = spec.names.at(sectors.at(k))
    let body = chart-text(ctx, name, size, th.muted) + linebreak() + chart-text(ctx, format-number(percents.at(k)) + "%", size, th.muted)
    let natural = calc.max(text-width(ctx, name, size), text-width(ctx, format-number(percents.at(k)) + "%", size))
    let label-w = calc.min(natural, 130 * s) + 1
    // The spacing setting has to be on the paragraph: per-run it is ignored once
    // runs are joined, and the extra `4 月` space would wrap the measured box.
    let label = line-box(size * px, 16 / 12, {
      set text(cjk-latin-spacing: none)
      align(if side > 0 { left } else { right }, body)
    }, width: label-w * px)
    labels.push((
      surface: surface,
      elbow: elbow,
      side: side,
      y: elbow.at(1),
      label: label,
      w: label-w,
      h: measure(label).height / px,
    ))
    angle += sweep
  }

  // Push each side's labels apart, top to bottom, then back up from the legend.
  let floor-y = h - 5 - legend.height - 4
  let placed = ()
  for side in (-1, 1) {
    let group = labels.filter(l => l.side == side).sorted(key: l => l.y)
    for k in range(1, group.len()) {
      let min-y = group.at(k - 1).y + (group.at(k - 1).h + group.at(k).h) / 2 + 2
      if group.at(k).y < min-y { group.at(k).y = min-y }
    }
    for k in range(group.len()).rev() {
      let max-y = if k == group.len() - 1 { floor-y - group.at(k).h / 2 } else {
        group.at(k + 1).y - (group.at(k).h + group.at(k + 1).h) / 2 - 2
      }
      if group.at(k).y > max-y { group.at(k).y = max-y }
    }
    for k in range(group.len()) {
      if group.at(k).y < group.at(k).h / 2 { group.at(k).y = group.at(k).h / 2 }
    }
    placed += group
  }
  // A label pushed up or down keeps clear of the ring: like ECharts, its leader
  // ends on the circle of radius `outer + 14` at the new height, plus 10px.
  for l in placed {
    let dy = l.y - cy
    let reach = outer + 14
    let ring-x = if calc.abs(dy) < reach { cx + l.side * calc.sqrt(reach * reach - dy * dy) } else { cx }
    let end-x = if l.side > 0 { calc.max(l.elbow.at(0), ring-x) + 10 } else { calc.min(l.elbow.at(0), ring-x) - 10 }
    put(0, 0, curve(
      stroke: 1 * px + th.chart.axis,
      curve.move(at-px(l.surface)),
      curve.line(at-px(l.elbow)),
      curve.line(at-px((end-x, l.y))),
    ))
    let x = end-x + l.side * 5
    put(if l.side > 0 { x } else { x - l.w }, l.y - l.h / 2, l.label)
  }

  // Centre: the total (fs(26) bold, ink) over「合计」(fs(12), 6px padding).
  let (big, small) = (26 * s, 12 * s)
  let top = cy - (big + small + 6) / 2
  put-text(ctx, format-number(total), big, th.ink, cx, top + big / 2, weight: "bold")
  put-text(ctx, "合计", small, th.muted, cx, top + big + 6 + small / 2)
  draw-legend(ctx, legend, w, h)
}

// ── Entry point ───────────────────────────────────────────────────────────────
// `spec` is a `chart-read` result with `ok: true`; `width` / `height` are the plot
// box. The result is exactly that size and clips whatever reaches past it
// (e.g. a pin on a point near the top).
#let chart-plot(ctx, spec, width, height) = context {
  let (w, h) = (calc.max(1, width / px), calc.max(1, height / px))
  block(width: w * px, height: h * px, clip: true, {
    if spec.kind == "donut" { donut(ctx, spec, w, h) } else { cartesian(ctx, spec, w, h) }
  })
}
