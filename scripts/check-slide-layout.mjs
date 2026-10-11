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
      import '/src/themes/notes.css';
      import '/src/themes/notes-layouts.css';
      import '/src/themes/typography.css';
      import '/src/views/WorkflowView.vue?vue&type=style&index=0&lang.css';
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
  const themes = ['light-terminal', 'dark-terminal', 'dark-yellow', 'dark-blue']
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
  for (const slide of fixtures.filter(s=>roles[s.kind] && !process.env.ISSUE_ONLY)) {
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
  // Include the editor's global CSS: sidebar descriptions must not constrain slide copy.
  let processChecks = 0
  for (const count of (process.env.ISSUE_ONLY ? [] : [2, 3, 4])) {
    const slide = { kind: 'process', title: '试点安排', phases: Array.from({ length: count }, (_, i) => ({
      phase: '阶段' + (i + 1), title: '场景验证与流程校准', tone: 'blue',
      steps: [
        { label: '选取对象', desc: '选取一类通用业务场景，明确参与人员与验收标准，形成可持续推进的试点方案。' },
        { label: '跑通流程', desc: '完整执行从需求提交到结果验收的流程，记录问题并根据反馈调整实施方案。' },
      ], highlight: '验证技术可行性与流程完整性。',
    })) }
    for (const theme of themes) {
      await page.evaluate(([s,t]) => window.renderSlide(s,t), [slide,theme])
      const descriptions = await page.locator('.process-slide .step-desc').evaluateAll(elements => elements.map(el => {
        const style = getComputedStyle(el), bounds = el.getBoundingClientRect()
        const card = el.closest('.phase-steps-wrap').getBoundingClientRect()
        return { width: bounds.width, whiteSpace: style.whiteSpace, textOverflow: style.textOverflow,
          wraps: bounds.height > parseFloat(style.lineHeight) * 1.5,
          fits: el.scrollWidth <= el.clientWidth + 1 && el.scrollHeight <= el.clientHeight + 1
            && bounds.right <= card.right && bounds.bottom <= card.bottom }
      }))
      assert.equal(descriptions.length, count * 2)
      for (const desc of descriptions) {
        const context = JSON.stringify({ count, theme, desc })
        assert.ok(desc.width > 130 && desc.whiteSpace === 'normal' && desc.textOverflow !== 'ellipsis', context)
        assert.ok(desc.wraps && desc.fits, context)
      }
      processChecks++
    }
  }
  // Issue stacks must preserve body copy instead of clipping it beneath a fixed header.
  let issueChecks = 0
  const issueCards = [
    { title:'背景压力',tone:'blue',icon:'i-carbon:document',body:'行业负增长倒逼转型，政策推动从拼产能转向拼安全与效率。',items:['主业面临负增长压力','政策强制要求转型','从拼产能转向拼安全、拼效率'] },
    { title:'核心任务',tone:'green',icon:'i-carbon:catalog',body:'减少人员暴露，在高风险场景中提升自主作业能力。',items:['减人','提效','保安全','瓦斯爆炸、顶板塌方等高风险场景自主作业'] },
    { title:'数据挑战',tone:'amber',icon:'i-carbon:data-base',body:'现场数据质量不稳定，设备和信号条件限制模型判断。',items:['传感器噪声','设备断联','高温高压信号丢失'] },
    { title:'可靠性要求',tone:'red',icon:'i-carbon:warning',body:'极端条件下仍需可靠判断，错误决策可能造成重大安全损失。',items:['极端条件判断可靠','系统错误代价不可接受'] },
  ]
  const issueFooter='采矿场景是大模型落地的特殊场景，核心价值在于极端环境下的自主决策与安全保障。'
  for (const count of [2,3,4]) {
    for (const aspect of ['ratio_16x9','ratio_32x9','ratio_48x9']) {
      for (const theme of [...themes,'light-notes']) {
        for (const subtitle of ['', '极端环境下的自主作业与安全保障']) {
          await page.evaluate(([s,t])=>window.renderSlide(s,t),[{
            kind:'issue_stack',section:'01',title:'采矿业：极端环境下的“特种兵”',
            aspect_ratio:aspect,subtitle,cards:issueCards.slice(0,count),footer:issueFooter,
          },theme])
          const result=await page.evaluate(()=>{
            const stack=document.querySelector('.issue-stack'),footer=document.querySelector('.issue-footer');
            const frame=document.querySelector('.slide-shell__frame').getBoundingClientRect();
            const cards=[...document.querySelectorAll('.issue-card')];
            return { rows:stack.classList.contains('issue-stack--rows'),
              footerFits:footer.getBoundingClientRect().bottom<=frame.bottom-20,
              bounds:[stack.getBoundingClientRect().top,stack.getBoundingClientRect().bottom,footer.getBoundingClientRect().bottom,frame.bottom],
              cards:cards.map(card=>{
                const bounds=card.getBoundingClientRect();
                const head=card.querySelector('.issue-card__head').getBoundingClientRect();
                const body=card.querySelector('.issue-card__body').getBoundingClientRect();
                return { horizontal:body.left>=head.right-1,
                  fits:bounds.bottom<=footer.getBoundingClientRect().top &&
                    [...card.querySelectorAll('.issue-card__eyebrow,.issue-card__summary,.issue-card__pill')].every(el=>{
                      const r=el.getBoundingClientRect();
                      return r.top>=bounds.top-1&&r.bottom<=bounds.bottom+1&&r.right<=bounds.right+1&&r.left>=bounds.left-1
                        &&el.scrollWidth<=el.clientWidth+1&&el.scrollHeight<=el.clientHeight+1;
                    }) };
              }) };
          })
          const context=JSON.stringify({count,aspect,theme,subtitle,result})
          if (process.env.ISSUE_SCREENSHOT && !result.cards.every(c=>c.fits))
            await page.locator('#app').screenshot({path:process.env.ISSUE_SCREENSHOT})
          assert.equal(result.rows,count===4,context)
          assert.ok(result.footerFits,context)
          for (const card of result.cards) {
            assert.equal(card.horizontal,count===4,context)
            assert.ok(card.fits,context)
          }
          issueChecks++
          if (process.env.ISSUE_SCREENSHOT && count===4 && aspect==='ratio_16x9' && theme==='light-notes' && !subtitle)
            await page.locator('#app').screenshot({path:process.env.ISSUE_SCREENSHOT})
        }
      }
    }
  }
  // A single long keyword must wrap within the body rather than widen the card.
  await page.evaluate(([cards,footer])=>window.renderSlide({kind:'issue_stack',title:'长关键词换行',cards:cards.map(c=>({...c,body:'一句简短说明。',items:['高风险场景中的自主作业与安全保障'.repeat(4)]})),footer},'light-notes'),[issueCards,issueFooter])
  for (const pill of await page.locator('.issue-card__pill').evaluateAll(els=>els.map(el=>({
    wraps:el.clientHeight>parseFloat(getComputedStyle(el).lineHeight)*1.5,
    fits:el.scrollWidth<=el.clientWidth+1&&el.getBoundingClientRect().bottom<=el.closest('.issue-card').getBoundingClientRect().bottom,
  })))) assert.ok(pill.wraps&&pill.fits,JSON.stringify(pill))
  console.log('PASS: '+checked+' overview cases; '+templateChecks+' template hierarchy cases; '+processChecks+' process wrapping cases; '+issueChecks+' issue stack cases; long keywords wrap; content edits reset fitting.')
} finally {
  await browser?.close()
  await server.close()
}
