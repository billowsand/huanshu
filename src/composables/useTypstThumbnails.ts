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
 * - **增量**：每页的键是 蓝图 JSON + 子集位置 + 主题 id + 字体设置 + 宽度档位（主题、字体变了
 *   键就全变）。只有「当前显示的图」和「正在请求的图」的键都对不上的页才进 `only`。
 * - **按键落图，不按请求序号**：响应到达时，只有键仍等于这一页当前键的图才会落下。不能整批丢弃
 *   过期响应——后一次请求只含它那一刻键变了的页，被丢掉的那批页就再也没人请求了。
 * - **失败**：命令报错时 `failed` 为真（调用方回退到幻灯片组件），并 `console.warn` 一次；
 *   不自动重试，下一次内容/主题/尺寸变化时这些页会重新请求。
 * - **停用**：`enabled()` 为假时只是不发请求，已有的图保留（全览关了再开不用重渲）；
 *   localStorage 开关关掉时才清空，让调用方回退到幻灯片组件。
 * - **`wanted` / `maxCached`**：只要 `wanted()` 里出现的下标（`thumbs` 的键，即传入数组的下标）
 *   才进 `only`；`wanted` 不影响发给 Rust 的牌组，仍是整副、仍按子集位置（缓存键里有子集位置，
 *   按 `wanted` 裁剪牌组会让翻一页就整批失效）。`maxCached` 超出时淘汰「不在 `wanted` 里、
 *   最久没被请求过」的图，并同步忘掉它们的已显示键，这样它们回到 `wanted` 时会重新请求。
 *
 * `thumbs` 以传入数组的下标为键（不是子集位置），没有缩略图的页不出现。
 */
export function useTypstThumbnails(opts: {
  /** 下标 = 牌组页序；`null` / `undefined` 表示该页还没有蓝图。 */
  slides: () => Array<SlideBlueprint | null | undefined>
  projectId: () => number | null
  /** 显示宽度 × devicePixelRatio。 */
  widthPx: () => number
  /** 为假时（例如全览没打开）不发请求，已有的图保留。 */
  enabled?: () => boolean
  /** 只要这些下标（传入数组的下标）的图；默认全部。 */
  wanted?: () => number[]
  /** `thumbs` 里最多保留几张；超出时淘汰最久没请求过、且不在 `wanted` 里的。 */
  maxCached?: number
}): {
  thumbs: Ref<Record<number, TypstThumbnail>>
  failed: Ref<boolean>
} {
  const appSettings = useAppSettingsStore()
  const thumbs = ref<Record<number, TypstThumbnail>>({})
  const failed = ref(false)

  /** 已有蓝图的页组成子集；空位留在原数组里，不发给 Rust。 */
  const presentSlides = computed(() => opts.slides().filter(Boolean) as SlideBlueprint[])

  /** 子集位置 → 当前显示的图的键 / 在途请求的键。 */
  let shownKeys: Record<number, string> = {}
  let pendingKeys: Record<number, string> = {}
  /** 最近一次 `refresh` 时，`wanted` 里那几页在子集里的位置。响应到达时按它淘汰。 */
  let latestWanted = new Set<number>()
  /** 子集位置 → 最后一次被 `wanted` 点名的次序，`maxCached` 淘汰用它排序。 */
  let lastUsed: Record<number, number> = {}
  let accessCounter = 0
  /** 最近一次 `refresh` 时的键与页序映射，响应到达时用它判断图是否仍然有效、该落到哪一页。 */
  let latestKeys: string[] = []
  let latestIndices: number[] = []
  let timer: ReturnType<typeof setTimeout> | null = null
  let disposed = false
  let warned = false

  function widthBucket(): number {
    return Math.max(WIDTH_BUCKET, Math.ceil(Math.max(opts.widthPx(), 1) / WIDTH_BUCKET) * WIDTH_BUCKET)
  }

  /** `wanted()` 说的「要哪几页」，翻译成传给 Rust 的数组下标；默认全部已有蓝图的页。 */
  function wantedIndices(all: Array<SlideBlueprint | null | undefined>): number[] {
    if (!opts.wanted) return all.flatMap((slide, index) => (slide ? [index] : []))
    const asked = new Set(opts.wanted().map(index => Math.max(0, Math.trunc(index))))
    return all.flatMap((slide, index) => (slide && asked.has(index) ? [index] : []))
  }

  /** 标记这些下标刚被用到，并把它们排到 `wanted` 前面（淘汰顺序用）。 */
  function touch(wanted: number[]) {
    for (const index of wanted) lastUsed[index] = ++accessCounter
  }

  /**
   * 把缓存压回 `maxCached`：不在 `wanted` 里、也没有在途请求的图，按最久没被点名的先丢。
   * 必须连「已显示键」一起忘掉，否则这一页再回到 `wanted` 时键还相等，永远不会重新请求。
   */
  function evict(wanted: Set<number>) {
    const limit = opts.maxCached
    if (!limit || Object.keys(thumbs.value).length <= limit) return
    const evictable = Object.keys(thumbs.value)
      .map(Number)
      .filter(index => !wanted.has(index) && pendingKeys[index] === undefined)
      .sort((a, b) => (lastUsed[a] ?? 0) - (lastUsed[b] ?? 0))
    const next = { ...thumbs.value }
    for (const index of evictable) {
      if (Object.keys(next).length <= limit) break
      delete next[index]
      delete shownKeys[index]
      delete lastUsed[index]
    }
    thumbs.value = next
  }

  function refresh() {
    if (!thumbnailsEnabledByPreference()) {
      thumbs.value = {}
      shownKeys = {}
      lastUsed = {}
      return
    }
    if (!(opts.enabled?.() ?? true)) return

    const all = opts.slides()
    const deck = presentSlides.value
    const width = widthBucket()
    const theme = document.documentElement.getAttribute('data-theme')
    const fonts = appSettings.settings.fonts
    const wanted = wantedIndices(all)
    touch(wanted)
    latestKeys = deck.map((slide, position) => JSON.stringify([slide, position, theme, fonts, width]))
    latestIndices = all.flatMap((slide, index) => (slide ? [index] : []))
    // `latestIndices` 是子集位置 → 数组下标；`wanted` 反过来用。
    const wantedPositions = latestIndices.flatMap((index, position) => (wanted.includes(index) ? [position] : []))
    latestWanted = new Set(wantedPositions)

    const only = wantedPositions.flatMap(position =>
      shownKeys[position] !== latestKeys[position] && pendingKeys[position] !== latestKeys[position] ? [position] : [],
    )
    if (!only.length) {
      evict(latestWanted)
      return
    }
    const requestKeys = [...latestKeys]
    for (const position of only) pendingKeys[position] = requestKeys[position]
    evict(latestWanted)
    void render(deck, theme, width, only, requestKeys)
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
    requestKeys: string[],
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
      if (disposed) return
      const next = { ...thumbs.value }
      for (const page of batch.pages) {
        const key = requestKeys[page.index]
        if (pendingKeys[page.index] === key) delete pendingKeys[page.index]
        // 这一页在请求期间又变了（内容、页序、主题或尺寸）：这张图已过期，等下一轮的新图。
        if (latestKeys[page.index] !== key) continue
        const index = latestIndices[page.index]
        if (index === undefined) continue
        next[index] = { url: page.data_url, placeholder: page.placeholder }
        shownKeys[page.index] = key
      }
      thumbs.value = next
      failed.value = false
      // 这一批请求期间可能翻过页：新的 `wanted` 之外的图要按上限淘汰。
      evict(latestWanted)
    } catch (e) {
      if (disposed) return
      for (const position of only) {
        if (pendingKeys[position] === requestKeys[position]) delete pendingKeys[position]
      }
      failed.value = true
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

  // 牌组内容、页序、宽度档位、启用状态、字体变化都会改变键或请求条件；键的比较决定重渲哪些页。
  watch(presentSlides, schedule, { deep: true })
  watch(() => opts.slides().length, schedule)
  watch(() => Math.ceil(Math.max(opts.widthPx(), 1) / WIDTH_BUCKET), schedule)
  watch(() => opts.enabled?.() ?? true, schedule)
  watch(() => appSettings.settings.fonts, schedule, { deep: true })
  // `wanted` 只决定请求哪几页（键里没有它），翻页时它会变，所以要单独盯着。
  watch(() => (opts.wanted ? opts.wanted().join(',') : ''), schedule)

  // 主题切换会改 --chart-* / 主题 token，键里带主题 id，触发一次刷新即可。
  const themeObserver = new MutationObserver(schedule)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })

  schedule()

  onBeforeUnmount(() => {
    if (timer) clearTimeout(timer)
    timer = null
    disposed = true
    themeObserver.disconnect()
  })

  return { thumbs, failed }
}
