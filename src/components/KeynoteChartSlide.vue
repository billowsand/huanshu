<script setup lang="ts">
/* chart_focus 模板：左侧要点 + 右侧图表。
 * 图表 option 由共享模块 buildChartOption 生成（与 PDF 导出的 SSR SVG 同一实现），
 * 主题 token 全部来自 CSS 变量，组件不写死颜色，主题切换自动跟随。 */
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import SlideTitle from './SlideTitle.vue'
import type { ChartSpec } from './chartOptions'
import { buildChartOption, chartSourceLabel, echarts, themeFromCssVars, TYPE_SCALE_BASE } from './chartOptions'
import { renderRichText } from '../utils/richText'

export interface ChartPoint {
  step?: string
  title: string
  body?: string
}

const props = defineProps<{
  section: string
  title: string
  subtitle?: string
  points: ChartPoint[]
  chart: ChartSpec
  footer?: string
}>()

const chartBox = ref<HTMLElement>()
// 注意：内部实例变量必须叫 chartInst，叫 chart 会和 props.chart 冲突
let chartInst: echarts.EChartsType | undefined

function render() {
  if (!chartBox.value || !props.chart) return
  const scope = chartBox.value.closest('[data-theme]') ?? document.documentElement
  const theme = themeFromCssVars(getComputedStyle(scope as Element))
  // 本页的 --type-scale 由 SlideRenderer 挂在 .slide-shell 上，图表容器继承得到；
  // PDF 侧用 typst 报告的同一个值（见 usePdfExport），两端图表字号一致。
  const typeScale = Number.parseFloat(getComputedStyle(chartBox.value).getPropertyValue('--type-scale')) || TYPE_SCALE_BASE
  const option = buildChartOption(props.chart, theme, 'screen', typeScale)
  chartInst ??= echarts.init(chartBox.value, null, { renderer: 'svg' })
  chartInst.setOption(option, { notMerge: true })
}

// ECharts 的颜色和字体是 setOption 时从 CSS 变量读出的快照：换主题（<html data-theme>）或在
// 设置里换字体（<html style="--font-*">）都不会自动生效，必须重新 render。
// 尺寸跟随图表容器本身（编辑器分栏拖动、缩略图首帧 0 尺寸都不会触发 window resize）。
let themeObserver: MutationObserver | undefined
let sizeObserver: ResizeObserver | undefined

// --type-scale 随整页内容量变化（要点增删也会改变它），所以监听全部 props；
// flush: 'post' 等 SlideRenderer 把新的 --type-scale 写到 DOM 之后再读。
watch(() => props, render, { deep: true, flush: 'post' })
onMounted(() => {
  render()
  themeObserver = new MutationObserver(render)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme', 'style'] })
  sizeObserver = new ResizeObserver(() => chartInst?.resize())
  if (chartBox.value) sizeObserver.observe(chartBox.value)
})
onBeforeUnmount(() => {
  themeObserver?.disconnect()
  sizeObserver?.disconnect()
  chartInst?.dispose()
  chartInst = undefined
})
</script>

<template>
  <div class="section-num"><span v-html="renderRichText(section)"></span></div>

  <div relative z-10 flex flex-col h-full py-4 min-h-0>
    <div class="slide-header">
      <SlideTitle class="slide-title" :title="title" />
      <div class="slide-title-divider" />
    </div>
    <TagBadge v-if="subtitle" inline class="slide-subtitle mt-3"><span v-html="renderRichText(subtitle)"></span></TagBadge>

    <div flex-1 grid grid-cols-12 gap-5 mt-4 min-h-0>
      <ul class="cf-points col-span-5 flex flex-col justify-center gap-4 m-0 p-0 list-none">
        <li v-for="(p, i) in points" :key="i" flex items-start gap-3>
          <span v-if="p.step" class="cf-step slide-number" flex-shrink-0><span v-html="renderRichText(p.step)"></span></span>
          <span v-else class="cf-dot" flex-shrink-0 mt-2 />
          <span min-w-0>
            <span class="slide-card-title" font-bold block><span v-html="renderRichText(p.title)"></span></span>
            <span v-if="p.body" class="slide-text-caption slide-font-body" opacity-70 block mt-1><span v-html="renderRichText(p.body)"></span></span>
          </span>
        </li>
      </ul>

      <figure class="cf-chart-card glass-card col-span-7 flex flex-col m-0 p-5 min-h-0">
        <div font-bold class="slide-card-title" text="var(--primary)" mb-2><span v-html="renderRichText(chart.title)"></span></div>
        <div ref="chartBox" class="cf-plot" flex-1 min-h-0 min-w-0 />
        <figcaption class="cf-caption slide-text-caption slide-font-caption"><span v-html="renderRichText(chartSourceLabel(chart))"></span></figcaption>
      </figure>
    </div>

    <footer v-if="footer" class="cf-footer slide-footnote slide-text-caption" opacity-60 mt-auto pt-3><span v-html="renderRichText(footer)"></span></footer>
  </div>
</template>

<style scoped>
/* 颜色只引用全局 token 变量（主题切换自动跟随） */
/* 序号与要点标题同字号同行高，落在标题那一行上（原来没有字号类，固定 16px，不随 --type-scale 缩放） */
.cf-step {
  color: var(--primary);
  font-size: var(--type-card-title);
  line-height: 1.3;
}
.cf-dot {
  width: 8px;
  height: 8px;
  border-radius: 2px;
  background: var(--primary);
}
.cf-chart-card {
  background: var(--glass);
  border: 1px solid var(--glass-border);
  border-radius: 12px;
}
/* 字号来自 .slide-text-caption（--type-scale 体系），这里不写裸 font-size */
.cf-caption {
  color: var(--text-muted);
  padding-top: 8px;
  border-top: 1px solid var(--glass-border);
  margin-top: 8px;
}
.cf-footer {
  color: var(--text-muted);
}
</style>
