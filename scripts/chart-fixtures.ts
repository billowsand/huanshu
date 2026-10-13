/**
 * Renders chart_focus SVGs offline with the app's own option builder, the same way the PDF
 * export does in the webview (src/composables/usePdfExport.ts), for Typst fixtures and visual
 * checks without running the app.
 *
 *   bun scripts/chart-fixtures.ts <deck.json> <chart-boxes.json> <chart-theme.json> <out-dir>
 *
 * - chart-boxes.json: `cargo run -p slide-typst --example render_fixture -- <deck.json> <dir> --chart-boxes`
 *   (the exact plot boxes Typst lays out, same as the `pdf_chart_boxes` command).
 * - chart-theme.json: a `ChartTheme` read from the real CSS cascade, e.g. in the Vue preview page
 *   (`bunx vite`, /experiments/typst/preview/index.html?theme=<id>) run
 *   `(await import('/src/components/chartOptions.ts')).themeFromCssVars(getComputedStyle(document.documentElement))`.
 *   Ready-made ones live in crates/slide-typst/tests/fixtures/charts/themes/.
 *
 * Writes `<out-dir>/chart-<index>.svg` (raw ECharts SSR output, unsanitized, exactly what the
 * app sends) and `<out-dir>/assets.json` (`chart:<index>` → file), which render_fixture takes
 * via `--assets`.
 */
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import type { ChartBox, ChartSpec, ChartTheme } from '../src/components/chartOptions'
import { renderChartSvg } from '../src/components/chartOptions'

const [deckPath, boxesPath, themePath, outDir] = process.argv.slice(2)
if (!outDir) {
  console.error('usage: bun scripts/chart-fixtures.ts <deck.json> <chart-boxes.json> <chart-theme.json> <out-dir>')
  process.exit(2)
}

const read = <T>(path: string): T => JSON.parse(readFileSync(path, 'utf8')) as T
const deck = read<Array<{ kind: string, chart?: ChartSpec }>>(deckPath)
const boxes = read<ChartBox[]>(boxesPath)
const theme = read<ChartTheme>(themePath)

mkdirSync(outDir, { recursive: true })
const manifest: Record<string, string> = {}
for (const box of boxes) {
  const chart = deck[box.slide]?.chart
  if (!chart) continue
  const file = `chart-${box.slide}.svg`
  writeFileSync(join(outDir, file), renderChartSvg(chart, theme, box))
  manifest[`chart:${box.slide}`] = file
}
writeFileSync(join(outDir, 'assets.json'), `${JSON.stringify(manifest, null, 2)}\n`)
console.log(`${Object.keys(manifest).length} chart SVGs → ${outDir}`)
