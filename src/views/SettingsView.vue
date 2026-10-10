<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { invoke } from '@tauri-apps/api/core'
import { useConfigStore, type ModelTarget } from '../stores/config'
import { useAppSettingsStore } from '../stores/appSettings'
import { useSystemFontsStore } from '../stores/systemFonts'
import { applyFonts, fontFamily, fontRoles, normalizeFonts, type FontRole } from '../typography'
import SystemFontPicker from '../components/SystemFontPicker.vue'
import BrandMark from '../components/BrandMark.vue'
import { version } from '../../package.json'

const router = useRouter()
const config = useConfigStore()
const appSettings = useAppSettingsStore()
const systemFonts = useSystemFontsStore()
const clone = <T,>(value: T): T => JSON.parse(JSON.stringify(value))
const sections = [
  { key: 'appearance', title: '外观与字体', icon: 'i-carbon:screen', desc: '自定义工作环境与演示文稿的文字风格。' },
  { key: 'models', title: '模型服务', icon: 'i-carbon:cube', desc: '为生成、检索与图片理解配置独立服务。' },
  { key: 'generation', title: '生成设置', icon: 'i-carbon:settings-adjust', desc: '控制生成效率与自动修复行为。' },
  { key: 'storage', title: '存储与缓存', icon: 'i-carbon:data-base', desc: '查看文件位置，管理图标检索的向量缓存。' },
  { key: 'about', title: '关于与帮助', icon: 'i-carbon:help', desc: '从 Markdown 到演示文稿，让想法清晰呈现。' },
] as const
const activeSection = ref<typeof sections[number]['key']>('appearance')
const section = computed(() => sections.find(item => item.key === activeSection.value)!)
const ready = ref(false)
const loading = ref(true)
const saving = ref(false)
const saveError = ref('')
const saved = ref(false)
let savedTimer: ReturnType<typeof setTimeout> | undefined
const draft = ref(clone(config.settings))
const baseline = ref(clone(config.settings))
const fontDraft = ref(normalizeFonts(appSettings.settings.fonts))
const fontBaseline = ref(normalizeFonts(appSettings.settings.fonts))
const themeBaseline = ref(document.documentElement.getAttribute('data-theme') || 'dark-yellow')
const themeDraft = ref(themeBaseline.value)
const activeFontRole = ref<FontRole | null>(null)
const previewTab = ref<'slides' | 'interface'>('slides')
const previewRole = ref<FontRole>('body')
const unified = ref(false)
const configDirty = computed(() => JSON.stringify(draft.value) !== JSON.stringify(baseline.value))
const fontsDirty = computed(() => JSON.stringify(fontDraft.value) !== JSON.stringify(fontBaseline.value))
const dirty = computed(() => configDirty.value || fontsDirty.value || themeDraft.value !== themeBaseline.value)
const themeTiles = [
  { id: 'light-terminal', label: '浅色', kind: 'light' },
  { id: 'dark-terminal', label: '深色', kind: 'dark' },
  { id: 'light-notes', label: '纸感', kind: 'paper' },
]
const isDarkTheme = computed(() => themeDraft.value.startsWith('dark'))
const darkThemes = [
  { id: 'dark-terminal', label: '终端 · 墨' }, { id: 'dark-yellow', label: '暗 · 琥珀' },
  { id: 'dark-blue', label: '暗 · 星图' }, { id: 'dark-glass-blue', label: '暗 · 极光' },
]
const roleLabels: Record<FontRole, string> = { interface: '界面', title: '标题', subtitle: '副标题', body: '正文', caption: '注释与标签', number: '数字与序号', code: '代码与编辑器' }
const roleDescriptions: Record<FontRole, string> = { interface: '菜单、按钮与表单', title: '封面与章节标题', subtitle: '副标题与章节说明', body: '演示正文与内容卡片', caption: '脚注、标签与辅助信息', number: '编号、序号与时间轴', code: 'Markdown 编辑器与代码块' }
function setFont(role: FontRole, value: string) {
  if (unified.value) applyFontToAll(value)
  else fontDraft.value[role] = value
  previewRole.value = role
}
function applyFontToAll(value: string) {
  fontDraft.value = normalizeFonts(Object.fromEntries(fontRoles.map(role => [role.key, value])))
}
function toggleUnified() {
  if (unified.value) applyFontToAll(fontDraft.value.body)
}
function resetFonts() {
  activeFontRole.value = null
  fontDraft.value = normalizeFonts()
}
watch(fontDraft, value => { if (ready.value) applyFonts(value) }, { deep: true, flush: 'sync' })
watch(themeDraft, value => { if (ready.value) document.documentElement.setAttribute('data-theme', value) })
watch(activeSection, () => { activeFontRole.value = null })
watch(dirty, () => { saved.value = false })

const services = [
  { key: 'llm', title: '大语言模型', subtitle: '文本生成', icon: 'i-carbon:chat', uses: ['Markdown 内容优化', '页面规划与布局', '幻灯片正文生成'], hint: '选择支持文本生成的指令模型。' },
  { key: 'embedding', title: '向量模型', subtitle: '图标检索', icon: 'i-carbon:data-base', uses: ['图标语义匹配', '内容向量化', '向量缓存初始化'], hint: '需要支持 /v1/embeddings 接口。' },
  { key: 'multimodal', title: '多模态模型', subtitle: '图片理解', icon: 'i-carbon:image', uses: ['图片内容分析', '图片自动描述', '图文内容理解'], hint: '选择支持图像输入的视觉模型。' },
] as const
const activeService = ref<ModelTarget>('llm')
const serviceInfo = computed(() => services.find(item => item.key === activeService.value)!)
const serviceDraft = computed(() => draft.value[activeService.value])
const modelLists = ref<Record<ModelTarget, string[]>>({ llm: [], embedding: [], multimodal: [] })
const testing = ref<Record<ModelTarget, boolean>>({ llm: false, embedding: false, multimodal: false })
const results = ref<Partial<Record<ModelTarget, { signature: string; ok: boolean; elapsed: number; error: string }>>>({})
const manualModel = ref<Record<ModelTarget, boolean>>({ llm: false, embedding: false, multimodal: false })
const showKey = ref(false)
const modelOptions = computed(() => [...new Set([serviceDraft.value.model, ...modelLists.value[activeService.value]])].filter(Boolean))
function serviceStatus(target: ModelTarget) {
  if (testing.value[target]) return '测试中'
  const result = results.value[target]
  if (result?.signature === JSON.stringify(draft.value[target])) return result.ok ? '已连接' : '连接失败'
  return draft.value[target].base_url.trim() && draft.value[target].model.trim() ? '待测试' : '未配置'
}
const activeResult = computed(() => {
  const result = results.value[activeService.value]
  return result?.signature === JSON.stringify(serviceDraft.value) ? result : undefined
})
const serviceLocation = computed(() => {
  try { return ['localhost', '127.0.0.1', '[::1]'].includes(new URL(serviceDraft.value.base_url).hostname) ? '本地服务' : '远程服务' }
  catch { return '地址未填写' }
})
watch(activeService, () => { showKey.value = false })
async function testConnection(target: ModelTarget) {
  if (testing.value[target]) return
  testing.value[target] = true
  const service = clone(draft.value[target])
  const started = performance.now()
  try {
    const models = await config.listModels(target, service)
    modelLists.value[target] = [...new Set(models)]
    results.value[target] = { signature: JSON.stringify(service), ok: true, elapsed: Math.round(performance.now() - started), error: '' }
  } catch (error) {
    results.value[target] = { signature: JSON.stringify(service), ok: false, elapsed: 0, error: String(error) }
  } finally { testing.value[target] = false }
}

const initializing = ref(false)
const embeddingMessage = ref('')
const embeddingReady = computed(() => appSettings.settings.embeddings_ready && !!draft.value.embedding.model.trim()
  && draft.value.embedding.model.trim() === appSettings.settings.initialized_embedding_model.trim())
const embeddingStatus = computed(() => embeddingReady.value ? '已初始化' : appSettings.settings.embeddings_ready ? '模型已变更' : '未初始化')
async function initEmbeddings() {
  if (initializing.value || configDirty.value) return
  initializing.value = true
  embeddingMessage.value = '正在初始化图标向量缓存…'
  try {
    await invoke('ensure_icon_embeddings')
    await appSettings.load()
    applyFonts(fontDraft.value)
    embeddingMessage.value = '向量缓存初始化完成。'
  } catch (error) { embeddingMessage.value = String(error) }
  finally { initializing.value = false }
}
async function loadSettings() {
  loading.value = true
  await Promise.all([config.load(), appSettings.load()])
  if (config.loaded && appSettings.loaded) {
    draft.value = clone(config.settings)
    baseline.value = clone(config.settings)
    fontDraft.value = normalizeFonts(appSettings.settings.fonts)
    fontBaseline.value = normalizeFonts(appSettings.settings.fonts)
    ready.value = true
  }
  loading.value = false
}
function revert() {
  draft.value = clone(baseline.value)
  fontDraft.value = normalizeFonts(fontBaseline.value)
  themeDraft.value = themeBaseline.value
  unified.value = false
  activeFontRole.value = null
  saveError.value = ''
}
async function save() {
  if (saving.value || !ready.value || !dirty.value) return
  if (!Number.isInteger(draft.value.repair_rounds) || draft.value.repair_rounds < 0 || draft.value.repair_rounds > 10
    || !Number.isInteger(draft.value.concurrency) || draft.value.concurrency < 1 || draft.value.concurrency > 8) {
    saveError.value = '修复轮数需为 0–10 的整数，并发数需为 1–8 的整数。'
    activeSection.value = 'generation'
    return
  }
  saving.value = true
  saveError.value = ''
  try {
    if (configDirty.value) {
      const previous = clone(config.settings)
      config.settings = clone(draft.value)
      try { await config.save(); baseline.value = clone(config.settings) }
      catch (error) { config.settings = previous; throw error }
    }
    if (fontsDirty.value) {
      const previous = normalizeFonts(appSettings.settings.fonts)
      appSettings.settings.fonts = normalizeFonts(fontDraft.value)
      try { await appSettings.save(); fontBaseline.value = normalizeFonts(appSettings.settings.fonts) }
      catch (error) { appSettings.settings.fonts = previous; applyFonts(fontDraft.value); throw error }
    }
    localStorage.setItem('app-theme', themeDraft.value)
    themeBaseline.value = themeDraft.value
    saved.value = true
    clearTimeout(savedTimer)
    savedTimer = setTimeout(() => { saved.value = false }, 3000)
  } catch (error) { saveError.value = `保存未完成：${String(error)}。未保存的修改仍保留，可重试。` }
  finally { saving.value = false }
}
onMounted(() => {
  document.documentElement.classList.add('settings-open')
  void loadSettings()
  void systemFonts.load()
})
onBeforeUnmount(() => {
  clearTimeout(savedTimer)
  applyFonts(appSettings.settings.fonts)
  document.documentElement.setAttribute('data-theme', themeBaseline.value)
  document.documentElement.classList.remove('settings-open')
})
</script>

<template>
  <div class="preferences">
    <aside class="prefs-sidebar">
      <button class="prefs-back" @click="router.push('/')"><span class="i-carbon:arrow-left" />返回工作台</button>
      <h2 class="prefs-nav-title">设置</h2>
      <nav class="prefs-nav" aria-label="设置分类">
        <button v-for="item in sections" :key="item.key" class="prefs-nav-item" :class="{ selected: activeSection === item.key }"
          :aria-current="activeSection === item.key ? 'page' : undefined" @click="activeSection = item.key">
          <span :class="item.icon" aria-hidden="true" /><span>{{ item.title }}</span>
        </button>
      </nav>
      <span class="prefs-version">v{{ version }}</span>
    </aside>
    <main class="prefs-main">
      <div class="prefs-scroll">
        <div class="prefs-content">
          <header class="prefs-heading"><p class="prefs-breadcrumb">设置<span>/</span>{{ section.title }}</p>
            <h1>{{ section.title }}</h1><p>{{ section.desc }}</p>
          </header>
          <div v-if="!ready" class="prefs-notice" role="status">{{ loading ? '正在加载设置…' : '设置加载失败，请重试。' }}
            <button v-if="!loading" class="prefs-button" @click="loadSettings">重新加载</button>
          </div>
          <fieldset class="prefs-workspace" :disabled="!ready || saving">
            <template v-if="activeSection === 'appearance'">
              <section class="prefs-appearance">
                <h2 class="prefs-section-title">界面外观</h2>
                <div class="prefs-theme-row">
                  <button v-for="theme in themeTiles" :key="theme.id" class="prefs-theme" :class="{ selected: theme.kind === 'dark' ? isDarkTheme : themeDraft === theme.id }"
                    :aria-pressed="theme.kind === 'dark' ? isDarkTheme : themeDraft === theme.id" @click="themeDraft = theme.id">
                    <span class="prefs-theme-preview" :class="`theme-${theme.kind}`"><span class="mini-toolbar"><i /><i /><i /></span><span class="mini-layout"><span class="mini-nav"><i /><i /><i /></span><span class="mini-page"><i /><i /><i /><b /></span></span>
                      <span v-if="theme.kind === 'dark' ? isDarkTheme : themeDraft === theme.id" class="prefs-theme-check">✓</span>
                    </span><span>{{ theme.label }}</span>
                  </button>
                  <label v-if="isDarkTheme" class="prefs-theme-variant">深色配色<select v-model="themeDraft" class="prefs-input"><option v-for="theme in darkThemes" :key="theme.id" :value="theme.id">{{ theme.label }}</option></select></label>
                </div>
              </section>
              <div class="prefs-font-layout">
                <section class="prefs-font-settings">
                  <div class="prefs-section-heading"><h2 class="prefs-section-title">字体</h2><span class="prefs-meta">{{ systemFonts.loaded ? `已安装 ${systemFonts.fonts.length} 种字体` : '系统字体' }}</span><button class="prefs-text-button" @click="resetFonts">恢复默认</button></div>
                  <div class="prefs-font-table">
                    <div class="prefs-font-table-head"><span>用途</span><span>字体</span></div>
                    <div v-for="role in fontRoles" :key="role.key" class="prefs-font-row" :class="{ highlighted: previewRole === role.key }">
                      <div class="prefs-font-label"><label :for="`font-${role.key}`">{{ roleLabels[role.key] }}</label><p>{{ roleDescriptions[role.key] }}</p></div>
                      <SystemFontPicker :id="`font-${role.key}`" :model-value="fontDraft[role.key]" :label="role.label" :sample="role.sample"
                        :disabled="!ready || saving" :open="activeFontRole === role.key" @update:model-value="setFont(role.key, $event)"
                        @update:open="activeFontRole = $event ? role.key : null; previewRole = role.key" @apply-all="applyFontToAll" />
                    </div>
                  </div>
                  <div class="prefs-font-unify"><label><input v-model="unified" type="checkbox" @change="toggleUnified" />统一所有字体</label><span>修改后可在右侧预览。</span></div>
                </section>
                <section class="prefs-preview-card" aria-label="字体效果预览">
                  <div class="prefs-preview-tabs" role="tablist" aria-label="预览用途"><button :aria-selected="previewTab === 'slides'" role="tab" :class="{ selected: previewTab === 'slides' }" @click="previewTab = 'slides'">演示预览</button><button :aria-selected="previewTab === 'interface'" role="tab" :class="{ selected: previewTab === 'interface' }" @click="previewTab = 'interface'">界面预览</button></div>
                  <div class="prefs-preview-body" role="tabpanel">
                    <div v-if="previewTab === 'slides'" class="prefs-slide-preview">
                      <p class="slide-kicker"><span :style="{ fontFamily: fontFamily(fontDraft.number) }">01</span> / IDEAS</p>
                      <h3 :style="{ fontFamily: fontFamily(fontDraft.title) }">让想法清晰呈现</h3>
                      <p class="slide-sub" :style="{ fontFamily: fontFamily(fontDraft.subtitle) }">从灵感到表达</p><div class="slide-rule" />
                      <p class="slide-copy" :style="{ fontFamily: fontFamily(fontDraft.body) }">将 Markdown 文档转化为精美演示文稿。<br />清晰的层级，让内容更容易阅读。</p>
                      <p class="slide-foot" :style="{ fontFamily: fontFamily(fontDraft.caption) }">幻述 Studio · 2026</p>
                      <div class="slide-art" aria-hidden="true"><div /><div /><div /></div>
                    </div>
                    <div v-else class="prefs-interface-preview" :style="{ fontFamily: fontFamily(fontDraft.interface) }">
                      <div class="interface-preview-bar"><BrandMark :size="18" />幻述 Studio</div><h3>新的演示文稿</h3><p>将灵感写下来，让内容自然成形。</p><span class="interface-preview-label">项目名称</span><div class="interface-preview-input">我的第一份演示文稿</div><div class="interface-preview-button">创建项目</div>
                    </div>
                    <div class="prefs-specimen" :style="{ fontFamily: fontFamily(fontDraft[previewRole]) }"><strong>Aa Bb</strong><span>0123456789</span><span>中文字体示例</span></div>
                    <div class="prefs-code-specimen" :style="{ fontFamily: fontFamily(fontDraft.code) }">const ideas = 'Hello, 幻述';</div>
                  </div>
                </section>
              </div>
            </template>
            <template v-else-if="activeSection === 'models'">
              <div class="prefs-services" role="tablist" aria-label="模型服务类型">
                <button v-for="service in services" :key="service.key" class="prefs-service-tile" role="tab" :aria-selected="activeService === service.key" :class="{ selected: activeService === service.key }" @click="activeService = service.key">
                  <span :class="service.icon" class="prefs-service-icon" aria-hidden="true" /><span class="prefs-service-copy"><strong>{{ service.title }}</strong><span>{{ service.subtitle }}</span><small :class="{ connected: serviceStatus(service.key) === '已连接' }"><i />{{ serviceStatus(service.key) }}</small></span><span class="i-carbon:chevron-right" />
                </button>
              </div>
              <div class="prefs-model-layout">
                <section class="prefs-panel prefs-model-form"><div class="prefs-panel-heading"><h2>{{ serviceInfo.title }}</h2><span v-if="serviceDraft.model" class="prefs-pill">{{ serviceDraft.model.split('/').pop() }}</span></div>
                  <div class="prefs-form-row"><label for="service-url">服务地址</label><div><input id="service-url" v-model="serviceDraft.base_url" class="prefs-input" placeholder="http://127.0.0.1:1234" spellcheck="false" /><p class="prefs-help">兼容 OpenAI 接口，支持本地或远程服务。</p></div></div>
                  <div class="prefs-form-row"><label for="service-key">API Key</label><div><div class="prefs-password"><input id="service-key" v-model="serviceDraft.api_key" class="prefs-input" :type="showKey ? 'text' : 'password'" placeholder="本地服务可留空" autocomplete="off" /><button type="button" :aria-label="showKey ? '隐藏 API Key' : '显示 API Key'" :aria-pressed="showKey" @click="showKey = !showKey"><span class="i-carbon:view" /></button></div><p class="prefs-help">配置保存在本机。</p></div></div>
                  <div class="prefs-form-row"><label for="service-model">模型</label><div><div class="prefs-model-input"><input v-if="manualModel[activeService]" id="service-model" v-model="serviceDraft.model" class="prefs-input" placeholder="输入模型 ID" spellcheck="false" /><select v-else id="service-model" v-model="serviceDraft.model" class="prefs-input"><option v-if="!modelOptions.length" value="">请选择模型</option><option v-for="model in modelOptions" :key="model" :value="model">{{ model }}</option></select><button class="prefs-button" :disabled="testing[activeService]" @click="testConnection(activeService)"><span class="i-carbon:renew" />{{ testing[activeService] ? '读取中' : '刷新列表' }}</button></div><button class="prefs-text-button prefs-manual-link" @click="manualModel[activeService] = !manualModel[activeService]">{{ manualModel[activeService] ? '从列表选择模型' : '手动输入模型 ID' }}</button><p class="prefs-help">{{ serviceInfo.hint }}</p></div></div>
                  <div class="prefs-proxy-section"><div class="prefs-form-row"><label for="proxy-enabled">网络代理</label><div class="prefs-toggle-row"><input id="proxy-enabled" v-model="serviceDraft.proxy.enabled" class="prefs-switch" type="checkbox" role="switch" /><span>{{ serviceDraft.proxy.enabled ? '通过 SOCKS5 代理连接' : '直接连接服务' }}</span></div></div>
                    <template v-if="serviceDraft.proxy.enabled"><div class="prefs-form-row"><label for="proxy-url">代理地址</label><div><input id="proxy-url" v-model="serviceDraft.proxy.url" class="prefs-input" placeholder="socks5://127.0.0.1:1080" /><p class="prefs-help">支持 socks5:// 和 socks5h://，需填写端口。</p></div></div><div class="prefs-form-row"><label for="proxy-user">用户名</label><input id="proxy-user" v-model="serviceDraft.proxy.username" class="prefs-input" placeholder="可选" autocomplete="off" /></div><div class="prefs-form-row"><label for="proxy-password">密码</label><input id="proxy-password" v-model="serviceDraft.proxy.password" class="prefs-input" type="password" placeholder="可选" autocomplete="off" /></div></template>
                  </div>
                </section>
                <aside class="prefs-panel prefs-connection"><div class="prefs-panel-heading"><h2>连接状态</h2><span class="prefs-pill" :class="{ success: activeResult?.ok, failure: activeResult && !activeResult.ok }" role="status">{{ serviceStatus(activeService) }}</span></div>
                  <dl class="prefs-summary"><div><dt>服务类型</dt><dd>{{ serviceLocation }}</dd></div><div><dt>模型</dt><dd>{{ serviceDraft.model || '未选择' }}</dd></div><div><dt>响应耗时</dt><dd>{{ activeResult?.ok ? `${activeResult.elapsed} ms` : '—' }}</dd></div></dl>
                  <button class="prefs-button prefs-test-button" :disabled="testing[activeService] || !serviceDraft.base_url.trim()" @click="testConnection(activeService)">{{ testing[activeService] ? '正在测试…' : '测试连接' }}</button>
                  <p v-if="activeResult && !activeResult.ok" class="prefs-inline-error" role="alert">{{ activeResult.error }}</p>
                  <p class="prefs-help">通过获取模型列表检查连接，模型能力以服务实际支持为准。</p>
                  <div class="prefs-service-help"><h3>这个服务用于</h3><ul><li v-for="use in serviceInfo.uses" :key="use">{{ use }}</li></ul><p class="prefs-help">各类服务独立配置，互不覆盖。</p></div>
                </aside>
              </div>
            </template>
            <template v-else-if="activeSection === 'generation'">
              <section class="prefs-panel prefs-simple-panel"><h2>生成参数</h2><div class="prefs-setting-row"><div><label for="repair-rounds">自动修复轮数</label><p>生成后遇到格式校验错误时，自动修复的最大次数。</p><small>建议 2–4 轮</small></div><input id="repair-rounds" v-model.number="draft.repair_rounds" class="prefs-input prefs-number" type="number" min="0" max="10" step="1" /></div><div class="prefs-setting-row"><div><label for="concurrency">生成并发数</label><p>同时生成的幻灯片数量，按模型服务的处理能力调整。</p><small>本地模型建议 1，最多 8</small></div><input id="concurrency" v-model.number="draft.concurrency" class="prefs-input prefs-number" type="number" min="1" max="8" step="1" /></div></section>
              <div class="prefs-guidance"><span class="i-carbon:information" /><p>并发越高，对模型服务的负载越大。遇到超时或资源不足时，可以降低并发数。</p></div>
            </template>
            <template v-else-if="activeSection === 'storage'">
              <section class="prefs-panel prefs-simple-panel"><h2>文件位置</h2><div class="prefs-path-row"><label for="data-dir">数据目录</label><input id="data-dir" class="prefs-input" :value="appSettings.settings.data_dir" readonly /><p class="prefs-help">存放数据库和项目配置文件。</p></div><div class="prefs-path-row"><label for="media-dir">媒体目录</label><input id="media-dir" class="prefs-input" :value="appSettings.settings.media_dir" readonly /><p class="prefs-help">存放上传的图片和视频。</p></div></section>
              <section class="prefs-panel prefs-simple-panel prefs-cache"><div class="prefs-panel-heading"><h2>图标向量缓存</h2><span class="prefs-pill" :class="{ success: embeddingReady }">{{ embeddingStatus }}</span></div><p class="prefs-help">为图标建立语义索引，提高内容与图标的匹配效果。</p><dl class="prefs-summary"><div><dt>当前向量模型</dt><dd>{{ draft.embedding.model || '未配置' }}</dd></div><div><dt>缓存对应模型</dt><dd>{{ appSettings.settings.initialized_embedding_model || '—' }}</dd></div></dl><div class="prefs-cache-action"><button class="prefs-button" :disabled="initializing || embeddingReady || configDirty" @click="initEmbeddings">{{ initializing ? '初始化中…' : embeddingReady ? '缓存已就绪' : '初始化缓存' }}</button><span class="prefs-help">{{ configDirty ? '请先保存模型配置，再初始化缓存。' : '使用已保存的模型服务配置。' }}</span></div><p v-if="embeddingMessage" class="prefs-help" role="status">{{ embeddingMessage }}</p></section>
            </template>
            <template v-else>
              <section class="prefs-panel prefs-about"><div class="prefs-about-brand"><BrandMark :size="44" /><div><h2>幻述 Studio</h2><p>版本 {{ version }}</p></div></div><p>将 Markdown 文档自动转化为精美的演示文稿。</p><div class="prefs-about-divider" /><h3>开始使用</h3><ol><li>在「模型服务」中配置文本生成、向量检索和图片理解服务。</li><li>测试连接并保存设置，需要图标检索时初始化向量缓存。</li><li>返回工作台，上传 Markdown 文件，开始生成演示文稿。</li></ol><button class="prefs-button" @click="router.push('/')"><span class="i-carbon:arrow-left" />返回工作台</button></section>
            </template>
          </fieldset>
        </div>
      </div>
      <footer class="prefs-footer"><div class="prefs-save-state"><span class="prefs-state-dot" :class="{ dirty }" /><span role="status">{{ saving ? '正在保存…' : saved ? '更改已保存' : dirty ? '未保存更改' : '所有更改已保存' }}</span></div><p v-if="saveError" class="prefs-save-error" role="alert">{{ saveError }}</p><div class="prefs-footer-buttons"><button class="prefs-button" :disabled="!dirty || saving" @click="revert">撤销更改</button><button class="prefs-button prefs-primary" :disabled="!dirty || !ready || saving || config.saving" @click="save">{{ saving ? '保存中…' : '保存更改' }}</button></div></footer>
    </main>
  </div>
</template>

<style src="./settings.css"></style>
