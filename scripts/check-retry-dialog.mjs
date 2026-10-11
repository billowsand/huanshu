import { createServer } from 'vite'
import { createRequire } from 'node:module'
import assert from 'node:assert/strict'
const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright')
const entry = `
import { createApp, h } from 'vue'
import Step from '/src/views/steps/Step2Generate.vue'
import 'virtual:uno.css'
import '@unocss/reset/tailwind.css'
import '/src/style.css'
import '/src/themes/notes.css'
import '/src/themes/notes-studio.css'
window.calls = []
window.__TAURI_INTERNALS__ = { invoke: async (command, args) => {
  window.calls.push({ command, args })
  if (window.layoutError) throw new Error('布局读取失败')
  return { current_kind: 'feature_grid', kinds: ['feature_grid', 'section_intro'] }
} }
createApp({ render: () => h(Step, {
  stage: 'done', stageLabel: '完成', running: false, message: '', slideCount: 4,
  blueprints: [{kind:'cover',title:'封面'}, {kind:'overview',title:'目录',overview_items:[]},
    {kind:'feature_grid',title:'失败页',cards:[]}, {kind:'closing',title:'结束'}],
  mediaMap: {}, logs: [], selectedSlideIndex: 0,
  pageStatuses: new Map([[0, {stage:'error',message:'生成失败'}]]),
  retryingPages: new Set(), runId: 42,
  onRetryPage: (index, kind) => { window.confirmed = {index, kind} },
}) }).mount('#app')
`
const server = await createServer({
  optimizeDeps: { entries: [], include: ['vue'], noDiscovery: true },
  server: { port: 5186, host: '127.0.0.1', hmr: false },
  plugins: [{ name: 'retry-dialog-check',
    resolveId(id) { if (id === '/__retry-entry.js') return id },
    load(id) { if (id === '/__retry-entry.js') return entry },
    configureServer(vite) { vite.middlewares.use('/__retry-check', async (_req, res) => {
      res.setHeader('Content-Type', 'text/html')
      res.end(await vite.transformIndexHtml('/__retry-check', '<html data-theme="light-notes"><body><div id="app"></div><script type="module" src="/__retry-entry.js"></script></body></html>'))
    }) },
  }],
})
let browser
try {
  await server.listen()
  browser = await chromium.launch({ headless: true, channel: 'msedge' })
  const page = await browser.newPage({ viewport: { width: 1440, height: 900 } })
  const errors = []
  page.on('pageerror', e => errors.push(e.message))
  await page.goto(server.resolvedUrls.local[0] + '__retry-check')
  await page.getByRole('button', { name: '重试这一页' }).click()
  await page.getByRole('dialog').waitFor({ state: 'visible', timeout: 5000 })
  await page.getByRole('button', { name: '按此布局重试' }).click()
  assert.deepEqual(await page.evaluate(() => window.confirmed), { index: 0, kind: 'feature_grid' })
  assert.deepEqual(await page.evaluate(() => window.calls), [{ command: 'get_retry_layouts', args: { runId: 42, pageIndex: 0 } }])
  await page.getByRole('button', { name: '重试这一页' }).click()
  await page.getByRole('button', { name: '取消', exact: true }).click()
  await page.getByRole('dialog').waitFor({ state: 'hidden' })
  await page.evaluate(() => { window.layoutError = true })
  await page.getByRole('button', { name: '重试这一页' }).click()
  await page.getByRole('alert').waitFor({ state: 'visible' })
  assert.equal(await page.getByRole('button', { name: '按此布局重试' }).isEnabled(), false)
  assert.deepEqual(errors, [])
  console.log('Retry dialog click, confirm, cancel, reopen and error checks passed')
} finally {
  await browser?.close()
  await server.close()
}
