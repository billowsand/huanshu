// Real-browser theme checks. Tauri data is mocked; no real projects are changed.
// PLAYWRIGHT_MODULE_PATH may point to the desktop app's bundled playwright.
import { createServer } from 'vite'
import { createRequire } from 'node:module'
import { readFile, mkdir } from 'node:fs/promises'
import { join } from 'node:path'
import { fileURLToPath } from 'node:url'
import { tmpdir } from 'node:os'
import assert from 'node:assert/strict'

const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright')
const fixtures = JSON.parse(await readFile(new URL('../fixtures/all-layout-blueprints.json', import.meta.url), 'utf8'))
fixtures[0] = { kind: 'cover', title: '把想法，写成一页好稿。', subtitle: '从灵感便签到完整演示', badges: ['便签 · 稿纸'] }
fixtures.push(
  { kind: 'section_intro', section: '01', badge: '章节导览', title: '从灵感到表达', subtitle: '先整理线索，再写成一页完整的稿纸。', note: '围绕内容、结构与表达展开。', cards: [
    { title: '内容准备', tone: 'blue', icon: 'i-carbon:document', body: '整理材料，留下值得展开的想法。' },
    { title: '结构规划', tone: 'green', icon: 'i-carbon:catalog', items: ['找到讲述的顺序', '形成清晰的章节'] },
    { title: '页面表达', tone: 'amber', icon: 'i-carbon:pen', body: '选择合适的版式，让信息更易阅读。' },
    { title: '检查完善', tone: 'red', icon: 'i-carbon:checkmark', body: '检查内容与细节，完成最后一轮校对。' },
  ] },
  { kind: 'issue_stack', section: '02', title: '给问题留下清楚的批注', cards: [
    { title: '内容过多', tone: 'blue', icon: 'i-carbon:document', body: '把材料整理成几个清晰的主题，再逐页展开。', items: ['分组整理', '适度精简'] },
    { title: '结构分散', tone: 'amber', icon: 'i-carbon:catalog', body: '找到贯穿页面的主线，让每一页都有明确的位置。', items: ['梳理顺序', '明确重点'] },
    { title: '细节遗漏', tone: 'red', icon: 'i-carbon:warning', body: '在提交前逐项检查，修正标题、数据和页尾说明。', items: ['校对文字', '检查来源'] },
  ] },
  { kind: 'closing', title: '让每一个想法，清晰落在纸上。', subtitle: '从一张便签开始，完成一次完整表达。', badges: ['便签 · 稿纸'], note: '感谢阅读，期待下一次分享。' },
)
const output = new URL('../output/notes-theme/', import.meta.url)
await mkdir(output, { recursive: true })
await mkdir(new URL('layouts/', output), { recursive: true })
const entry = `
import {createApp,h,shallowRef,nextTick} from 'vue';
import SlideRenderer from '/src/components/SlideRenderer.vue';
import GlassCard from '/src/components/GlassCard.vue';
import CardGrid from '/src/components/CardGrid.vue';
import TagBadge from '/src/components/TagBadge.vue';
import StepNumber from '/src/components/StepNumber.vue';
import Step2Generate from '/src/views/steps/Step2Generate.vue';
import 'virtual:uno.css';
import '/node_modules/@unocss/reset/tailwind.css';
import '/src/style.css';
import '/src/themes/terminal.css';
import '/src/themes/notes.css';
import '/src/themes/notes-layouts.css';
import '/src/themes/notes-studio.css';
import '/src/themes/typography.css';
import '/src/views/WorkflowView.vue?vue&type=style&index=0&lang.css';
document.documentElement.dataset.theme='light-notes';
const slide=shallowRef({kind:'cover',title:'便签·稿纸'});
const gallery=shallowRef(null),generation=shallowRef(null);
const labels={cover:'封面',overview:'目录',feature_grid:'功能卡片',outcome_grid:'成果卡片',center_grid:'中心网格',section_list:'章节列表',focus_example:'重点示例',split_layers:'分层说明',spotlight:'重点页',timeline:'时间线',step_flow:'步骤流程',process:'阶段流程',compare:'对比',swot:'SWOT分析',section_intro:'章节导览',issue_stack:'问题批注',closing:'结束页'};
createApp({render:()=>generation.value?h(Step2Generate,generation.value):gallery.value?
 h('div',{style:'display:grid;grid-template-columns:repeat(4,400px);gap:20px;padding:24px;background:#eed9ac'},gallery.value.map(s=>
  h('div',{style:'width:400px;height:254px'},[
   h('div',{style:'height:28px;color:#493b30;font-size:14px'},labels[s.kind]||s.kind),
   h('div',{style:'width:400px;height:225px;overflow:hidden;box-shadow:0 3px 8px #60472b26'},[
    h('div',{style:'width:1280px;height:720px;transform:scale(.3125);transform-origin:top left'},[h(SlideRenderer,{slide:s})])
   ])
  ])
 )):h(SlideRenderer,{slide:slide.value})})
 .component('GlassCard',GlassCard).component('CardGrid',CardGrid)
 .component('TagBadge',TagBadge).component('StepNumber',StepNumber).mount('#app');
window.renderSlide=async(value)=>{
 const width={ratio_16x9:1280,ratio_32x9:2560,ratio_48x9:3840}[value.aspect_ratio||'ratio_16x9'];
 document.querySelector('#app').style.width=width+'px';
 slide.value=value;await nextTick();await document.fonts.ready;
 await new Promise(resolve=>requestAnimationFrame(()=>requestAnimationFrame(resolve)));
};
window.renderGallery=async(values)=>{
 document.querySelector('#app').style.cssText='width:1708px;height:auto';
 gallery.value=values;await nextTick();await document.fonts.ready;
};
window.renderGeneration=async(values)=>{
 document.querySelector('#app').style.cssText='width:1440px;height:960px;background:#eed9ac';
 generation.value={stage:'content',stageLabel:'生成中',running:true,message:'正在整理页面内容',slideCount:4,
  blueprints:values.slice(0,2),mediaMap:{},logs:[],selectedSlideIndex:0,
  pageStatuses:new Map([[0,{stage:'done'}],[1,{stage:'content'}],[2,{stage:'error',previousStage:'layout'}],[3,{stage:'pending'}]])};
 await nextTick();await document.fonts.ready;
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
    if (!localStorage.getItem('app-theme')) localStorage.setItem('app-theme', 'light-notes')
    let callbackId = 0
    window.__TAURI_INTERNALS__ = {
      metadata: { currentWindow: { label: 'main' }, currentWebview: { label: 'main' } },
      transformCallback: () => ++callbackId,
      unregisterCallback: () => {},
      invoke: async (command) => {
        if (command === 'is_first_run') return false
        if (command === 'get_app_settings') return { fonts: {}, llm_configured: true, embeddings_ready: true, initialized_embedding_model: 'embedding-model', data_dir: 'E:/demo', media_dir: 'E:/demo/media' }
        if (command === 'get_settings') {
          const service = { base_url: 'http://127.0.0.1:1234', api_key: '', model: 'demo-model', proxy: { enabled: false, url: '', username: '', password: '' } }
          return { llm: service, embedding: { ...service, model: 'embedding-model' }, multimodal: service, concurrency: 4, repair_rounds: 4 }
        }
        if (command === 'list_projects') return ['产品发布提案', '季度工作总结', '设计灵感记录'].map((name, i) => ({ id: i + 1, name, slide_count: 12 - i, created_at: 1791561600, updated_at: 1791561600 }))
        if (command === 'open_project') return { id: 1, name: '产品发布提案', md_content: '# 产品发布提案\n\n## 从一张便签开始\n\n整理想法，写成一页好稿。', blueprints_json: JSON.stringify(fixtures) }
        if (command === 'get_project_media') return []
        if (command === 'create_project') return 2
        if (command === 'get_project_aspect_ratio') return 'ratio_16x9'
        if (command === 'list_system_fonts') return [{ name: 'Microsoft YaHei', label: '微软雅黑', aliases: [] }, { name: 'SimSun', label: '宋体', aliases: [] }]
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
  await page.locator('.project-card').first().getByTitle('导出为 .keynn').click()
  await page.locator('.export-modal').waitFor()
  await page.getByText('加密导出', { exact: true }).first().click()
  await page.getByPlaceholder('设置密码', { exact: true }).fill('Draft2026!Paper')
  await page.getByPlaceholder('再次输入密码').fill('mismatch')
  await page.screenshot({ path: fileURLToPath(new URL('export.png', output)) })
  assert.equal(await page.locator('.field-error').count(), 1)
  await page.getByRole('button', { name: '取消', exact: true }).click()
  await page.goto(base + '#/project/1')
  await page.locator('.slide-shell').first().waitFor()
  await page.waitForTimeout(800)
  await page.screenshot({ path: new URL('editor.png', output).pathname.replace(/^\/(\w:)/, '$1') })
  await page.getByText('打开高级 JSON', { exact: true }).click()
  await page.locator('.ej-wrap').waitFor()
  await page.screenshot({ path: fileURLToPath(new URL('json-editor.png', output)) })
  await page.getByRole('button', { name: '演示', exact: true }).click()
  await page.locator('.presentation-overlay').waitFor()
  await page.keyboard.press('o')
  await page.locator('.pres-overview-card').first().waitFor()
  await page.screenshot({ path: fileURLToPath(new URL('presentation.png', output)) })
  await page.keyboard.press('Escape')
  await page.goto(base + '#/new')
  await page.reload()
  await page.locator('.cm-content').fill('# 产品发布提案\n\n## 从一张便签开始\n\n整理想法，写成一页好稿。')
  await page.locator('.md-preview').first().waitFor()
  await page.screenshot({ path: new URL('prepare.png', output).pathname.replace(/^\/(\w:)/, '$1') })
  await page.goto(base + '#/settings')
  await page.locator('.font-trigger').first().waitFor()
  await page.locator('.font-trigger').first().click()
  await page.locator('.font-popover').first().waitFor()
  await page.screenshot({ path: fileURLToPath(new URL('settings.png', output)), fullPage: true })
  await page.goto(base + '#/wizard')
  await page.locator('.wizard-card').waitFor()
  await page.screenshot({ path: fileURLToPath(new URL('wizard.png', output)) })
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
      if (slide.kind === 'swot') {
        const sizes = await page.locator('.swot-body').evaluateAll(cards => cards.map(card => ({ height: card.clientHeight, scroll: card.scrollHeight,
          children: [...card.children].map(el => ({ class: el.className, height: el.clientHeight, scroll: el.scrollHeight })) })))
        assert.ok(sizes.every(card => card.scroll <= card.height + 1 && card.children.every(el => el.scroll <= el.height + 1)), 'SWOT labels and copy must fit: ' + JSON.stringify({ aspect, sizes }))
      }
      if (slide.kind === 'spotlight') {
        const fit = await page.locator('.spotlight-side').evaluate(el => {
          const bounds = el.getBoundingClientRect();
          const bottoms = [...el.children].map(panel => panel.getBoundingClientRect().bottom);
          return { bottom: bounds.bottom, bottoms };
        });
        assert.ok(fit.bottoms.every(bottom => bottom <= fit.bottom + 1), 'Spotlight side panels must fit the writing sheet: ' + JSON.stringify({ aspect, ...fit }));
      }
      if (aspect === 'ratio_16x9') {
        await page.locator('#app').screenshot({ path: fileURLToPath(new URL('layouts/' + slide.kind + '.png', output)) })
      }
      checks++
    }
  }
  const comparison = structuredClone(fixtures.find(slide => slide.kind === 'compare'));
  comparison.compare_data.left.items[1].highlight = true;
  await page.evaluate(s => window.renderSlide(s), comparison);
  const highlight = await page.locator('.cp-row').nth(1).locator('.cp-item').first().evaluate(el => ({
    background: getComputedStyle(el).backgroundColor,
    tone: getComputedStyle(el).getPropertyValue('--notes-tone-bg').trim(),
  }));
  assert.ok(highlight.tone && highlight.background !== 'rgb(250, 244, 232)', 'Highlighted comparison rows retain their semantic color');
  await page.evaluate(s => window.renderSlide(s), fixtures[0])
  await page.waitForTimeout(800)
  await page.locator('#app').screenshot({ path: new URL('cover.png', output).pathname.replace(/^\/(\w:)/, '$1') })
  await page.evaluate(values => window.renderGallery(values), fixtures)
  await page.setViewportSize({ width: 1708, height: 960 })
  await page.locator('#app').screenshot({ path: fileURLToPath(new URL('gallery.png', output)) })
  await page.evaluate(values => window.renderGeneration(values), fixtures)
  await page.setViewportSize({ width: 1440, height: 960 })
  await page.locator('.slide-progress-card').first().waitFor()
  assert.equal(await page.locator('.slide-progress-card').count(), 4)
  await page.screenshot({ path: fileURLToPath(new URL('generation.png', output)) })
  const statusColors = await page.locator('.spc-status').evaluateAll(elements => elements.map(el => getComputedStyle(el).color))
  assert.equal(new Set(statusColors).size, 4, 'Four generation states must remain visually distinct')
  assert.deepEqual(errors, [], 'Browser errors')
  console.log(`PASS: theme selection/persistence; home, preparation, generation, editor, JSON, export, settings, wizard, presentation; ${checks} slide/ratio checks. Screenshots: output/notes-theme/`)
} finally {
  await browser?.close()
  await server.close()
}
