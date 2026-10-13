// Reference harness: renders fixtures/all-layout-blueprints.json with the real
// Vue templates so Typst output can be compared side by side.
// Open http://localhost:5173/experiments/typst/preview/index.html?theme=light-notes&kind=feature_grid
import { createApp, h } from 'vue'
// Component styles must be injected before the theme sheets, as in src/main.ts,
// otherwise scoped .slide-shell backgrounds beat the theme overrides.
import SlideRenderer from '../../../src/components/SlideRenderer.vue'
import 'virtual:uno.css'
import '@unocss/reset/tailwind.css'
import '../../../src/style.css'
import '../../../src/themes/terminal.css'
import '../../../src/themes/notes.css'
import '../../../src/themes/notes-layouts.css'
import '../../../src/themes/notes-studio.css'
import '../../../src/themes/editorial.css'
import '../../../src/themes/typography.css'
import { applyFonts, normalizeFonts } from '../../../src/typography'
import deck from '../../../fixtures/all-layout-blueprints.json'
// Registered globally in src/main.ts and used by templates without importing;
// without these the templates render them as unknown elements (no card or
// pill styling), so measurements here would not match the app.
import GlassCard from '../../../src/components/GlassCard.vue'
import TagBadge from '../../../src/components/TagBadge.vue'
import CardGrid from '../../../src/components/CardGrid.vue'
import ImageCarousel from '../../../src/components/ImageCarousel.vue'
import StepNumber from '../../../src/components/StepNumber.vue'

const params = new URLSearchParams(location.search)
document.documentElement.setAttribute('data-theme', params.get('theme') ?? 'light-notes')
applyFonts(normalizeFonts(null))
const kind = params.get('kind')

// Optional `?deck=/path.json&assets=/path/assets.json`: image refs listed in the
// asset manifest (same file the Rust example takes) are rewritten to URLs.
async function loadDeck(): Promise<any[]> {
  const deckUrl = params.get('deck')
  if (!deckUrl) return deck as any[]
  const slides = await (await fetch(deckUrl)).json()
  const assetsUrl = params.get('assets')
  if (!assetsUrl) return slides
  const manifest: Record<string, string> = await (await fetch(assetsUrl)).json()
  // Absolute http URLs: SlideRenderer passes them through, while `/path` would
  // go to Tauri's convertFileSrc, which does not exist in a plain browser.
  const base = new URL('.', new URL(assetsUrl, location.href)).href
  const url = (ref: string) => (manifest[ref] ? base + manifest[ref] : ref)
  return slides.map((s: any) => ({ ...s, image: s.image && url(s.image), images: s.images?.map(url) }))
}
const slides = (await loadDeck()).filter(s => !kind || s.kind === kind)

createApp({
  render: () => h('div', { style: 'display:flex;flex-direction:column;gap:24px;padding:24px' },
    slides.map((slide, i) => h('div', {
      class: 'slidev-slide',
      'data-kind': slide.kind,
      style: 'width:1280px;height:720px;position:relative;overflow:hidden;flex:none',
    }, [h(SlideRenderer, { slide, slideIndex: i })]))),
})
  .component('GlassCard', GlassCard)
  .component('TagBadge', TagBadge)
  .component('CardGrid', CardGrid)
  .component('ImageCarousel', ImageCarousel)
  .component('StepNumber', StepNumber)
  .mount('#app')

// Calibration helper for the Typst port: geometry + computed styles relative
// to the first slide, e.g. measure(['.slide-title', '.sf-hnode']).
;(window as any).measure = (selectors: string[], limit = 2) => {
  const root = document.querySelector('.slidev-slide')!.getBoundingClientRect()
  return selectors.flatMap(sel => [...document.querySelectorAll<HTMLElement>(sel)].slice(0, limit).map((el) => {
    const r = el.getBoundingClientRect()
    const c = getComputedStyle(el)
    return [sel, Math.round(r.x - root.x), Math.round(r.y - root.y), Math.round(r.width), Math.round(r.height),
      `fs=${c.fontSize} lh=${c.lineHeight} color=${c.color} bg=${c.backgroundColor} border=${c.borderTopWidth} ${c.borderTopStyle} ${c.borderTopColor} rad=${c.borderRadius} pad=${c.padding} op=${c.opacity}`].join(' | ')
  }))
}
