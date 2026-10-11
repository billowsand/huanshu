<script setup lang="ts">
import { computed, nextTick, ref, watch } from 'vue'
import { useSystemFontsStore } from '../stores/systemFonts'
import { DEFAULT_FONT, findSystemFont, fontFamily, matchesFont } from '../typography'

const props = defineProps<{ modelValue: string; label: string; sample: string; id: string; open: boolean; disabled?: boolean }>()
const emit = defineEmits<{ 'update:modelValue': [value: string]; 'update:open': [value: boolean]; 'apply-all': [value: string] }>()
const systemFonts = useSystemFontsStore()
const trigger = ref<HTMLButtonElement | null>(null)
const panel = ref<HTMLElement | null>(null)
const searchInput = ref<HTMLInputElement | null>(null)
const list = ref<HTMLElement | null>(null)
const query = ref('')
const activeIndex = ref(0)
const applyAll = ref(false)
const current = computed(() => findSystemFont(systemFonts.fonts, props.modelValue))
const currentLabel = computed(() => current.value?.label ?? (props.modelValue === DEFAULT_FONT ? '微软雅黑' : props.modelValue))
const missing = computed(() => systemFonts.loaded && !current.value)
const commonNames = computed(() => props.id === 'font-code'
  ? ['Consolas', 'Cascadia Code', 'JetBrains Mono', DEFAULT_FONT]
  : [DEFAULT_FONT, 'Microsoft YaHei UI', 'SimSun', 'SimHei', 'KaiTi', 'DengXian', 'MiSans', 'Arial', 'Georgia'])
const commonFonts = computed(() => commonNames.value.flatMap(name => {
  const font = findSystemFont(systemFonts.fonts, name)
  return font ? [font] : []
}))
const filtered = computed(() => {
  const matches = systemFonts.fonts.filter(font => matchesFont(font, query.value))
  if (query.value.trim()) return matches
  const priority = new Set(commonFonts.value.map(font => font.name))
  return [...commonFonts.value, ...matches.filter(font => !priority.has(font.name))]
})
const candidate = computed(() => filtered.value[activeIndex.value])
const previewName = computed(() => candidate.value?.name ?? props.modelValue)
function resetActive() {
  const selected = findSystemFont(filtered.value, props.modelValue)
  activeIndex.value = selected ? filtered.value.indexOf(selected) : 0
}
async function revealActive() {
  await nextTick()
  list.value?.querySelector<HTMLElement>(`[data-index="${activeIndex.value}"]`)?.scrollIntoView({ block: 'nearest' })
}
watch(filtered, () => { resetActive(); if (props.open) void revealActive() })
watch(() => props.open, async open => {
  if (!open) return
  query.value = ''
  applyAll.value = false
  resetActive()
  await nextTick()
  searchInput.value?.focus()
  await systemFonts.load()
  if (props.open) void revealActive()
})
function close() {
  emit('update:open', false)
  void nextTick(() => trigger.value?.focus())
}
function select() {
  if (!candidate.value || systemFonts.loading) return
  if (applyAll.value) emit('apply-all', candidate.value.name)
  else emit('update:modelValue', candidate.value.name)
  close()
}
function onSearchKeydown(event: KeyboardEvent) {
  if (event.key === 'ArrowDown' || event.key === 'ArrowUp' || event.key === 'Home' || event.key === 'End') {
    if (query.value && (event.key === 'Home' || event.key === 'End')) return
    event.preventDefault()
    activeIndex.value = event.key === 'Home' ? 0 : event.key === 'End' ? filtered.value.length - 1
      : Math.max(0, Math.min(filtered.value.length - 1, activeIndex.value + (event.key === 'ArrowDown' ? 1 : -1)))
    void revealActive()
  } else if (event.key === 'Enter' && !event.isComposing) {
    event.preventDefault()
    select()
  }
}
function onPanelKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    close()
  } else if (event.key === 'Tab') {
    const controls = panel.value?.querySelectorAll<HTMLElement>('button:not(:disabled):not([tabindex="-1"]), input:not(:disabled)')
    if (!controls?.length) return
    const first = controls[0]
    const last = controls[controls.length - 1]
    if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last?.focus() }
    else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first?.focus() }
  }
}
</script>

<template>
  <div class="system-font-picker">
    <button :id="id" ref="trigger" type="button" class="font-trigger" :disabled="disabled"
      :aria-label="`${label}：${currentLabel}`" aria-haspopup="dialog" :aria-expanded="open" :aria-controls="`${id}-panel`"
      @click="emit('update:open', !open)">
      <span class="font-trigger-name">{{ currentLabel }}</span><span v-if="modelValue === DEFAULT_FONT" class="font-default">默认</span>
      <span class="i-carbon:chevron-down" aria-hidden="true" />
    </button>
    <p v-if="missing" class="font-warning" role="status">此字体未安装，已回退到微软雅黑。</p>
    <Teleport to="body">
      <div v-if="open" class="font-overlay" @click.self="close">
        <section :id="`${id}-panel`" ref="panel" class="font-dialog" role="dialog" aria-modal="true" :aria-labelledby="`${id}-heading`" @keydown="onPanelKeydown">
          <header class="font-dialog-header">
            <div><h2 :id="`${id}-heading`">选择{{ label }}</h2><p>先预览，再应用到设置</p></div>
            <button type="button" class="font-icon-button" aria-label="关闭字体选择" @click="close"><span class="i-carbon:close" /></button>
          </header>
          <div class="font-browser">
            <div class="font-catalog">
              <div class="font-search">
                <span class="i-carbon:search" aria-hidden="true" />
                <input ref="searchInput" v-model="query" placeholder="搜索中文或英文名称" :aria-label="`搜索${label}`"
                  role="combobox" aria-autocomplete="list" aria-expanded="true" :aria-controls="`${id}-list`"
                  :aria-activedescendant="candidate ? `${id}-option-${activeIndex}` : undefined"
                  autocomplete="off" spellcheck="false" @keydown="onSearchKeydown" />
                <button v-if="query" class="font-icon-button" type="button" aria-label="清空搜索" @click="query = ''; searchInput?.focus()"><span class="i-carbon:close" /></button>
              </div>
              <div class="font-list-meta"><span>{{ query.trim() ? '搜索结果' : '常用优先 · 全部字体' }}</span><span>{{ filtered.length }}</span></div>
              <p v-if="systemFonts.error" class="font-warning" role="alert">{{ systemFonts.error }}</p>
              <div :id="`${id}-list`" ref="list" class="font-list" role="listbox" :aria-label="`${label}系统字体列表`" :aria-busy="systemFonts.loading">
                <p v-if="systemFonts.loading && !systemFonts.loaded" class="font-empty" role="status">正在读取系统字体…</p>
                <p v-else-if="!filtered.length" class="font-empty" role="status">{{ systemFonts.loaded ? '没有找到字体，试试其他名称。' : '未读取到字体，请刷新重试。' }}</p>
                <button v-for="(font, index) in filtered" :id="`${id}-option-${index}`" :key="font.name" type="button" role="option" tabindex="-1"
                  :disabled="systemFonts.loading" :aria-selected="activeIndex === index" :data-index="index" class="font-option"
                  :class="{ active: activeIndex === index }" :title="font.name" @click="activeIndex = index; searchInput?.focus()">
                  <span class="font-option-name">{{ font.label }}</span><span v-if="current?.name === font.name" class="font-current">当前</span>
                  <span class="font-check" aria-hidden="true">{{ activeIndex === index ? '✓' : '' }}</span>
                </button>
              </div>
              <div class="font-picker-footer"><span>系统字体</span>
                <button type="button" class="font-refresh" :disabled="systemFonts.loading" @click="systemFonts.load(true)"><span class="i-carbon:renew" />{{ systemFonts.loading ? '读取中…' : '刷新' }}</button>
              </div>
            </div>
            <div class="font-preview">
              <span class="font-eyebrow">效果预览</span><h3>{{ candidate?.label ?? currentLabel }}</h3>
              <p v-if="candidate && candidate.name !== candidate.label" class="font-alias">{{ candidate.name }}</p>
              <div class="font-specimen" :style="{ fontFamily: fontFamily(previewName) }"><p>{{ sample }}</p><p class="font-latin">Aa Bb Cc<br />0123456789</p></div>
              <p class="font-preview-note">预览当前用途的文字、中英文与数字</p>
            </div>
          </div>
          <footer class="font-dialog-actions">
            <label class="font-all-toggle"><input v-model="applyAll" type="checkbox" />所有用途使用此字体</label>
            <div class="font-actions"><button type="button" class="btn btn-ghost" @click="close">取消</button>
              <button type="button" class="btn btn-primary" :disabled="!candidate || systemFonts.loading" @click="select">{{ applyAll ? '应用到全部' : '应用字体' }}</button></div>
          </footer>
        </section>
      </div>
    </Teleport>
  </div>
</template>

<style scoped>
.system-font-picker { min-width: 0; }
.font-trigger { display: flex; align-items: center; gap: 10px; width: 100%; min-height: 40px; padding: 8px 12px; background: var(--studio-panel); border: 1px solid var(--studio-border); border-radius: 8px; color: var(--studio-text); font: inherit; font-size: 13px; text-align: left; cursor: pointer; }
.font-trigger:hover { border-color: var(--studio-primary-border); }
.font-trigger-name { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.font-trigger:disabled { opacity: 0.5; cursor: not-allowed; }
.font-default, .font-current { color: var(--studio-muted); font-size: 11px; flex-shrink: 0; }
.font-overlay { position: fixed; inset: var(--titlebar-height, 36px) 0 0; z-index: 1000; display: grid; place-items: center; padding: 24px; background: #0007; backdrop-filter: blur(4px); font-family: var(--font-sans); font-size: 13px; color: var(--studio-text); }
.font-overlay .font-option.active { box-shadow: none; }
.font-dialog { display: flex; flex-direction: column; width: min(760px, 100%); max-height: calc(100dvh - var(--titlebar-height, 36px) - 48px); overflow: hidden; border: 1px solid var(--studio-border); border-radius: 16px; background: var(--studio-panel); box-shadow: 0 24px 80px #0004; }
.font-dialog-header { display: flex; justify-content: space-between; align-items: center; padding: 20px 24px; border-bottom: 1px solid var(--studio-border); }
.font-dialog-header h2 { margin: 0; font-size: 17px; font-family: inherit; font-weight: 600; }
.font-dialog-header p { margin: 5px 0 0; color: var(--studio-muted); font-size: 12px; }
.font-icon-button { display: inline-flex; align-items: center; justify-content: center; flex-shrink: 0; width: 30px; height: 30px; border: 0; border-radius: 6px; background: transparent; color: var(--studio-muted); cursor: pointer; }
.font-icon-button:hover { background: var(--studio-surface); color: var(--studio-text); }
.font-browser { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); min-height: 0; overflow: auto; }
.font-catalog { display: flex; flex-direction: column; min-width: 0; min-height: 0; padding: 16px; border-right: 1px solid var(--studio-border); }
.font-search { display: flex; align-items: center; gap: 8px; flex-shrink: 0; padding: 0 10px; border: 1px solid var(--studio-border); border-radius: 8px; background: var(--studio-panel-2); }
.font-search:focus-within { border-color: var(--studio-primary); }
.font-search input { width: 100%; min-width: 0; height: 38px; padding: 0; border: 0; border-radius: 0; background: transparent; font: inherit; box-shadow: none; outline: none; }
.font-search input:focus-visible { outline: none; box-shadow: none; }
.font-list-meta { display: flex; justify-content: space-between; padding: 14px 4px 8px; color: var(--studio-muted); font-size: 11px; }
.font-list { height: clamp(160px, 40dvh, 340px); overflow-y: auto; overscroll-behavior: contain; scrollbar-gutter: stable; }
.font-option { width: 100%; display: flex; gap: 8px; align-items: center; min-height: 38px; padding: 8px 10px; color: var(--studio-text); background: transparent; border: 0; border-radius: 6px; text-align: left; font: inherit; cursor: pointer; }
.font-option:hover { background: var(--studio-surface); }
.font-option.active { background: var(--studio-primary-bg); color: var(--studio-primary); }
.font-option:disabled { opacity: 0.5; cursor: wait; }
.font-option-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.font-check { width: 14px; flex-shrink: 0; color: var(--studio-primary); }
.font-preview { display: flex; flex-direction: column; min-width: 0; padding: 28px; background: var(--studio-panel-2); }
.font-eyebrow { color: var(--studio-muted); font-size: 11px; }
.font-preview h3 { margin: 12px 0 4px; font: inherit; font-size: 15px; overflow-wrap: anywhere; }
.font-alias { color: var(--studio-muted); font-size: 11px; overflow-wrap: anywhere; }
.font-specimen { flex: 1; display: flex; flex-direction: column; justify-content: center; gap: 24px; padding-block: 28px; font-size: 26px; line-height: 1.7; overflow-wrap: anywhere; }
.font-latin { font-size: 22px; }
.font-preview-note { color: var(--studio-muted); font-size: 11px; }
.font-picker-footer { display: flex; align-items: center; justify-content: space-between; padding: 10px 4px 0; color: var(--studio-muted); font-size: 11px; }
.font-refresh { display: flex; align-items: center; gap: 5px; padding: 4px; background: transparent; border: 0; color: var(--studio-muted); font: inherit; cursor: pointer; }
.font-dialog-actions { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 16px 24px; border-top: 1px solid var(--studio-border); }
.font-all-toggle { display: flex; align-items: center; gap: 8px; color: var(--studio-muted); font-size: 12px; cursor: pointer; }
.font-all-toggle input { width: 14px; height: 14px; margin: 0; accent-color: var(--studio-primary); }
.font-actions { display: flex; gap: 8px; flex-shrink: 0; }
.font-empty { padding: 24px 10px; color: var(--studio-muted); line-height: 1.6; }
.font-warning { margin-top: 6px; font-size: 12px; line-height: 1.5; color: var(--studio-primary); overflow-wrap: anywhere; }
.font-dialog button:focus-visible, .font-trigger:focus-visible { outline: 2px solid var(--studio-primary); outline-offset: 2px; }
.font-dialog-header, .font-dialog-actions { flex-shrink: 0; }
.font-browser { flex: 1; }
@media (max-width: 600px) {
  .font-overlay { padding: 12px; }
  .font-dialog { height: calc(100dvh - var(--titlebar-height, 36px) - 24px); max-height: calc(100dvh - var(--titlebar-height, 36px) - 24px); }
  .font-dialog-header { padding: 16px; }
  .font-browser { grid-template-columns: 1fr; }
  .font-catalog { border-right: 0; }
  .font-list { height: 180px; }
  .font-preview { padding: 16px 20px; border-top: 1px solid var(--studio-border); }
  .font-specimen { padding-block: 12px; gap: 8px; font-size: 22px; }
  .font-latin { font-size: 18px; }
  .font-preview-note { display: none; }
  .font-dialog-actions { padding: 12px 16px; flex-wrap: wrap; }
  .font-actions { margin-left: auto; }
}
</style>
