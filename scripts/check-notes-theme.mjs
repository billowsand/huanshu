// Real-browser theme checks. Tauri data is mocked; no real projects are changed.
// PLAYWRIGHT_MODULE_PATH may point to the desktop app's bundled playwright.
import { createServer } from 'vite'
import { createRequire } from 'node:module'
import { readFile, mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { tmpdir } from 'node:os'
import assert from 'node:assert/strict'

const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright')
const fixtures = JSON.parse(await readFile(new URL('../fixtures/all-layout-blueprints.json', import.meta.url), 'utf8'))
fixtures[0] = { kind: 'cover', title: '把想法，写成一页好稿。', subtitle: '从灵感便签到完整演示', badges: ['便签 · 稿纸'] }
const output = new URL('../output/notes-theme/', import.meta.url)
await mkdir(output, { recursive: true })
const entry = `
import {createApp,h,shallowRef,nextTick} from 'vue';
import SlideRenderer from '/src/components/SlideRenderer.vue';
import GlassCard from '/src/components/GlassCard.vue';
import CardGrid from '/src/components/CardGrid.vue';
import TagBadge from '/src/components/TagBadge.vue';
import StepNumber from '/src/components/StepNumber.vue';
import 'virtual:uno.css';
import '/node_modules/@unocss/reset/tailwind.css';
import '/src/style.css';
import '/src/themes/terminal.css';
import '/src/themes/notes.css';
import '/src/themes/typography.css';
document.documentElement.dataset.theme='light-notes';
const slide=shallowRef({kind:'cover',title:'便签·稿纸'});
createApp({render:()=>h(SlideRenderer,{slide:slide.value})})
 .component('GlassCard',GlassCard).component('CardGrid',CardGrid)
 .component('TagBadge',TagBadge).component('StepNumber',StepNumber).mount('#app');
window.renderSlide=async(value)=>{
 const width={ratio_16x9:1280,ratio_32x9:2560,ratio_48x9:3840}[value.aspect_ratio||'ratio_16x9'];
 document.querySelector('#app').style.width=width+'px';
 slide.value=value;await nextTick();await document.fonts.ready;
};`
const server = await createServer({
  cacheDir: join(tmpdir(), 'huanshu-notes-vite-cache'),
  optimizeDeps: { entries: [], include: ['vue'], noDiscovery: true },
  server: { host: '127.0.0.1', port: 5182, strictPort: false, hmr: false },
  plugins: [{ name: 'notes-preview',
    resolveId(id) { if (id === '/__notes-entry.js') return id },
    load(id) { if (id === '/__notes-entry.js') return entry },
    configureServer(vite) {
      vite.middlewares.use('/__notes-slides', async (_req, res) => {
        res.setHeader('Content-Type', 'text/html')
        res.end(await vite.transformIndexHtml('/__notes-slides', '<div id="app" style="width:1280px;height:720px"></div><script type="module" src="/__notes-entry.js"></script>'))
      })
    },
  }],
})
let browser
const errors = []
try {
  await server.listen()
  browser = await chromium.launch({ headless: true, channel: process.env.LAYOUT_BROWSER_CHANNEL || undefined })
  const page = await browser.newPage({ viewport: { width: 1440, height: 960 }, reducedMotion: 'reduce' })
  page.on('pageerror', error => errors.push(error.message))
  await page.addInitScript(({ fixtures }) => {
    localStorage.setItem('app-theme', 'light-notes')
    let callbackId = 0
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callbackId,
      unregisterCallback: () => {},
      invoke: async (command) => {
        if (command === 'is_first_run') return false
        if (command === 'get_app_settings') return { fonts: {}, llm_configured: true, embeddings_ready: true }
        if (command === 'get_settings') return { llm: {}, embedding: {}, multimodal: {}, concurrency: 4, repair_rounds: 4 }
        if (command === 'list_projects') return ['产品发布提案', '季度工作总结', '设计灵感记录'].map((name, i) => ({ id: i + 1, name, slide_count: 12 - i, created_at: 1791561600, updated_at: 1791561600 }))
        if (command === 'open_project') return { id: 1, name: '产品发布提案', md_content: '# 产品发布提案\n\n## 从一张便签开始\n\n整理想法，写成一页好稿。', blueprints_json: JSON.stringify(fixtures) }
        if (command === 'get_project_media') return []
        if (command === 'create_project') return 2
        if (command === 'get_project_aspect_ratio') return 'ratio_16x9'
        if (command === 'list_system_fonts') return []
        if (command === 'plugin:window|is_maximized') return false
        if (command === 'plugin:event|listen') return ++callbackId
        return null
      },
    }
    window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} }
  }, { fixtures })
  const base = server.resolvedUrls.local[0]
  await page.goto(base)
  await page.locator('.project-card').first().waitFor()
  assert.equal(await page.locator('.project-card').count(), 3)
  await page.getByTitle('切换主题').click()
  await page.getByRole('button', { name: '暗·星图', exact: true }).click()
  assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'dark-blue')
  const swatch = page.getByRole('button', { name: '便签·稿纸', exact: true })
  await swatch.click()
  await page.reload()
  await page.locator('.project-card').first().waitFor()
  await page.waitForTimeout(700)
  assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'light-notes')
  const palette = await page.evaluate(() => {
    const style = getComputedStyle(document.documentElement)
    return ['--studio-bg', '--studio-panel', '--studio-text'].map(key => style.getPropertyValue(key).trim())
  })
  assert.deepEqual(palette, ['#eed9ac', '#fffdf7', '#493b30'])
  await page.screenshot({ path: new URL('home.png', output).pathname.replace(/^\/(\w:)/, '$1') })
  await page.goto(base + '#/project/1')
  await page.locator('.slide-shell').first().waitFor()
  await page.waitForTimeout(800)
  await page.screenshot({ path: new URL('editor.png', output).pathname.replace(/^\/(\w:)/, '$1') })
  await page.goto(base + '#/new')
  await page.reload()
  await page.locator('.cm-content').fill('# 产品发布提案\n\n## 从一张便签开始\n\n整理想法，写成一页好稿。')
  await page.locator('.md-preview').first().waitFor()
  await page.screenshot({ path: new URL('prepare.png', output).pathname.replace(/^\/(\w:)/, '$1') })
  await page.goto(base + '__notes-slides')
  await page.waitForFunction(() => !!window.renderSlide)
  let checks = 0
  for (const slide of fixtures) {
    for (const aspect of ['ratio_16x9', 'ratio_32x9', 'ratio_48x9']) {
      await page.evaluate(s => window.renderSlide(s), { ...slide, aspect_ratio: aspect })
      const result = await page.locator('.slide-shell').evaluate(el => {
        const style = getComputedStyle(el)
        const frame = el.querySelector('.slide-shell__frame')
        return { background: style.backgroundColor, grid: style.backgroundImage,
          color: style.color, radius: getComputedStyle(frame).borderRadius,
          overflow: el.scrollWidth > el.clientWidth + 1 || el.scrollHeight > el.clientHeight + 1 }
      })
      assert.equal(result.background, 'rgb(255, 253, 247)', slide.kind)
      assert.equal(result.color, 'rgb(73, 59, 48)', slide.kind)
      assert.ok(result.grid.includes('repeating-linear-gradient'), slide.kind)
      assert.equal(result.radius, '0px', slide.kind)
      assert.equal(result.overflow, false, slide.kind)
      if (slide.kind === 'overview') {
        const fits = await page.locator('.overview-card').evaluateAll(cards => cards.every(card => {
          const body = card.querySelector('.overview-desc')
          const bounds = card.getBoundingClientRect(), copy = body.getBoundingClientRect()
          return copy.bottom <= bounds.bottom + 1 && copy.right <= bounds.right + 1
        }))
        assert.ok(fits, 'Overview content must fit its note cards')
      }
      checks++
    }
  }
  await page.evaluate(s => window.renderSlide(s), fixtures[0])
  await page.waitForTimeout(800)
  await page.locator('#app').screenshot({ path: new URL('cover.png', output).pathname.replace(/^\/(\w:)/, '$1') })
  assert.deepEqual(errors, [], 'Browser errors')
  console.log(`PASS: theme selection and persistence; home, editor, preparation; ${checks} slide/ratio checks. Screenshots: output/notes-theme/`)
} finally {
  await browser?.close()
  await server.close()
}
