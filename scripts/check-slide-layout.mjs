// Real-browser regression checks; pass a Playwright module path if not installed locally.
import { createServer } from 'vite'
import { createRequire } from 'node:module'
import assert from 'node:assert/strict'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { readFile } from 'node:fs/promises'
const require = createRequire(import.meta.url)
const { chromium } = require(process.env.PLAYWRIGHT_MODULE_PATH || 'playwright')
const entry = `
      import {createApp, h, shallowRef, nextTick} from 'vue';
      import SlideRenderer from '/src/components/SlideRenderer.vue';
      import GlassCard from '/src/components/GlassCard.vue';
      import CardGrid from '/src/components/CardGrid.vue';
      import TagBadge from '/src/components/TagBadge.vue';
      import StepNumber from '/src/components/StepNumber.vue';
      import 'virtual:uno.css';
      import '/node_modules/@unocss/reset/tailwind.css';
      import '/src/style.css';
      import '/src/themes/terminal.css';
      import '/src/themes/typography.css';
      const slide=shallowRef({kind:'overview',title:'提纲总览',overview_items:[]});
      const app=createApp({render:()=>h(SlideRenderer,{slide:slide.value})});
      app.component('GlassCard',GlassCard).component('CardGrid',CardGrid).component('TagBadge',TagBadge).component('StepNumber',StepNumber).mount('#app');
      window.renderSlide=async (value,theme='light-terminal')=>{
        document.documentElement.dataset.theme=theme;
        const width={ratio_16x9:1280,ratio_32x9:2560,ratio_48x9:3840}[value.aspect_ratio||'ratio_16x9'];
        document.getElementById('app').style.width=width+'px';
        slide.value=value; await nextTick(); await document.fonts.ready;
        await new Promise(resolve=>setTimeout(resolve,180));
      };
    `
const server = await createServer({
  cacheDir: join(tmpdir(), 'huanshu-layout-vite-cache'),
  optimizeDeps: { entries: [], include: ['vue'], noDiscovery: true },
  server: { port: 5179, strictPort: false, host: '127.0.0.1', hmr: false },
  plugins: [{ name:'layout-check-entry', resolveId(id){if(id==='/__layout-entry.js')return id}, load(id){if(id==='/__layout-entry.js')return entry},
    configureServer(vite) {
      vite.middlewares.use('/__layout-check', async (_req,res) => {
        res.setHeader('Content-Type','text/html')
        res.end(await vite.transformIndexHtml('/__layout-check','<div id="app" style="width:1280px;height:720px"></div><script type="module" src="/__layout-entry.js"></script>'))
      })
    },
  }],
})
let browser
try {
  await server.listen()
  browser = await chromium.launch({ headless: true, channel: process.env.LAYOUT_BROWSER_CHANNEL || undefined })
  const page = await browser.newPage({ viewport: { width: 3840, height: 760 }, reducedMotion: 'reduce' })
  page.on('pageerror', error => console.error('Browser:',error.message))
  page.on('console', message => { if(message.type()==='error') console.error(message.text()) })
  await page.goto(server.resolvedUrls.local[0]+'__layout-check')
  await page.waitForFunction(() => !!window.renderSlide)
  const themes = ['light-terminal', 'dark-terminal', 'dark-yellow', 'dark-blue', 'dark-glass-blue']
  let checked = 0
  for (const count of (process.env.LAYOUT_TEMPLATES_ONLY ? [] : [3, 4, 5, 6, 7, 8])) {
    console.log('Checking '+count+' items')
    for (const long of [false, true]) {
      for (const aspect of ['ratio_16x9', 'ratio_32x9', 'ratio_48x9']) {
        for (const note of ['', '从当前问题出发，介绍建设方向、预期效益与实施安排。']) {
          const value = { kind: 'overview', title: '提纲总览', aspect_ratio: aspect, note,
            overview_items: Array.from({ length: count }, (_, i) => ({ number: String(i + 1).padStart(2, '0'),
              title: i % 2 ? '统筹机制与实施建议' : '当前需要关注的几个问题',
              desc: long ? '结合实际场景明确优先级，统筹人员、经费与技术资源，建立分阶段实施和反馈机制，形成可以持续推进的工作方案，并根据试点成效逐步推广。' : '当前作业负担与资源投入不足',
            })) }
          for (const theme of themes) {
            await page.evaluate(([s,t]) => window.renderSlide(s,t), [value,theme])
            const result = await page.evaluate(() => {
              const grid=document.querySelector('.overview-grid');
              const header=document.querySelector('.slide-header').getBoundingClientRect();
              const pageTitle=parseFloat(getComputedStyle(document.querySelector('.slide-title')).fontSize);
              return [...document.querySelectorAll('.overview-card')].map(card=>{
                const title=card.querySelector('.overview-title'),body=card.querySelector('.overview-desc');
                const a=card.getBoundingClientRect(),b=title.getBoundingClientRect(),c=body.getBoundingClientRect();
                const g=grid.getBoundingClientRect();
                return { title:parseFloat(getComputedStyle(title).fontSize),body:parseFloat(getComputedStyle(body).fontSize),pageTitle,
                  fits:b.top>=a.top-1&&c.bottom<=a.bottom+1&&c.right<=a.right+1&&a.bottom<=g.bottom+1&&g.top>=header.bottom-1,
                  bounds:[a.top,a.bottom,b.top,c.bottom] };
              });
            })
            assert.equal(result.length,count)
            for (const card of result) {
              const context=JSON.stringify({count,long,aspect,note:!!note,theme,card})
              assert.ok(card.fits,context)
              assert.ok(card.body>=15.9&&card.title>=card.body*1.24&&card.pageTitle>card.title,context)
            }
            checked++
          }
        }
      }
    }
  }
  // Return from dense to sparse and from long to short without retaining a shrink factor.
  await page.evaluate(() => window.renderSlide({ kind:'overview',title:'提纲总览',overview_items:
    Array.from({length:4},(_,i)=>({number:String(i+1),title:'章节标题',desc:'简短说明'})) }))
  assert.equal(await page.locator('.overview-content').evaluate(el=>el.style.getPropertyValue('--overview-fit')),'1')
  if (process.env.LAYOUT_SCREENSHOT) await page.locator('#app').screenshot({ path:process.env.LAYOUT_SCREENSHOT })
  const roles = {
    section_list: ['.slide-card-title', '.slide-text-meta'],
    timeline: ['.tl-card .slide-card-title', '.tl-card .slide-text-caption'],
    step_flow: ['.sf-htitle', '.sf-hbody'],
    center_grid: ['.center-grid-item-title', '.glass-card .slide-meta'],
    outcome_grid: ['.outcome-title', '.outcome-copy'],
    feature_grid: ['.feature-header span', '.feature-copy'],
    issue_stack: ['.issue-card__eyebrow', '.issue-card__summary'],
    spotlight: ['.spotlight-panel-title', '.spotlight-side .slide-meta'],
    split_layers: ['.slide-card-title', '.split-left-body'],
    focus_example: ['.slide-card-title', '.glass-card .slide-text-caption'],
    compare: ['.slide-card-title', '.cp-body .slide-text-caption'],
    process: ['.phase-head__title', '.step-desc'],
    swot: ['.swot-flag-text', '.swot-item span'],
  }
  const fixtures=JSON.parse(await readFile(new URL('../fixtures/all-layout-blueprints.json',import.meta.url),'utf8'))
  let templateChecks=0
  for (const slide of fixtures.filter(s=>roles[s.kind])) {
    for (const theme of themes) {
      await page.evaluate(([s,t])=>window.renderSlide(s,t),[slide,theme])
      const [titleSelector,bodySelector]=roles[slide.kind]
      const sizes=await page.evaluate(([a,b])=>{
        const titles=[...document.querySelectorAll(a)],bodies=[...document.querySelectorAll(b)]
        return { titles:titles.map(el=>parseFloat(getComputedStyle(el).fontSize)),bodies:bodies.map(el=>parseFloat(getComputedStyle(el).fontSize)) }
      },[titleSelector,bodySelector])
      assert.ok(sizes.titles.length&&sizes.bodies.length,slide.kind+' missing role elements')
      assert.ok(Math.min(...sizes.titles)>Math.max(...sizes.bodies),JSON.stringify({kind:slide.kind,theme,sizes}))
      templateChecks++
    }
  }
  console.log('PASS: '+checked+' overview cases; '+templateChecks+' template hierarchy cases; content edits reset fitting.')
} finally {
  await browser?.close()
  await server.close()
}
