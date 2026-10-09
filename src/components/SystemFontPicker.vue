<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useSystemFontsStore } from '../stores/systemFonts'
import { DEFAULT_FONT, findSystemFont, fontFamily, matchesFont, type SystemFont } from '../typography'

const props = defineProps<{
  modelValue: string
  label: string
  sample: string
  id: string
  open: boolean
  disabled?: boolean
}>()
const emit = defineEmits<{
  'update:modelValue': [value: string]
  'update:open': [value: boolean]
  'apply-all': [value: string]
}>()
const systemFonts = useSystemFontsStore()
const root = ref<HTMLElement | null>(null)
const trigger = ref<HTMLButtonElement | null>(null)
const searchInput = ref<HTMLInputElement | null>(null)
const list = ref<HTMLElement | null>(null)
const query = ref('')
const activeIndex = ref(0)
const current = computed(() => findSystemFont(systemFonts.fonts, props.modelValue))
const currentLabel = computed(() => current.value?.label ?? (props.modelValue === DEFAULT_FONT ? '微软雅黑' : props.modelValue))
const missing = computed(() => systemFonts.loaded && !current.value)
const filtered = computed(() => systemFonts.fonts.filter(font => matchesFont(font, query.value)))
const candidate = computed(() => filtered.value[activeIndex.value])
const previewName = computed(() => candidate.value?.name ?? props.modelValue)

function resetActive() {
  const selected = findSystemFont(filtered.value, props.modelValue)
  activeIndex.value = selected ? filtered.value.indexOf(selected) : 0
}

async function revealActive() {
  await nextTick()
  const option = list.value?.querySelector<HTMLElement>(`[data-index="${activeIndex.value}"]`)
  option?.scrollIntoView({ block: 'nearest' })
}

watch(filtered, () => { resetActive(); if (props.open) void revealActive() })
watch(() => props.open, async open => {
  if (!open) return
  query.value = ''
  resetActive()
  await nextTick()
  searchInput.value?.focus()
  await systemFonts.load()
  if (props.open) void revealActive()
})

function close(restoreFocus = false) {
  emit('update:open', false)
  if (restoreFocus) void nextTick(() => trigger.value?.focus())
}

function select(font: SystemFont, all = false) {
  if (all) emit('apply-all', font.name)
  else emit('update:modelValue', font.name)
  close(true)
}

function onKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    close(true)
  } else if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
    event.preventDefault()
    const delta = event.key === 'ArrowDown' ? 1 : -1
    activeIndex.value = Math.max(0, Math.min(filtered.value.length - 1, activeIndex.value + delta))
    void revealActive()
  } else if (event.key === 'Enter' && candidate.value) {
    event.preventDefault()
    select(candidate.value)
  }
}

function onOutsidePointer(event: PointerEvent) {
  if (props.open && !root.value?.contains(event.target as Node)) close()
}
function onFocusOut(event: FocusEvent) {
  if (props.open && event.relatedTarget && !root.value?.contains(event.relatedTarget as Node)) close()
}
onMounted(() => document.addEventListener('pointerdown', onOutsidePointer))
onBeforeUnmount(() => document.removeEventListener('pointerdown', onOutsidePointer))
</script>

<template>
  <div ref="root" class="system-font-picker" @focusout="onFocusOut">
    <button :id="id" ref="trigger" type="button" class="font-trigger"
      :disabled="disabled" :aria-label="`${label}：${currentLabel}`" aria-haspopup="dialog"
      :aria-expanded="open" :aria-controls="`${id}-panel`" @click="emit('update:open', !open)">
      <span>{{ currentLabel }}<small v-if="modelValue === DEFAULT_FONT">（默认）</small></span>
      <span class="i-carbon:chevron-down" :class="{ expanded: open }" />
    </button>
    <p v-if="missing" class="font-warning" role="status">「{{ modelValue }}」未安装或已卸载，当前回退到微软雅黑。</p>
    <div class="font-sample" :style="{ fontFamily: fontFamily(modelValue) }">{{ sample }}</div>

    <div v-if="open" :id="`${id}-panel`" class="font-popover" role="dialog" :aria-label="`选择${label}`" @keydown.esc.stop.prevent="close(true)">
      <div class="font-search">
        <span class="i-carbon:search" aria-hidden="true" />
        <input ref="searchInput" v-model="query" placeholder="搜索系统字体（中文 / English）"
          :aria-label="`搜索${label}`" role="combobox" aria-autocomplete="list" aria-expanded="true"
          :aria-controls="`${id}-list`" :aria-activedescendant="candidate ? `${id}-option-${activeIndex}` : undefined"
          autocomplete="off" spellcheck="false" @keydown="onKeydown" />
      </div>
      <p v-if="systemFonts.error" class="font-warning" role="alert">{{ systemFonts.error }}</p>
      <div :id="`${id}-list`" ref="list" class="font-list" role="listbox" :aria-label="`${label}系统字体列表`" :aria-busy="systemFonts.loading">
        <p v-if="systemFonts.loading && !systemFonts.loaded" class="font-empty" role="status">正在加载系统字体…</p>
        <p v-else-if="!filtered.length" class="font-empty" role="status">{{ systemFonts.loaded ? '没有匹配的字体，试试中文名或英文名。' : '未加载系统字体，请点击刷新重试。' }}</p>
        <button v-for="(font, index) in filtered" :id="`${id}-option-${index}`" :key="font.name"
          type="button" role="option" tabindex="-1" :disabled="systemFonts.loading" :aria-selected="current?.name === font.name"
          :data-index="index" class="font-option" :class="{ active: activeIndex === index }"
          @mouseenter="activeIndex = index" @click="select(font)">
          <span class="font-check" aria-hidden="true">{{ current?.name === font.name ? '✓' : '' }}</span>
          <span class="font-option-name">{{ font.label }}<small v-if="font.name !== font.label">{{ font.name }}</small></span>
        </button>
      </div>
      <div class="font-preview" aria-live="polite">
        <p class="hint">字体预览 · {{ candidate?.label ?? currentLabel }}</p>
        <div :style="{ fontFamily: fontFamily(previewName) }">{{ sample }}<br />Aa Bb 0123456789</div>
      </div>
      <div class="font-picker-footer">
        <span>{{ systemFonts.loaded ? `系统已加载 ${systemFonts.fonts.length} 种字体` : '系统字体' }}</span>
        <button type="button" class="btn btn-ghost" :disabled="systemFonts.loading" @click="systemFonts.load(true)">
          <span class="i-carbon:renew" />{{ systemFonts.loading ? '加载中' : '刷新' }}
        </button>
      </div>
      <button type="button" class="btn font-apply-all" :disabled="!candidate || systemFonts.loading" @click="candidate && select(candidate, true)">全部使用此字体</button>
    </div>
  </div>
</template>

<style scoped>
.system-font-picker { display: flex; flex-direction: column; gap: 0.5rem; min-width: 0; }
.font-trigger { display: flex; align-items: center; justify-content: space-between; gap: 0.5rem; width: 100%; padding: 0.55rem 0.65rem; background: var(--studio-surface); border: 1px solid var(--studio-border); border-radius: 6px; color: var(--studio-text); font: inherit; font-size: 0.85rem; text-align: left; cursor: pointer; }
.font-trigger:disabled { opacity: 0.5; cursor: not-allowed; }
.font-trigger small { color: var(--studio-muted); }
.font-trigger:focus-visible, .font-option:focus-visible { outline: 2px solid var(--studio-primary); outline-offset: 2px; }
.expanded { transform: rotate(180deg); }
.font-sample { padding: 0.65rem; min-height: 3.5rem; line-height: 1.6; overflow-wrap: anywhere; background: var(--studio-surface); border-radius: 6px; }
.font-popover { padding: 0.65rem; border: 1px solid var(--studio-primary-border); border-radius: 8px; background: var(--studio-panel); display: flex; flex-direction: column; gap: 0.5rem; }
.font-search { display: flex; align-items: center; gap: 0.4rem; }
.font-search input { min-width: 0; }
.font-list { max-height: 220px; overflow-y: auto; border-block: 1px solid var(--studio-border); padding-block: 0.25rem; }
.font-option { width: 100%; display: flex; gap: 0.5rem; align-items: center; padding: 0.45rem; color: var(--studio-text); background: transparent; border: 0; border-radius: 4px; text-align: left; font: inherit; font-size: 0.82rem; cursor: pointer; }
.font-option.active { background: var(--studio-primary-bg); }
.font-option:disabled { opacity: 0.5; cursor: wait; }
.font-check { width: 1rem; flex-shrink: 0; color: var(--studio-primary); }
.font-option-name { min-width: 0; overflow-wrap: anywhere; }
.font-option-name small { display: block; color: var(--studio-muted); font-size: 0.7rem; }
.font-preview { padding: 0.5rem; border-radius: 4px; background: var(--studio-surface); line-height: 1.6; overflow-wrap: anywhere; }
.hint, .font-picker-footer { color: var(--studio-muted); font-size: 0.7rem; }
.font-picker-footer { display: flex; align-items: center; justify-content: space-between; gap: 0.25rem; }
.font-apply-all { justify-content: center; }
.font-empty { padding: 1rem 0.5rem; color: var(--studio-muted); font-size: 0.8rem; line-height: 1.6; }
.font-warning { font-size: 0.75rem; line-height: 1.5; color: var(--studio-primary); overflow-wrap: anywhere; }
</style>
