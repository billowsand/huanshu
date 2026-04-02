<script setup lang="ts">
import { computed } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'
import type { SlideBlueprint } from './types'

import KeynoteCoverSlide from './KeynoteCoverSlide.vue'
import KeynoteClosingSlide from './KeynoteClosingSlide.vue'
import KeynoteOverviewSlide from './KeynoteOverviewSlide.vue'
import KeynoteSectionIntroSlide from './KeynoteSectionIntroSlide.vue'
import KeynoteFeatureGridSlide from './KeynoteFeatureGridSlide.vue'
import KeynoteSpotlightSlide from './KeynoteSpotlightSlide.vue'
import KeynoteSplitLayersSlide from './KeynoteSplitLayersSlide.vue'
import KeynoteSectionListSlide from './KeynoteSectionListSlide.vue'
import KeynoteFocusExampleSlide from './KeynoteFocusExampleSlide.vue'
import KeynoteOutcomeGridSlide from './KeynoteOutcomeGridSlide.vue'
import KeynoteCenterGridSlide from './KeynoteCenterGridSlide.vue'
import KeynoteTimelineSlide from './KeynoteTimelineSlide.vue'
import KeynoteStepFlowSlide from './KeynoteStepFlowSlide.vue'
import KeynoteProcessSlide from './KeynoteProcessSlide.vue'
import KeynoteCompareSlide from './KeynoteCompareSlide.vue'
import KeynoteIssueStackSlide from './KeynoteIssueStackSlide.vue'
import KeynoteSwotSlide from './KeynoteSwotSlide.vue'
import KeynoteInfographicSlide from './KeynoteInfographicSlide.vue'

const props = defineProps<{
  slide: SlideBlueprint
  slideIndex?: number
  mediaMap?: Record<string, string>
}>()

const componentMap: Record<string, unknown> = {
  cover: KeynoteCoverSlide,
  closing: KeynoteClosingSlide,
  overview: KeynoteOverviewSlide,
  section_intro: KeynoteSectionIntroSlide,
  feature_grid: KeynoteFeatureGridSlide,
  spotlight: KeynoteSpotlightSlide,
  split_layers: KeynoteSplitLayersSlide,
  section_list: KeynoteSectionListSlide,
  focus_example: KeynoteFocusExampleSlide,
  outcome_grid: KeynoteOutcomeGridSlide,
  center_grid: KeynoteCenterGridSlide,
  timeline: KeynoteTimelineSlide,
  step_flow: KeynoteStepFlowSlide,
  process: KeynoteProcessSlide,
  compare: KeynoteCompareSlide,
  issue_stack: KeynoteIssueStackSlide,
  swot: KeynoteSwotSlide,
  infographic: KeynoteInfographicSlide,
}

const component = computed(() => componentMap[props.slide.kind] ?? KeynoteCoverSlide)

type AmbientPreset = {
  primary: {
    top: string
    left: string
    width: string
    height: string
    opacity: number
  }
  secondary: {
    top?: string
    left?: string
    right?: string
    bottom?: string
    width: string
    height: string
    opacity: number
  }
  tertiary?: {
    top?: string
    left?: string
    right?: string
    bottom?: string
    width: string
    height: string
    opacity: number
  }
}

const ambientPresets: AmbientPreset[] = [
  {
    primary: { top: '38px', left: '52px', width: '260px', height: '220px', opacity: 0.4 },
    secondary: { top: '56px', right: '74px', width: '180px', height: '180px', opacity: 0.26 },
    tertiary: { bottom: '52px', right: '138px', width: '220px', height: '140px', opacity: 0.18 },
  },
  {
    primary: { top: '96px', left: '78px', width: '210px', height: '210px', opacity: 0.32 },
    secondary: { bottom: '64px', right: '82px', width: '260px', height: '180px', opacity: 0.24 },
    tertiary: { top: '42px', right: '210px', width: '140px', height: '140px', opacity: 0.12 },
  },
  {
    primary: { top: '44px', left: '140px', width: '320px', height: '170px', opacity: 0.28 },
    secondary: { bottom: '46px', left: '86px', width: '180px', height: '180px', opacity: 0.18 },
    tertiary: { top: '86px', right: '76px', width: '200px', height: '220px', opacity: 0.2 },
  },
  {
    primary: { top: '122px', left: '54px', width: '240px', height: '240px', opacity: 0.26 },
    secondary: { top: '36px', right: '70px', width: '260px', height: '150px', opacity: 0.2 },
    tertiary: { bottom: '48px', right: '118px', width: '190px', height: '190px', opacity: 0.16 },
  },
]

const kindPresetMap: Partial<Record<SlideBlueprint['kind'], number[]>> = {
  cover: [0],
  closing: [0, 3],
  overview: [2],
  section_intro: [0, 1],
  feature_grid: [1, 2],
  spotlight: [3, 0],
  split_layers: [2, 3],
  section_list: [1],
  focus_example: [3, 1],
  outcome_grid: [1, 0],
  center_grid: [0, 2],
  timeline: [2, 3],
  step_flow: [1, 2],
  process: [3, 1],
  compare: [0, 3],
  issue_stack: [3, 2],
  swot: [2, 0],
  infographic: [1, 3],
}

function hashString(input: string) {
  let hash = 0
  for (let i = 0; i < input.length; i += 1) {
    hash = (hash * 31 + input.charCodeAt(i)) >>> 0
  }
  return hash
}

const ambientPreset = computed(() => {
  const pool = kindPresetMap[props.slide.kind] ?? [0, 1, 2, 3]
  const hashSeed = `${props.slide.kind}:${props.slide.title}:${props.slide.section ?? ''}:${props.slideIndex ?? 0}`
  return ambientPresets[pool[hashString(hashSeed) % pool.length]]
})

const shellStyle = computed(() => ({
  '--ambient-primary-top': ambientPreset.value.primary.top,
  '--ambient-primary-left': ambientPreset.value.primary.left,
  '--ambient-primary-width': ambientPreset.value.primary.width,
  '--ambient-primary-height': ambientPreset.value.primary.height,
  '--ambient-primary-opacity': String(ambientPreset.value.primary.opacity),
  '--ambient-secondary-top': ambientPreset.value.secondary.top ?? 'auto',
  '--ambient-secondary-left': ambientPreset.value.secondary.left ?? 'auto',
  '--ambient-secondary-right': ambientPreset.value.secondary.right ?? 'auto',
  '--ambient-secondary-bottom': ambientPreset.value.secondary.bottom ?? 'auto',
  '--ambient-secondary-width': ambientPreset.value.secondary.width,
  '--ambient-secondary-height': ambientPreset.value.secondary.height,
  '--ambient-secondary-opacity': String(ambientPreset.value.secondary.opacity),
  '--ambient-tertiary-top': ambientPreset.value.tertiary?.top ?? 'auto',
  '--ambient-tertiary-left': ambientPreset.value.tertiary?.left ?? 'auto',
  '--ambient-tertiary-right': ambientPreset.value.tertiary?.right ?? 'auto',
  '--ambient-tertiary-bottom': ambientPreset.value.tertiary?.bottom ?? 'auto',
  '--ambient-tertiary-width': ambientPreset.value.tertiary?.width ?? '0px',
  '--ambient-tertiary-height': ambientPreset.value.tertiary?.height ?? '0px',
  '--ambient-tertiary-opacity': String(ambientPreset.value.tertiary?.opacity ?? 0),
}))

const compareFallbackLeft = {
  title: '左侧',
  tone: 'blue',
  items: [],
}

const compareFallbackRight = {
  title: '右侧',
  tone: 'amber',
  items: [],
}

const MEDIA_REF_RE = /^media:(\d+)$/

function resolveMediaRef(value?: string): string | undefined {
  if (!value)
    return value
  const match = value.match(MEDIA_REF_RE)
  if (match)
    return props.mediaMap?.[match[1]]
  if (/^(asset:|blob:|data:|https?:)/.test(value))
    return value
  if (/^(\/|[A-Za-z]:[\\/])/.test(value))
    return convertFileSrc(value)
  return value
}

function resolveMediaRefs(values?: string[]): string[] {
  return (values ?? [])
    .map(resolveMediaRef)
    .filter((value): value is string => Boolean(value))
}

const slideProps = computed(() => {
  const s = props.slide
  switch (s.kind) {
    case 'cover':
      return { badges: s.badges, title: s.title, subtitle: s.subtitle, enTitle: '' }
    case 'closing':
      return { badges: s.badges, title: s.title, subtitle: s.subtitle, note: s.note ?? '' }
    case 'overview':
      return { section: s.section ?? '00', title: s.title, items: s.overview_items ?? [], note: s.note ?? '' }
    case 'section_intro':
      return {
        section: s.section ?? '',
        badge: s.badge ?? '章节导览',
        title: s.title,
        subtitle: s.subtitle ?? '',
        cards: s.cards ?? [],
        note: s.note ?? '',
      }
    case 'feature_grid':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        cols: Math.min(4, (s.cards ?? []).length) || 1,
        cards: s.cards ?? [],
        note: s.note ?? '',
      }
    case 'spotlight':
      return {
        label: s.label ?? '章节细化',
        labelTone: s.label_tone ?? 'blue',
        title: s.title,
        image: resolveMediaRef(s.image),
        images: resolveMediaRefs(s.images),
        placeholder: s.placeholder,
        sideWidth: s.side_width,
        panels: s.panels ?? [],
      }
    case 'split_layers':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        leftItems: s.left_items ?? [],
        layers: s.layers ?? [],
        footer: s.footer ?? '',
      }
    case 'section_list':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        items: s.list_items ?? [],
      }
    case 'focus_example':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        points: s.points ?? [],
        exampleTitle: s.example_title ?? '类比',
        exampleBody: s.example_body ?? '',
        ranking: s.ranking ?? [],
      }
    case 'outcome_grid':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        cards: s.cards ?? [],
        note: s.note ?? '',
      }
    case 'center_grid':
      return {
        badge: s.badge ?? '总结',
        title: s.title,
        accent: s.accent ?? '',
        cols: Math.min(4, (s.center_items ?? []).length) || 1,
        items: s.center_items ?? [],
        footer: s.footer ?? '',
      }
    case 'timeline':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        events: s.timeline_events ?? [],
        footer: s.footer ?? '',
      }
    case 'step_flow':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        direction: s.direction ?? 'horizontal',
        steps: s.steps ?? [],
        footer: s.footer ?? '',
      }
    case 'process':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        phases: s.phases ?? [],
        footer: s.footer ?? '',
      }
    case 'compare':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        mode: s.compare_data?.mode ?? 'side-by-side',
        left: s.compare_data?.left ?? compareFallbackLeft,
        right: s.compare_data?.right ?? compareFallbackRight,
        footer: s.footer ?? '',
      }
    case 'issue_stack':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        cards: s.cards ?? [],
        footer: s.footer ?? '',
      }
    case 'swot':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        quadrants: s.swot_data?.quadrants ?? [],
        strategy: s.swot_data?.strategy ?? '',
      }
    case 'infographic':
      return {
        section: s.section ?? '',
        title: s.title,
        subtitle: s.subtitle ?? '',
        infographicSyntax: s.infographic_syntax ?? '',
        footer: s.footer ?? '',
      }
    default:
      return { title: s.title }
  }
})
</script>

<template>
  <div class="slide-shell" :style="shellStyle">
    <div class="slide-shell__ambient slide-shell__ambient--left" />
    <div class="slide-shell__ambient slide-shell__ambient--right" />
    <div class="slide-shell__ambient slide-shell__ambient--bottom" />
    <div class="slide-shell__frame">
      <component :is="component" v-bind="slideProps" />
    </div>
  </div>
</template>

<style scoped>
.slide-shell {
  position: relative;
  width: 100%;
  height: 100%;
  padding: 18px 20px;
  background:
    radial-gradient(circle at 12% 18%, color-mix(in srgb, var(--primary) 10%, transparent), transparent 30%),
    radial-gradient(circle at 84% 12%, color-mix(in srgb, var(--info) 10%, transparent), transparent 26%),
    linear-gradient(180deg, color-mix(in srgb, var(--glass) 28%, transparent), transparent 32%),
    var(--bg-gradient, var(--bg));
  box-sizing: border-box;
}

.slide-shell__ambient {
  position: absolute;
  border-radius: 999px;
  filter: blur(42px);
  opacity: 0.45;
  pointer-events: none;
}

.slide-shell__ambient--left {
  top: var(--ambient-primary-top);
  left: var(--ambient-primary-left);
  width: var(--ambient-primary-width);
  height: var(--ambient-primary-height);
  background: color-mix(in srgb, var(--primary) 16%, transparent);
  opacity: var(--ambient-primary-opacity);
}

.slide-shell__ambient--right {
  top: var(--ambient-secondary-top);
  left: var(--ambient-secondary-left);
  right: var(--ambient-secondary-right);
  bottom: var(--ambient-secondary-bottom);
  width: var(--ambient-secondary-width);
  height: var(--ambient-secondary-height);
  background: color-mix(in srgb, var(--info) 14%, transparent);
  opacity: var(--ambient-secondary-opacity);
}

.slide-shell__ambient--bottom {
  top: var(--ambient-tertiary-top);
  left: var(--ambient-tertiary-left);
  right: var(--ambient-tertiary-right);
  bottom: var(--ambient-tertiary-bottom);
  width: var(--ambient-tertiary-width);
  height: var(--ambient-tertiary-height);
  background: color-mix(in srgb, var(--primary-light) 12%, transparent);
  opacity: var(--ambient-tertiary-opacity);
}

.slide-shell__frame {
  position: relative;
  z-index: 1;
  width: 100%;
  height: 100%;
  padding: 30px 36px 32px;
  border-radius: 20px;
  box-sizing: border-box;
  background: linear-gradient(180deg, color-mix(in srgb, var(--glass) 32%, transparent), transparent 18%);
  overflow: hidden;
}
</style>
