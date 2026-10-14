// 规则已移植到 huanshu_core::generation_progress（crates/huanshu-core/src/generation_progress.rs），改一边必须改另一边；一致性由 crates/huanshu-core/tests/generation_progress.rs 快照守住。
export const PAGE_STAGES = [
  { key: 'planning', label: '页面规划' },
  { key: 'layout', label: '布局排版' },
  { key: 'content', label: '内容生成' },
  { key: 'normalizing', label: '规范修复' },
  { key: 'validating', label: '校验修复' },
] as const

export type PipelineStage = typeof PAGE_STAGES[number]['key']
export type PageStageKey = 'pending' | PipelineStage | 'done' | 'error'
export type PageProgressStatus = { stage: string, message?: string, previousStage?: string, waiting?: boolean }
export type PageProgressItem = { index: number, pagePlan: unknown, layoutPlan: unknown, blueprint: unknown }

const order: string[] = ['pending', ...PAGE_STAGES.map(stage => stage.key), 'done']

export function countReadyBlueprints(blueprints: unknown[]): number {
  return blueprints.filter(Boolean).length
}

export function resolveProgressStage(item: PageProgressItem, status?: PageProgressStatus): PageStageKey {
  if (status && [...order, 'error'].includes(status.stage)) return status.stage as PageStageKey
  if (item.blueprint) return 'done'
  if (item.layoutPlan) return 'content'
  return 'pending'
}

export function pageStepState(item: PageProgressItem, status: PageProgressStatus | undefined, key: PipelineStage, running: boolean): 'done' | 'active' | 'pending' {
  const stage = resolveProgressStage(item, status)
  const effectiveStage = stage === 'error' ? status?.previousStage ?? 'pending' : stage
  const rank = order.indexOf(effectiveStage)
  const keyRank = order.indexOf(key)
  if (rank > keyRank || (key === 'layout' && !!item.layoutPlan) || (key === 'planning' && !!item.layoutPlan)) return 'done'
  if (rank === keyRank && stage !== 'error' && running && !status?.waiting && !!status) return 'active'
  return 'pending'
}

export function summarizeProgress(items: PageProgressItem[], statuses: Map<number, PageProgressStatus>, running: boolean) {
  let done = 0, error = 0, active = 0, interrupted = 0
  for (const item of items) {
    const status = statuses.get(item.index)
    const stage = resolveProgressStage(item, status)
    if (stage === 'done') done++
    else if (stage === 'error') error++
    else if (stage !== 'pending' && !status?.waiting && status) {
      if (running) active++
      else interrupted++
    }
  }
  const total = items.length
  return {
    total, done, error, active, interrupted, pending: total - done - error - active - interrupted,
    percent: total ? Math.round(done / total * 100) : 0,
    doneWidth: `${total ? done / total * 100 : 0}%`,
    stageCards: PAGE_STAGES.map(stage => {
      const states = items.map(item => pageStepState(item, statuses.get(item.index), stage.key, running))
      const completed = states.filter(state => state === 'done').length
      const active = states.filter(state => state === 'active').length
      return { ...stage, completed, active, percent: total ? Math.round(completed / total * 100) : 0,
        state: total > 0 && completed === total ? 'done' : active > 0 ? 'active' : 'pending' }
    }),
  }
}
