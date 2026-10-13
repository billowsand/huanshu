import { computed, onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useAppSettingsStore } from '../stores/appSettings'
import { buildChartSvgs } from './chartSvgs'
import type { SlideBlueprint } from '../components/types'

/** 与 `commands::thumbnails::ThumbnailBatch`（src-tauri/src/commands/thumbnails.rs）对应。 */
export interface ThumbnailBatch {
  pages: Array<{ index: number, data_url: string, placeholder: boolean }>
  theme_fallback: string | null
  elapsed_ms: number
}

export interface TypstThumbnail {
  url: string
  placeholder: boolean
}

/** 关闭开关用的 localStorage 键（`'0'` 表示禁用，便于 A/B 对比）。 */
export const TYPST_THUMBNAILS_KEY = 'huanshu.typstThumbnails'

/** 缩略图宽度向上取整到这个倍数，避免窗口每变 1px 就整副重渲。 */
const WIDTH_BUCKET = 64
/** 牌组变化后等这么久再渲染，生成过程中一页页到达不会触发多次整副编译。 */
const DEBOUNCE_MS = 200

function thumbnailsEnabledByPreference(): boolean {
  try {
    return localStorage.getItem(TYPST_THUMBNAILS_KEY) !== '0'
  } catch {
    return true
  }
}

/**
 * 用 Typst 光栅化的 PNG 作为幻灯片缩略图。
 *
 * 一次展示很多页时，每页挂一个完整的幻灯片组件（玻璃主题还带 backdrop-filter）代价很高，
 * 所以改为把牌组交给 Rust 侧编译一次、逐页光栅化，这里只负责 `<img>` 需要的数据。
 *
 * 要点：
 * - **子集与页序**：`slides()` 里还没有蓝图的页（生成中的空位）被跳过，只有已有蓝图组成子集
 *   发给 Rust，`deckIndex` 记下子集位置到传入数组下标的映射。Typst 的页序、返回的 `index`、
 *   `chart:<index>` 都是**子集位置**，所以图表 SVG 也用同一份子集构建（见 `render`），
 *   缩略图结果再映射回 `thumbs` 的下标。
 * - **增量**：每页的缓存键是 蓝图 JSON + 子集位置 + 主题 id + 字体设置 + 宽度档位；
 *   只有键变了的页进 `only`，全都没变就不调用命令。
 * - **主题与字体**：`data-theme` 变化、字体设置变化都会让全部缩略图失效。
 * - **防抖与竞态**：200ms 防抖 + 递增请求序号，过期响应直接丢弃；新图到达前保留旧图，不闪烁。
 * - **失败**：命令报错时 `failed` 为真（调用方回退到幻灯片组件），并 `console.warn` 一次。
 *
 * `thumbs` 以传入数组的下标为键（不是子集位置），没有缩略图的页不出现。
 */
export function useTypstThumbnails(opts: {
  /** 下标 = 牌组页序；`null` / `undefined` 表示该页还没有蓝图。 */
  slides: () => Array<SlideBlueprint | null | undefined>
  projectId: () => number | null
  /** 显示宽度 × devicePixelRatio。 */
  widthPx: () => number
  /** 为假时（例如全览没打开）不渲染，也不发起请求。 */
  enabled?: () => boolean
}): {
  thumbs: Ref<Record<number, TypstThumbnail>>
  failed: Ref<boolean>
  /** `renderDeck[i]` 在传入数组里的下标（缩略图下标 = 它，`chart:<i>` = 子集位置 i）。 */
  deckIndex: Ref<number[]>
} {
  const appSettings = useAppSettingsStore()
  const thumbs = ref<Record<number, TypstThumbnail>>({})
  const failed = ref(false)

  /** 已有蓝图的页组成子集；空位留在原数组里，不发给 Rust。 */
  const presentSlides = computed(() => opts.slides().filter(Boolean) as SlideBlueprint[])
  const deckIndex = computed(() =>
    opts.slides().flatMap((slide, index) => (slide ? [index] : [])),
  )

  /** 每页上一次渲染用的键，用来算出这一轮要重渲哪些页。 */
  let cacheKeys: Record<number, string> = {}
  let timer: ReturnType<typeof setTimeout> | null = null
  let requestSeq = 0
  let warned = false

  function isEnabled(): boolean {
    return (opts.enabled?.() ?? true) && thumbnailsEnabledByPreference()
  }

  function renderKey(slide: SlideBlueprint, position: number, width: number, theme: string | null): string {
    return JSON.stringify([slide, position, theme, appSettings.settings.fonts, width])
  }

  function invalidate() {
    cacheKeys = {}
    schedule()
  }

  function refresh() {
    if (!isEnabled()) {
      thumbs.value = {}
      cacheKeys = {}
      return
    }
    const deck = presentSlides.value
    const indices = deckIndex.value
    // 子集为空时清掉缓存，免得切回有内容的牌组时命中过期键。
    if (!deck.length) {
      thumbs.value = {}
      cacheKeys = {}
      return
    }
    const width = Math.max(WIDTH_BUCKET, Math.ceil(Math.max(opts.widthPx(), 1) / WIDTH_BUCKET) * WIDTH_BUCKET)
    const theme = document.documentElement.getAttribute('data-theme')
    const keys = deck.map((slide, position) => renderKey(slide, position, width, theme))
    // 张数变了也算全部失效：子集位置整体平移过，旧图和新页对不上。
    const changed = Object.keys(cacheKeys).length !== keys.length
    const only = keys.flatMap((key, position) =>
      changed || cacheKeys[position] !== key ? [position] : [],
    )
    if (!only.length) return

    // 先记下键：请求失败时这一页不会再被反复请求，等下次内容/主题变化再试。
    const nextKeys: Record<number, string> = {}
    keys.forEach((key, position) => { nextKeys[position] = key })
    cacheKeys = nextKeys

    const seq = ++requestSeq
    void render(deck, theme, width, only, indices, seq)
  }

  /**
   * chart_focus 页的图表在 typst 里是一张按绘图框渲染的 SVG，缩略图也必须带上，否则这一页只剩
   * 空卡片。`chart:<index>` 的下标是子集位置，所以用同一份 `deck` 调用 buildChartSvgs。
   */
  async function render(
    deck: SlideBlueprint[],
    theme: string | null,
    width: number,
    only: number[],
    indices: number[],
    seq: number,
  ) {
    let chartSvgs: Array<{ index: number, svg: string }> | null = null
    try {
      chartSvgs = await buildChartSvgs(deck, { projectId: opts.projectId(), themeId: theme })
    } catch (e) {
      // 图表渲染失败不该拖垮整页缩略图：图表页会退回占位卡片。
      console.warn('[thumbnails] 图表 SVG 渲染失败：', e)
    }
    try {
      const batch = await invoke<ThumbnailBatch>('render_slide_thumbnails', {
        projectId: opts.projectId(),
        slides: deck,
        theme,
        chartSvgs,
        widthPx: width,
        only,
      })
      // 过期响应（内容又变了 / 组件已卸载）直接丢弃，否则会把旧图盖回去。
      if (seq !== requestSeq) return
      const next = { ...thumbs.value }
      for (const page of batch.pages) {
        const index = indices[page.index]
        if (index === undefined) continue
        next[index] = { url: page.data_url, placeholder: page.placeholder }
      }
      thumbs.value = next
      failed.value = false
    } catch (e) {
      if (seq === requestSeq) failed.value = true
      if (!warned) {
        warned = true
        console.warn('[thumbnails] Typst 缩略图渲染失败，回退到幻灯片组件：', e)
      }
    }
  }

  function schedule() {
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => {
      timer = null
      refresh()
    }, DEBOUNCE_MS)
  }

  // 牌组内容变化（页序也会影响子集位置）。
  watch(presentSlides, schedule, { deep: true })
  // 宽度档位：窗口尺寸变化不改牌组，要单独触发。
  watch(() => Math.ceil(Math.max(opts.widthPx(), 1) / WIDTH_BUCKET), schedule)
  watch(() => opts.enabled?.() ?? true, schedule)
  watch(() => appSettings.settings.fonts, invalidate, { deep: true })

  // 主题切换会改 --chart-* / 主题 token，缩略图必须整副重渲。
  const themeObserver = new MutationObserver(invalidate)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })

  schedule()

  onBeforeUnmount(() => {
    if (timer) clearTimeout(timer)
    timer = null
    requestSeq += 1
    themeObserver.disconnect()
  })

  return {
    thumbs,
    failed,
    deckIndex,
  }
}
