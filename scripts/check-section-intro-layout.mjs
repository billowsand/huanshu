// Check real card geometry for each approved count and keep all preview cards visible.
import { createServer } from 'vite'
import { createRequire } from 'node:module'
import { readFile, mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import assert from 'node:assert/strict'
const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright')
const source = await readFile(new URL('./check-slide-layout.mjs', import.meta.url), 'utf8')
const entry = source.match(/const entry = `([\s\S]*?)`/)[1]
const server = await createServer({
  cacheDir: join(tmpdir(), 'huanshu-section-intro-cache'),
  optimizeDeps: { entries: [], include: ['vue'], noDiscovery: true },
  server: { port: 5184, host: '127.0.0.1', hmr: false },
  plugins: [{ name: 'section-intro-check',
    resolveId(id) { if (id === '/__intro-entry.js') return id },
    load(id) { if (id === '/__intro-entry.js') return entry },
    configureServer(vite) { vite.middlewares.use('/__intro-check', async (_req, res) => {
      res.setHeader('Content-Type', 'text/html')
      res.end(await vite.transformIndexHtml('/__intro-check', '<div id="app" style="width:1280px;height:720px"></div><script type="module" src="/__intro-entry.js"></script>'))
    }) },
  }],
})
const cards = [
  { title: '现状对比', tone: 'green', icon: 'i-carbon:roadmap', body: '从“非洲之角”作为难民接收地转变为区域军事冲突中心' },
  { title: '东非主要安全爆点', tone: 'red', icon: 'i-carbon:warning', body: '索马里青年党、埃塞俄比亚提格雷战争及苏丹内战的外溢效应' },
  { title: '媒体关注差异', tone: 'blue', icon: 'i-carbon:document', body: '西方聚焦人道主义危机，而地缘战略界更关注红海航道与能源通道安全' },
  { title: '联动机制', tone: 'amber', icon: 'i-carbon:building', body: '中东代理人战争模式向非洲之角的渗透与本土化演变' },
  { title: '区域影响', tone: 'red', icon: 'i-carbon:chart-column', body: '地区竞争加剧了东非国家的政治分化与安全不稳定' },
  { title: '未来展望', tone: 'green', icon: 'i-carbon:chart-line', body: '东非安全局势将呈现长期复杂化与多重变量交织的态势' },
]
const output = new URL('../output/section-intro/', import.meta.url)
await mkdir(output, { recursive: true })
let browser
try {
  await server.listen()
  browser = await chromium.launch({ headless: true, channel: process.env.LAYOUT_BROWSER_CHANNEL || undefined })
  const page = await browser.newPage({ viewport: { width: 3840, height: 760 }, reducedMotion: 'reduce' })
  const errors = []
  page.on('pageerror', error => errors.push(error.message))
  await page.goto(server.resolvedUrls.local[0] + '__intro-check')
  await page.waitForFunction(() => !!window.renderSlide)
  let checked = 0
  for (const count of [2, 3, 4, 5, 6]) {
    for (const aspect of ['ratio_16x9', 'ratio_32x9', 'ratio_48x9']) {
      for (const theme of ['light-notes', 'light-terminal', 'dark-blue']) {
        const slide = { kind: 'section_intro', badge: '章节导览', title: '引言：东非安全局势的新常态', aspect_ratio: aspect, cards: cards.slice(0, count) }
        await page.evaluate(([s, t]) => window.renderSlide(s, t), [slide, theme])
        const result = await page.evaluate(() => {
          const frame = document.querySelector('.slide-shell__frame').getBoundingClientRect()
          const rects = [...document.querySelectorAll('.section-intro__card')].map(card => {
            const r = card.getBoundingClientRect()
            const title = card.querySelector('.section-intro__card-title').getBoundingClientRect()
            const icon = card.querySelector('.section-intro__card-icon').getBoundingClientRect()
            const copy = [...card.querySelectorAll('.section-intro__card-title,.section-intro__card-body,.section-intro__card-item')]
            return { x: r.x, y: r.y, w: r.width, h: r.height,
              fits: r.bottom <= frame.bottom - 20 && copy.every(el => {
                const b = el.getBoundingClientRect()
                return b.bottom <= r.bottom - 10 && b.right <= r.right - 10 && el.scrollWidth <= el.clientWidth + 1 && el.scrollHeight <= el.clientHeight + 1
              }),
              separated: title.right <= icon.left || title.top >= icon.bottom - 1,
              iconSize: icon.width,
            }
          })
          const lead = document.querySelector('.section-intro__lead')
          const leadBox = lead.getBoundingClientRect()
          const leadFits = [...lead.querySelectorAll('.section-intro__title,.section-intro__subtitle,.section-intro__note')].every(el => {
            const r = el.getBoundingClientRect()
            return r.right <= leadBox.right - 10 && r.bottom <= leadBox.bottom - 10 && el.scrollWidth <= el.clientWidth + 1
          })
          return { rects, leadFits, badge: getComputedStyle(document.querySelector('.section-intro__badge')).color }
        })
        if (theme === 'light-notes' && aspect === 'ratio_16x9') {
          await page.locator('#app').screenshot({ path: new URL(`cards-${count}.png`, output).pathname.replace(/^\/(\w:)/, '$1') })
          assert.equal(result.badge, 'rgb(104, 80, 60)')
          const rows = Map.groupBy(result.rects, r => Math.round(r.y))
          assert.deepEqual([...rows.values()].map(row => row.length), { 2: [1, 1], 3: [1, 1, 1], 4: [2, 2], 5: [2, 3], 6: [3, 3] }[count])
        }
        assert.equal(result.rects.length, count)
        assert.ok(result.leadFits, JSON.stringify({ count, aspect, theme, result }))
        assert.ok(result.rects.every(r => r.fits && r.separated && r.iconSize >= 38), JSON.stringify({ count, aspect, theme, result }))
        checked++
      }
    }
  }
  assert.deepEqual(errors, [])
  console.log(`Passed ${checked} section intro layouts; previews in output/section-intro`)
} finally {
  await browser?.close()
  await server.close()
}
