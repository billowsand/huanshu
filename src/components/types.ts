// TypeScript mirrors of Rust types in src-tauri/src/types.rs

export type SlideKind =
  | 'cover'
  | 'closing'
  | 'overview'
  | 'section_intro'
  | 'feature_grid'
  | 'spotlight'
  | 'split_layers'
  | 'section_list'
  | 'focus_example'
  | 'outcome_grid'
  | 'center_grid'
  | 'timeline'
  | 'step_flow'
  | 'process'
  | 'compare'
  | 'issue_stack'
  | 'swot'
  | 'infographic'

export type AspectRatio = 'ratio_16x9' | 'ratio_32x9' | 'ratio_48x9'

export const ASPECT_DIMENSIONS: Record<AspectRatio, { w: number, h: number }> = {
  ratio_16x9: { w: 1280, h: 720 },
  ratio_32x9: { w: 2560, h: 720 },
  ratio_48x9: { w: 3840, h: 720 },
}

export interface OverviewItem {
  number: string
  title: string
  desc: string
}

export interface GridCard {
  title: string
  tone?: string
  icon?: string
  subtitle?: string
  body?: string
  items?: string[]
  conclusion?: string
  footer_tag?: string
  footer_tone?: string
  top_bar_class?: string
  risk?: string
  tag?: string
}

export interface SpotlightPanel {
  title: string
  kind?: string
  icon?: string
  tone?: string
  body?: string
  items?: string[]
  steps?: string[]
  highlight?: string
}

export interface ListItem {
  step?: string
  icon?: string
  title: string
  body: string
}

export interface LayerItem {
  title: string
  meta: string
  tone?: string
}

export interface RankingItem {
  index: string
  label: string
  meta: string
  muted?: boolean
}

export interface TimelineEvent {
  date: string
  title: string
  body: string
  tone?: string
  icon?: string
}

export interface CenterItem {
  title: string
  desc: string
  icon?: string
  tone?: string
}

export interface StepItem {
  title: string
  body?: string
  icon?: string
  tone?: string
}

export interface PhaseStep {
  label: string
  desc?: string
}

export interface PhaseItem {
  phase: string
  title: string
  icon?: string
  tone?: string
  steps: PhaseStep[]
  highlight?: string
}

export interface CompareItem {
  label: string
  desc?: string
  highlight?: boolean
}

export interface CompareSide {
  title: string
  tone?: string
  icon?: string
  items: CompareItem[]
  conclusion?: string
}

export interface CompareData {
  mode?: string
  left: CompareSide
  right: CompareSide
}

export interface SwotQuadrant {
  key: string
  title: string
  tone?: string
  icon?: string
  items: string[]
  summary?: string
}

export interface SwotData {
  quadrants: SwotQuadrant[]
  strategy?: string
}

export interface SlideBlueprint {
  kind: SlideKind
  aspect_ratio?: AspectRatio
  section?: string
  title: string
  subtitle?: string
  badge?: string
  accent?: string
  note?: string
  label?: string
  label_tone?: string
  image?: string
  images?: string[]
  placeholder?: string
  side_width?: string
  badges?: string[]
  overview_items?: OverviewItem[]
  cards?: GridCard[]
  panels?: SpotlightPanel[]
  left_items?: ListItem[]
  layers?: LayerItem[]
  list_items?: ListItem[]
  points?: ListItem[]
  ranking?: RankingItem[]
  center_items?: CenterItem[]
  footer?: string
  example_title?: string
  example_body?: string
  timeline_events?: TimelineEvent[]
  steps?: StepItem[]
  phases?: PhaseItem[]
  direction?: string
  compare_data?: CompareData
  swot_data?: SwotData
  infographic_syntax?: string
}
