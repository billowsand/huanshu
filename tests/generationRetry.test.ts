import { beforeEach, describe, expect, mock, test } from 'bun:test'
import { createPinia, setActivePinia } from 'pinia'

const handlers = new Map<string, (event: { payload: any }) => void>()
const invoke = mock(async (_command: string, _args?: any): Promise<any> => undefined)
mock.module('@tauri-apps/api/core', () => ({ invoke }))
mock.module('@tauri-apps/api/event', () => ({
  listen: async (name: string, handler: any) => {
    handlers.set(name, handler)
    return () => handlers.delete(name)
  },
}))
const { useGenerationStore } = await import('../src/stores/generation')
const slide = (kind: string, title: string) => ({ kind, title })
/** retry_generation_page result for a deck that validates after the retry. */
const ready = (s: ReturnType<typeof slide>) => ({ slide: s, deck_ready: true, remaining: '', invalid_pages: [] })
const deck = [slide('cover', '封面'), slide('overview', '目录'), slide('feature_grid', '兜底'), slide('closing', '结束')]
const run = { id: 42, project_id: 1, status: 'done', source_markdown: '', debug_dir: '', created_at: 0, updated_at: 0 }

function failedStore() {
  const store = useGenerationStore()
  store.loadFromJson(JSON.stringify(deck))
  store.currentRun = run
  store.pageStatuses = new Map([[0, { stage: 'error', previousStage: 'content', message: '坏 JSON' }]])
  return store
}

beforeEach(() => {
  setActivePinia(createPinia())
  invoke.mockReset()
  handlers.clear()
})

describe('single-page retry', () => {
  test('final validation failure retains the assembled deck for a page retry', async () => {
    const store = useGenerationStore()
    invoke.mockImplementation(async (command: string) => {
      if (command === 'generate_slides') {
        handlers.get('gen:log')!({ payload: { ...run, run_id: 42, stage: 'content', created_at: 0 } })
        handlers.get('gen:page_status')!({ payload: { slide_index: 0, stage: 'error', message: '校验失败' } })
        throw new Error('最终校验失败')
      }
      if (command === 'get_blueprints') return structuredClone(deck)
      if (command === 'retry_generation_page') return ready(slide('feature_grid', '成功'))
    })
    await store.generate('## 原文')
    expect(store.stage).toBe('error')
    expect(store.blueprints.map(s => s.title)).toEqual(deck.map(s => s.title))
    await store.retryPage(0)
    expect(store.stage).toBe('done')
    expect(store.blueprints[2].title).toBe('成功')
  })

  test('successful retry recovers the overall error state and editor access', async () => {
    const store = failedStore()
    store.stage = 'error'
    store.lastError = '之前生成失败'
    store.currentRun = { ...run, status: 'error', current_stage: 'error' }
    invoke.mockResolvedValue(ready(slide('feature_grid', '已恢复')))
    await store.retryPage(0)
    expect(store.stage).toBe('done')
    expect(store.progress).toBe(1)
    expect(store.lastError).toBe('')
    expect(store.currentRun?.status).toBe('done')
    expect(store.pageStatuses.get(0)?.stage).toBe('done')
  })

  test('replaces only the content page, leaving cover/overview/closing intact', async () => {
    const store = failedStore()
    invoke.mockResolvedValue(ready(slide('issue_stack', '已恢复')))
    await store.retryPage(0)
    expect(invoke).toHaveBeenCalledWith('retry_generation_page', { runId: 42, pageIndex: 0, layoutKind: undefined })
    expect(store.blueprints.map(s => s.title)).toEqual(['封面', '目录', '已恢复', '结束'])
    expect(store.pageStatuses.get(0)?.stage).toBe('done')
    expect(store.retryingPages.size).toBe(0)
  })

  test('a fixed page is kept while other pages still fail, and the deck completes with the last one', async () => {
    const store = useGenerationStore()
    const twoPageDeck = [slide('cover', '封面'), slide('overview', '目录'), slide('feature_grid', '坏一'), slide('feature_grid', '坏二'), slide('closing', '结束')]
    store.loadFromJson(JSON.stringify(twoPageDeck))
    store.stage = 'error'
    store.currentRun = { ...run, status: 'error', current_stage: 'error' }
    store.pageStatuses = new Map([
      [0, { stage: 'error', previousStage: 'content', message: '未通过最终校验' }],
      [1, { stage: 'error', previousStage: 'content', message: '未通过最终校验' }],
    ])
    invoke.mockResolvedValueOnce({ slide: slide('feature_grid', '修好一'), deck_ready: false, remaining: '第 2 页需要重试', invalid_pages: [1] })
    await store.retryPage(0)
    expect(store.blueprints[2].title).toBe('修好一')
    expect(store.pageStatuses.get(0)?.stage).toBe('done')
    expect(store.pageStatuses.get(1)?.stage).toBe('error')
    expect(store.stage).toBe('error')
    expect(store.currentRun?.status).toBe('error')
    expect(store.lastError).toContain('第 2 页')

    invoke.mockResolvedValueOnce(ready(slide('feature_grid', '修好二')))
    await store.retryPage(1)
    expect(store.blueprints.map(s => s.title)).toEqual(['封面', '目录', '修好一', '修好二', '结束'])
    expect(store.stage).toBe('done')
    expect(store.currentRun?.status).toBe('done')
    expect(store.lastError).toBe('')
  })

  test('failed retry preserves fallback content and allows another attempt', async () => {
    const store = failedStore()
    store.stage = 'error'
    store.currentRun = { ...run, status: 'error' }
    invoke.mockRejectedValueOnce(new Error('仍然是坏 JSON'))
    await store.retryPage(0)
    expect(store.stage).toBe('error')
    expect(store.currentRun?.status).toBe('error')
    expect(store.blueprints[2].title).toBe('兜底')
    expect(store.pageStatuses.get(0)?.message).toContain('仍然是坏 JSON')
    invoke.mockResolvedValueOnce(ready(slide('feature_grid', '成功')))
    await store.retryPage(0)
    expect(store.blueprints[2].title).toBe('成功')
  })

  test('passes the manually selected layout to the backend', async () => {
    const store = failedStore()
    invoke.mockResolvedValue(ready(slide('compare', '人工选择')))
    await store.retryPage(0, 'compare')
    expect(invoke).toHaveBeenCalledWith('retry_generation_page', { runId: 42, pageIndex: 0, layoutKind: 'compare' })
    expect(store.blueprints[2].kind).toBe('compare')
  })

  test('duplicate clicks do not submit concurrent retries', async () => {
    const store = failedStore()
    let finish!: (value: any) => void
    invoke.mockImplementation(() => new Promise(resolve => { finish = resolve }))
    const first = store.retryPage(0)
    await store.retryPage(0)
    expect(invoke).toHaveBeenCalledTimes(1)
    finish(ready(slide('feature_grid', '成功')))
    await first
  })

  test('late retry responses do not overwrite a newly opened project', async () => {
    const store = failedStore()
    let finish!: (value: any) => void
    invoke.mockImplementation(() => new Promise(resolve => { finish = resolve }))
    const retry = store.retryPage(0)
    store.reset()
    store.loadFromJson(JSON.stringify([slide('feature_grid', '新项目')]))
    finish(ready(slide('feature_grid', '旧项目结果')))
    await retry
    expect(store.blueprints.map(s => s.title)).toEqual(['新项目'])
    expect(store.pageStatuses.get(0)?.stage).toBe('done')
  })

  test('assembled deck statuses exclude the cover, overview and closing', () => {
    const store = useGenerationStore()
    store.loadFromJson(JSON.stringify(deck))
    expect(store.pageStatuses.size).toBe(1)
    expect(store.slideCount).toBe(4)
  })

  test('queues while generating and retries after final assembly', async () => {
    const store = useGenerationStore()
    let finish!: (value: number) => void
    invoke.mockImplementation(async (command: string) => {
      if (command === 'generate_slides') return new Promise(resolve => { finish = resolve })
      if (command === 'get_blueprints') return structuredClone(deck)
      if (command === 'retry_generation_page') return ready(slide('feature_grid', '成功'))
    })
    const generating = store.generate('## 原文')
    for (let i = 0; i < 10 && !finish; i++) await Promise.resolve()
    expect(finish).toBeDefined()
    handlers.get('gen:log')!({ payload: { ...run, run_id: 42, stage: 'content', created_at: 0 } })
    handlers.get('gen:page_status')!({ payload: { slide_index: 0, stage: 'error', message: '坏 JSON' } })
    await store.retryPage(0, 'issue_stack')
    expect(invoke.mock.calls.some(call => call[0] === 'retry_generation_page')).toBe(false)
    finish(4)
    await generating
    expect(invoke).toHaveBeenCalledWith('retry_generation_page', { runId: 42, pageIndex: 0, layoutKind: 'issue_stack' })
    expect(store.blueprints[2].title).toBe('成功')
    expect(store.pageStatuses.get(0)?.stage).toBe('done')
  })

  test('final fallback assembly retains the error so retry remains available', async () => {
    const store = useGenerationStore()
    invoke.mockImplementation(async (command: string) => {
      if (command === 'generate_slides') {
        handlers.get('gen:page_status')!({ payload: { slide_index: 0, stage: 'error', message: '坏 JSON' } })
        return 4
      }
      if (command === 'get_blueprints') return structuredClone(deck)
    })
    await store.generate('## 原文')
    expect(store.stage).toBe('done')
    expect(store.pageStatuses.get(0)?.stage).toBe('error')
    expect(store.blueprints[2].title).toBe('兜底')
  })

  test('reopening restores unresolved errors and successful retries from logs', async () => {
    const store = useGenerationStore()
    invoke.mockImplementation(async (command: string) => command === 'get_latest_generation_run' ? run : [
      { slide_index: 0, kind: 'page_error', summary: '坏 JSON' },
      { slide_index: 1, kind: 'page_error', summary: '坏 JSON' },
      { slide_index: 1, kind: 'retry', summary: '恢复', detail: { retry_succeeded: true } },
    ])
    await store.loadLatestRun(1)
    expect(store.pageStatuses.get(0)?.stage).toBe('error')
    expect(store.pageStatuses.get(1)?.stage).toBe('done')
  })

  test('reopening a recovered run restores editor access without replaying old errors', async () => {
    const store = useGenerationStore()
    store.loadFromJson(JSON.stringify(deck))
    store.stage = 'error'
    invoke.mockImplementation(async (command: string) => command === 'get_latest_generation_run' ? run : [
      { slide_index: 0, kind: 'page_error', summary: '坏 JSON' },
      { slide_index: 0, kind: 'retry', summary: '恢复', detail: { retry_succeeded: true } },
    ])
    await store.loadLatestRun(1)
    expect(store.stage).toBe('done')
    expect(store.pageStatuses.get(0)?.stage).toBe('done')
    expect(store.currentRun?.status).toBe('done')
  })
})
