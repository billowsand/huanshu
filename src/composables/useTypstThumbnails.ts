import { computed, onBeforeUnmount, ref, watch, type Ref } from 'vue'
import { invoke } from '@tauri-apps/api/core'
import { useAppSettingsStore } from '../stores/appSettings'
import type { SlideBlueprint } from '../components/types'

/** 与 `commands::thumbnails::ThumbnailBatch`（src-tauri/src/commands/thumbnails.rs）对应。 */
export interface ThumbnailBatch {
  pages: Array<{ index: number, data_url: string, placeholder: boolean, overflow: boolean }>
  theme_fallback: string | null
  elapsed_ms: number
}

export interface TypstThumbnail {
  url: string
  placeholder: boolean
}

/** 缩略图宽度向上取整到这个倍数，避免窗口每变 1px 就整副重渲。 */
const WIDTH_BUCKET = 64
/** 牌组变化后等这么久再渲染，生成过程中一页页到达不会触发多次整副编译。 */
const DEBOUNCE_MS = 200

/**
 * 用 Typst 光栅化的 PNG 作为幻灯片画面（缩略图、详情大图、演示主画面、编辑器预览）。
 *
 * 每页挂一个完整的幻灯片组件（还得把网页渲染器搬进来）代价很高，所以把牌组交给 Rust 侧编译
 * 一次、逐页光栅化，这里只负责 `<img>` 需要的数据。**没有图时由调用方显示加载框**
 * （`components/SlideFrame.vue`），这里不再提供任何回退渲染。
 *
 * 要点：
 * - **子集与页序**：`slides()` 里还没有蓝图的页（生成中的空位）被跳过，只有已有蓝图组成子集
 *   发给 Rust，`latestIndices` 记下子集位置到传入数组下标的映射。Typst 的页序和返回的 `index`
 *   都是**子集位置**，缩略图结果再映射回下标。
 * - **两套下标**：`thumbs` / `overflow` / `lastUsed` / `shownSequence` 以**数组下标**为键；
 *   `shownKeys` / `pendingKeys` / `wantedPositions` 以**子集位置**为键（生成中有空位时两者不同）。
 *   换算一律经过 `latestIndices`（子集位置 → 数组下标）；`positionOf` 是它反过来。
 * - **增量**：每页的键是 蓝图 JSON + 子集位置 + 主题 id + 字体设置 + 宽度档位（主题、字体变了
 *   键就全变）。只有「当前显示的图」和「正在请求的图」的键都对不上的页才进 `only`。
 * - **按键落图，不按请求序号**：响应到达时，只有键仍等于这一页当前键（`progressive` 下是序号
 *   更新）的图才会落下。不能整批丢弃过期响应——后一次请求只含它那一刻键变了的页，
 *   被丢掉的那批页就再也没人请求了。
 * - **`serial` + `progressive`**：打字时每一页的键每敲一个字都变，`serial` 保证同一时刻只有
 *   一个请求在途，变化被合并进一次尾部请求；`progressive` 让每个响应只要比当前显示的图新就
 *   先落下（键记成它自己的序号，所以最新内容仍会被再次请求）。因此预览最多落后一个请求的
 *   时间、每次响应都可能更新画面，而最后完成的那个响应一定是最新内容；停手后不会再有待请求
 *   的内容，画面落在最新一版。
 * - **`resetKey`**：值变化说明调用方换了目标（编辑者换了页），前一页的在途响应整批作废，
 *   已显示的图和键一并清空，调用方立刻显示加载框。
 * - **失败**：命令报错时 `failed` 为真（调用方在画面角上显示「渲染失败 · 重试」，旧图仍然
 *   照常显示），并 `console.warn` 一次；不自动重试，下一次内容/主题/尺寸变化时这些页会重新
 *   请求，调用方也可以直接调 `refresh()` 重试。
 * - **`enabled()` 为假**：只是不发请求，已有的图保留（全览关了再开不用重渲）。
 * - **`wanted` / `maxCached`**：只要 `wanted()` 里出现的下标（`thumbs` 的键，即传入数组的下标）
 *   才进 `only`；`wanted` 不影响发给 Rust 的牌组，仍是整副、仍按子集位置（缓存键里有子集位置，
 *   按 `wanted` 裁剪牌组会让翻一页就整批失效）。`maxCached` 超出时淘汰「不在 `wanted` 里、
 *   最久没被请求过」的图，并同步忘掉它们的已显示键，这样它们回到 `wanted` 时会重新请求。
 *
 * `thumbs` 以传入数组的下标为键（不是子集位置），还没有图的页不出现。
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
  /** 内容变化后等这么久再渲染，默认 200。打字时调小它可以让画面跟得更紧。 */
  debounceMs?: number
  /** 为真时同一时刻最多一个请求在途，在途期间的变化合并成一次尾部请求。 */
  serial?: boolean
  /** 为真时，过期但比当前显示的图新的响应也先落下（对应页的键记成它自己的序号，所以
   *  最新内容仍会被再次请求）。 */
  progressive?: boolean
  /** 值变化时清空全部图和键（在途响应一并作废），用于「换了一页」。 */
  resetKey?: () => unknown
}): {
  thumbs: Ref<Record<number, TypstThumbnail>>
  failed: Ref<boolean>
  /** 有请求在途，或在防抖窗口里等着发。 */
  pending: Ref<boolean>
  /** 该页内容被裁切（缩到 `fit-min` 仍放不下）；键是传入数组的下标。 */
  overflow: Ref<Record<number, boolean>>
  /**
   * 立刻按当前状态重新评估一次（不过 debounce）。`wanted()` 变化已经被内部盯着了，
   * 这个出口是给「不看 `wanted` 的输入变了、但要马上出图」的调用方用的（演示翻页）。
   */
  refresh: () => void
} {
  const appSettings = useAppSettingsStore()
  const thumbs = ref<Record<number, TypstThumbnail>>({})
  const failed = ref(false)
  const pending = ref(false)
  const overflow = ref<Record<number, boolean>>({})

  /** 已有蓝图的页组成子集；空位留在原数组里，不发给 Rust。 */
  const presentSlides = computed(() => opts.slides().filter(Boolean) as SlideBlueprint[])

  /** 子集位置 → 当前显示的图的键 / 在途请求的键。 */
  let shownKeys: Record<number, string> = {}
  let pendingKeys: Record<number, string> = {}
  /** 子集位置 → 已显示的图来自哪一次请求 / 最后一次为它发出的请求；`progressive` 用它判新旧。 */
  let shownSequence: Record<number, number> = {}
  let requestedSequence: Record<number, number> = {}
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
  /** 请求序号：每次真的调用命令就 +1，`progressive` 用它判断响应新旧。 */
  let sequence = 0
  /** `serial` 用：有请求在途时，期间发生的变化合并成一次尾部请求。 */
  let inFlight = false
  let dirty = false
  /** 换过目标（`resetAll`）：在途响应已作废，下一次 `refresh` 必须真的发请求。 */
  let resetPending = false
  let lastResetKey: unknown
  /** 每次 `resetAll` +1；请求记下发出时的值，响应回来时不相等就整批作废。 */
  let generation = 0

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
  function evict(wantedPositions: Set<number>) {
    const limit = opts.maxCached
    if (!limit || Object.keys(thumbs.value).length <= limit) return
    const positionOf = new Map(latestIndices.map((index, position) => [index, position]))
    const evictable = Object.keys(thumbs.value)
      .map(Number)
      .filter((index) => {
        const position = positionOf.get(index)
        return position === undefined || (!wantedPositions.has(position) && pendingKeys[position] === undefined)
      })
      .sort((a, b) => (lastUsed[a] ?? 0) - (lastUsed[b] ?? 0))
    const next = { ...thumbs.value }
    for (const index of evictable) {
      if (Object.keys(next).length <= limit) break
      delete next[index]
      delete lastUsed[index]
      delete overflow.value[index]
      const position = positionOf.get(index)
      if (position !== undefined) delete shownKeys[position]
    }
    thumbs.value = next
  }

  /** 清空全部图和键，调用方随即显示加载框（`resetKey` 变化：换了目标页）。 */
  function resetAll() {
    generation += 1
    resetPending = true
    thumbs.value = {}
    overflow.value = {}
    shownKeys = {}
    shownSequence = {}
    lastUsed = {}
  }

  function refresh() {
    if (disposed) return
    if (!(opts.enabled?.() ?? true)) {
      pending.value = false
      return
    }
    // 一次请求在途：`serial` 把这段时间里的变化压成尾部的一次请求，否则直接照常发。
    if (inFlight) {
      if (opts.serial) dirty = true
      return
    }
    // `resetAll`（换页）把上一页的在途响应作废了，所以不能因为 `pendingKeys` 里还留着那个位置
    // 就跳过这一页——那个响应回来时已经过期，也不会触发尾部请求，画面会一直空着。
    // 记下这一次要忽略在途键，并把 `dirty` 立起来，保证这一次请求结束后一定会重来。
    const ignoringPending = resetPending
    resetPending = false
    dirty = true

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

    const only = wantedPositions.flatMap(position => {
      // 在途请求带的就是这一页的当前键，等它回来即可；换过目标时那个请求已经作废，不算数。
      if (!ignoringPending && pendingKeys[position] !== undefined) return []
      // `progressive` 下显示的可能是过期但更新的图，此时键不等；两种模式都靠这一条重发。
      return shownKeys[position] === latestKeys[position] ? [] : [position]
    })
    if (!only.length) {
      dirty = false
      evict(latestWanted)
      pending.value = false
      return
    }
    const requestKeys = [...latestKeys]
    const requestSequence = ++sequence
    for (const position of only) {
      pendingKeys[position] = requestKeys[position]
    }
    evict(latestWanted)
    pending.value = true
    void render(deck, theme, width, only, requestKeys, requestSequence)
  }

  async function render(
    deck: SlideBlueprint[],
    theme: string | null,
    width: number,
    only: number[],
    requestKeys: string[],
    requestSequence: number,
  ) {
    inFlight = true
    const requestGeneration = generation
    let requestFailed = false
    try {
      const batch = await invoke<ThumbnailBatch>('render_slide_thumbnails', {
        projectId: opts.projectId(),
        slides: deck,
        theme,
        widthPx: width,
        only,
      })
      if (disposed) {
        inFlight = false
        return
      }
      const next = { ...thumbs.value }
      // 请求发出后调用方换过目标（`resetKey` 变了）：这批图属于上一页，一张都不落。
      // 不能靠键判断——换页后 `latestKeys` 要等下一次 `refresh` 才更新，旧页的键此刻仍然「相等」。
      const pages = requestGeneration === generation ? batch.pages : []
      for (const page of pages) {
        const key = requestKeys[page.index]
        const position = page.index
        const index = latestIndices[position]
        if (index !== undefined) {
          // 裁切是这一页自己的属性：即使图过期，也要让调用方知道它被裁了。
          const nextOverflow = { ...overflow.value }
          nextOverflow[index] = page.overflow
          overflow.value = nextOverflow
        }
        // 只有「键仍等于这一页当前键」的图才作数；`progressive` 放宽成「比已显示的图新」，
        // 后者落地时把键记成**它自己的**序号，于是最新内容仍会被下一轮请求。
        const current = latestKeys[position] === key
        const newer = opts.progressive === true && requestSequence > (shownSequence[position] ?? 0)
        if (!current && !newer) continue
        if (index === undefined) continue
        next[index] = { url: page.data_url, placeholder: page.placeholder }
        shownSequence[position] = requestSequence
        // 过期但更新的图：键记成触发它的那次请求的键，这样最新内容还会被再次请求。
        shownKeys[position] = current ? key : requestKeys[position]
      }
      thumbs.value = next
      failed.value = false
      // 这一批请求期间可能翻过页：新的 `wanted` 之外的图要按上限淘汰。
      evict(latestWanted)
    } catch (e) {
      if (disposed) {
        inFlight = false
        return
      }
      failed.value = true
      requestFailed = true
      if (!warned) {
        warned = true
        console.warn('[thumbnails] Typst 渲染失败：', e)
      }
    }
    // 成功或失败都要清掉在途标记：失败的页会留到下一次内容变化（或调用方的「重试」）时重新请求。
    inFlight = false
    for (const position of only) {
      if (pendingKeys[position] === requestKeys[position]) delete pendingKeys[position]
    }
    // `serial`：期间变化过就立刻用最新状态再请求一次；`dirty` 只在这里清掉，
    // 所以这条链一直走到「返回后再没有新变化」为止，最后一次请求带的就是最新内容。
    // 失败时不接着请求：`refresh` 每次都会立起 `dirty`，失败的页键又没落下，接着请求只会
    // 原样再失败一次，变成对 Rust 的死循环（一副编译不过的牌组就会这样）。
    if (requestFailed) dirty = false
    if (!disposed && dirty) {
      dirty = false
      refresh()
      return
    }
    if (!disposed) {
      // 非 serial 的调用方没有在途合并，但也要把「还在防抖窗口里等着发」反映到 `pending` 上。
      // （`refresh` 里还有别的出口会把它置回 false。）
      pending.value = timer !== null
    }
  }

  function schedule() {
    if (timer) clearTimeout(timer)
    if (opts.serial && inFlight) {
      // 在途期间不需要再排一次定时器：响应回来后 `refresh` 里会读到 `dirty`。
      dirty = true
      pending.value = true
      return
    }
    const delay = opts.debounceMs ?? DEBOUNCE_MS
    pending.value = true
    timer = setTimeout(() => {
      timer = null
      refresh()
    }, delay)
  }

  // 牌组内容、页序、宽度档位、启用状态、字体变化都会改变键或请求条件；键的比较决定重渲哪些页。
  watch(presentSlides, schedule, { deep: true })
  watch(() => opts.slides().length, schedule)
  watch(() => Math.ceil(Math.max(opts.widthPx(), 1) / WIDTH_BUCKET), schedule)
  watch(() => opts.enabled?.() ?? true, schedule)
  watch(() => appSettings.settings.fonts, schedule, { deep: true })
  // `wanted` 只决定请求哪几页（键里没有它），翻页时它会变，所以要单独盯着。
  watch(() => (opts.wanted ? opts.wanted().join(',') : ''), schedule)
  // 换了目标：上一页的在途响应全部作废，已显示的图和键立刻清空。
  watch(() => opts.resetKey?.(), () => {
    if (timer) clearTimeout(timer)
    timer = null
    if (opts.resetKey && opts.resetKey() !== lastResetKey) {
      lastResetKey = opts.resetKey()
      resetAll()
    }
    schedule()
  })

  // 主题切换：键里带主题 id，触发一次刷新即可。
  const themeObserver = new MutationObserver(schedule)
  themeObserver.observe(document.documentElement, { attributes: true, attributeFilter: ['data-theme'] })

  if (opts.resetKey) lastResetKey = opts.resetKey()
  schedule()

  onBeforeUnmount(() => {
    if (timer) clearTimeout(timer)
    timer = null
    disposed = true
    themeObserver.disconnect()
  })

  return { thumbs, failed, pending, overflow, refresh }
}
