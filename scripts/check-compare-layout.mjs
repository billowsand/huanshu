// Exercise actual wrapping, shared row geometry and content edits in Chromium.
import { createServer } from 'vite'
import { createRequire } from 'node:module'
import { readFile, mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import assert from 'node:assert/strict'
const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright')
// Use the same renderer, themes and editor styles as the general layout checks.
const source = await readFile(new URL('./check-slide-layout.mjs', import.meta.url), 'utf8')
const entry = source.match(/const entry = `([\s\S]*?)`/)[1]
const server = await createServer({
  cacheDir: join(tmpdir(), 'huanshu-compare-vite-cache'),
  optimizeDeps: { entries: [], include: ['vue'], noDiscovery: true },
  server: { port: 5180, host: '127.0.0.1', hmr: false },
  plugins: [{ name: 'compare-check', resolveId(id) { if (id === '/__compare-entry.js') return id },
    load(id) { if (id === '/__compare-entry.js') return entry },
    configureServer(vite) { vite.middlewares.use('/__compare-check', async (_req, res) => {
      res.setHeader('Content-Type', 'text/html')
      res.end(await vite.transformIndexHtml('/__compare-check', '<div id="app" style="width:1280px;height:720px"></div><script type="module" src="/__compare-entry.js"></script>'))
    }) },
  }],
})
const example = {
  kind: 'compare', section: '01', title: '现状对比',
  compare_data: { mode: 'side-by-side',
    left: { title: '东非坦桑尼亚', tone: 'blue', icon: 'i-carbon:earth-southeast-asia', conclusion: '区域局势总体稳定', items: [
      { label: '地理反差', desc: '呈现祥和旅游景象，国际游客往来频繁。' },
      { label: '冲突量级', desc: '相对平稳，未形成大规模卷入性军事冲突。' },
    ] },
    right: { title: '中东阿拉伯半岛', tone: 'amber', icon: 'i-carbon:warning', conclusion: '地缘动荡重塑安全格局', items: [
      { label: '地理反差', desc: '距离坦桑尼亚 2000 多公里，处于高度紧张状态。' },
      { label: '冲突量级', desc: '复杂程度及卷入国家数量已攀升至与中东不相上下的水平。', highlight: true },
    ] },
  }, footer: '东非安全局势已从孤立事件演变为与中东深度联动的复杂新常态。',
}
let browser
try {
  await server.listen()
  browser = await chromium.launch({ headless: true, channel: process.env.LAYOUT_BROWSER_CHANNEL || undefined })
  const page = await browser.newPage({ viewport: { width: 3840, height: 760 }, reducedMotion: 'reduce' })
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto(server.resolvedUrls.local[0] + '__compare-check')
  await page.waitForFunction(() => !!window.renderSlide)
  let checked = 0
  async function check(slide, theme) {
    await page.evaluate(([s, t]) => window.renderSlide(s, t), [slide, theme])
    const result = await page.evaluate(() => {
      const frame = document.querySelector('.slide-shell__frame').getBoundingClientRect()
      const body = document.querySelector('.cp-body')
      const rows = [...document.querySelectorAll('.cp-row')]
      const all = [...document.querySelectorAll('.cp-header,.cp-label,.cp-desc,.cp-footer,.compare-footnote')]
      return {
        pairsAligned: rows.every(row => {
          const cells = [...row.querySelectorAll('.cp-item')].map(el => el.getBoundingClientRect())
          const labels = [...row.querySelectorAll('.cp-label')].map(el => el.getBoundingClientRect())
          return Math.abs(cells[0].top - cells[1].top) < 1 && Math.abs(cells[0].bottom - cells[1].bottom) < 1
            && (labels.length < 2 || Math.abs(labels[0].top - labels[1].top) < 1)
        }),
        bodyFits: body.scrollHeight <= body.clientHeight + 1,
        geometry: [body.clientHeight, body.scrollHeight, ...rows.map(el => el.getBoundingClientRect().height)],
        copyFits: all.every(el => { const r = el.getBoundingClientRect(); return r.bottom <= frame.bottom - 20
          && r.right <= frame.right && el.scrollWidth <= el.clientWidth + 1 }),
        overflow: all.map(el => { const r = el.getBoundingClientRect(); return {class: el.className, bottom:r.bottom, right:r.right, width:el.clientWidth, scroll:el.scrollWidth, frameBottom:frame.bottom} }).filter(r => r.bottom > frame.bottom - 20 || r.scroll > r.width + 1),
        sizes: [...document.querySelectorAll('.cp-desc')].map(el => parseFloat(getComputedStyle(el).fontSize)),
        footersAligned: (() => { const cells = [...document.querySelectorAll('.cp-footer')].map(el => el.getBoundingClientRect());
          return cells.length < 2 || (Math.abs(cells[0].top - cells[1].top) < 1 && Math.abs(cells[0].bottom - cells[1].bottom) < 1) })(),
      }
    })
    if (!result.bodyFits || !result.copyFits) {
      await mkdir(new URL('../output/compare-layout/', import.meta.url), { recursive: true })
      await page.locator('#app').screenshot({ path: new URL('../output/compare-layout/failure.png', import.meta.url).pathname.replace(/^\/(\w:)/, '$1') })
    }
    assert.ok(result.pairsAligned && result.footersAligned, JSON.stringify({ theme, result, slide }))
    assert.ok(result.bodyFits && result.copyFits, JSON.stringify({ theme, result, slide }))
    assert.ok(result.sizes.every(size => size >= 15.9), JSON.stringify(result))
    checked++
  }
  for (const count of [2, 3, 4, 6]) {
    for (const aspect of ['ratio_16x9', 'ratio_32x9', 'ratio_48x9']) {
      for (const theme of ['light-notes', 'light-terminal', 'dark-blue']) {
        for (const mode of ['side-by-side', 'pros-cons', 'versus']) {
          const slide = structuredClone(example)
          slide.aspect_ratio = aspect
          slide.compare_data.mode = mode
          for (const side of ['left', 'right']) slide.compare_data[side].items = Array.from({ length: count }, (_, i) => ({
            label: '关联维度 ' + (i + 1), highlight: i === 1,
            desc: side === 'left' ? '简短说明。' : count > 4 ? '多条目采用简练描述，保留关联内容的顺序。' : '这里保留更长的说明，以验证左右换行不同的情况下，下一组关联内容仍然对齐，不发生错位。',
          }))
          await check(slide, theme)
        }
      }
    }
  }
  const unequal = structuredClone(example)
  unequal.compare_data.right.items.push({ label: '额外维度', desc: '单侧条目保留原有顺序。' })
  unequal.compare_data.left.conclusion = ''
  unequal.subtitle = '左右条目数量不同'
  await check(unequal, 'light-notes')
  assert.equal(await page.locator('.cp-item--empty').count(), 1)
  const duplicate = structuredClone(example)
  duplicate.footer = duplicate.compare_data.left.conclusion
  await check(duplicate, 'light-notes')
  assert.equal(await page.locator('.compare-footnote').count(), 0)
  const cases = structuredClone(example)
  cases.title = '具体冲突案例与外部支持链'
  cases.section = '04'
  cases.compare_data.left.title = '苏丹内战：代理人战争格局'
  cases.compare_data.left.items = [
    { label: '外部资助方', desc: '阿联酋全力资助叛军快速支援部队，沙特和埃及援助政府军。' },
    { label: '战场态势', desc: '政府军在外部输血下夺回主动权，拒绝停火建议。', highlight: true },
    { label: '人道后果', desc: '平民伤亡与饥荒加剧，冲突持续升级。' },
  ]
  cases.compare_data.right.title = '索马里与埃塞局势：地缘博弈'
  cases.compare_data.right.items = [
    { label: '索马里兰独立', desc: '以色列公开承认，获阿联酋和埃塞俄比亚默许。' },
    { label: '联邦政府支持', desc: '获得沙特、土耳其和埃及的军事与政治支持。', highlight: true },
    { label: '埃塞俄比亚局势', desc: '阿比总理指控提格雷叛乱背后有苏丹和厄立特里亚影子，试图武力解决。' },
  ]
  cases.compare_data.left.conclusion = '外部势力深度介入将局部内战转化为代理人战争，并通过资源贸易链条为冲突提供持续燃料。'
  cases.compare_data.right.conclusion = '地区大国通过承认分裂政权或支持特定武装力量，重塑了东非的地缘政治版图。'
  cases.footer = cases.compare_data.left.conclusion
  await check(cases, 'light-notes')
  await mkdir(new URL('../output/compare-layout/', import.meta.url), { recursive: true })
  await page.locator('#app').screenshot({ path: new URL('../output/compare-layout/cases.png', import.meta.url).pathname.replace(/^\/(\w:)/, '$1') })
  await check(example, 'light-notes')
  assert.ok(await page.locator('.cp-desc').first().evaluate(el => parseFloat(getComputedStyle(el).fontSize)) > 24, 'Sparse content restores larger text')
  await mkdir(new URL('../output/compare-layout/', import.meta.url), { recursive: true })
  await page.locator('#app').screenshot({ path: new URL('../output/compare-layout/after.png', import.meta.url).pathname.replace(/^\/(\w:)/, '$1') })
  assert.deepEqual(errors, [])
  console.log(`PASS: ${checked} comparison cases; paired rows, wrapping, conclusions, unequal lists, duplicate footer and fitting reset.`)
} finally {
  await browser?.close()
  await server.close()
}
