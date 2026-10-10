import { describe, expect, test } from 'bun:test'
import { PAGE_STAGES, countReadyBlueprints, summarizeProgress } from '../src/utils/generationProgress'
import type { PageProgressItem, PageProgressStatus } from '../src/utils/generationProgress'

const items = (count: number): PageProgressItem[] => Array.from({ length: count }, (_, index) => ({ index, pagePlan: {}, layoutPlan: null, blueprint: null }))
const statuses = (...values: PageProgressStatus[]) => new Map(values.map((value, index) => [index, value]))

describe('generation progress counters', () => {
  test('out-of-order completion does not count empty blueprint slots', () => {
    expect(countReadyBlueprints([null, null, {}, null, {}])).toBe(2)
    expect(countReadyBlueprints([])).toBe(0)
  })
  test('entering a stage does not complete that stage', () => {
    for (const stage of PAGE_STAGES) {
      const summary = summarizeProgress(items(1), statuses({ stage: stage.key }), true)
      const current = summary.stageCards.find(card => card.key === stage.key)!
      expect(current.completed).toBe(0)
      expect(current.active).toBe(1)
      expect(summary.doneWidth).toBe('0%')
    }
  })
  test('queued pages are not active and assigned layouts are complete', () => {
    const summary = summarizeProgress(items(17), statuses(...Array.from({ length: 17 }, (_, i) => ({ stage: 'content', waiting: i >= 3 }))), true)
    expect(summary.active).toBe(3)
    expect(summary.pending).toBe(14)
    expect(summary.stageCards.map(card => card.completed)).toEqual([17, 17, 0, 0, 0])
    expect(summary.percent).toBe(0)
    expect(summary.doneWidth).toBe('0%')
  })
  test('failures retain completed stages without completing failed validation', () => {
    const summary = summarizeProgress(items(2), statuses({ stage: 'done' }, { stage: 'error', previousStage: 'validating' }), true)
    expect(summary.stageCards.map(card => card.completed)).toEqual([2, 2, 2, 2, 1])
    expect(summary.done).toBe(1)
    expect(summary.error).toBe(1)
    expect(summary.percent).toBe(50)
  })
  test('stopped runs have no actively processing pages', () => {
    const summary = summarizeProgress(items(3), statuses({ stage: 'content' }, { stage: 'content', waiting: true }, { stage: 'error', previousStage: 'content' }), false)
    expect(summary.active).toBe(0)
    expect(summary.interrupted).toBe(1)
    expect(summary.pending).toBe(1)
    expect(summary.error).toBe(1)
    expect(summary.stageCards.every(card => card.active === 0)).toBe(true)
  })
  test('empty and finished decks have valid percentages', () => {
    expect(summarizeProgress([], new Map(), false).percent).toBe(0)
    const summary = summarizeProgress(items(2), statuses({ stage: 'done' }, { stage: 'done' }), false)
    expect(summary.percent).toBe(100)
    expect(summary.stageCards.every(card => card.state === 'done')).toBe(true)
  })
  test('missing page events do not mark base plans or sparse slots as complete', () => {
    expect(summarizeProgress(items(3), new Map(), true).pending).toBe(3)
    expect(summarizeProgress(items(3), new Map(), true).done).toBe(0)
  })
})
