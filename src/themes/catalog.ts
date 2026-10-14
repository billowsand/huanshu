// App themes: startup validation (main.ts), the quick switcher (ThemeToggle) and
// the settings page all read this list.
//
// The same list lives in `crates/huanshu-core/src/themes.rs` for the native UI.
// Two copies while both UIs ship: **change one, change the other**.
export interface AppTheme {
  id: string
  name: string
  tone: 'light' | 'dark'
  /** Swatch background for the compact switcher. */
  swatch: string
  accent: string
}

export const APP_THEMES: AppTheme[] = [
  { id: 'dark-yellow', name: '暗·琥珀', tone: 'dark', swatch: '#0b0c11', accent: '#f59e0b' },
  { id: 'dark-blue', name: '暗·星图', tone: 'dark', swatch: '#020810', accent: '#0EA5E9' },
  { id: 'dark-terminal', name: '终端·墨', tone: 'dark', swatch: '#151614', accent: '#ed9b62' },
  { id: 'light-terminal', name: '终端·纸', tone: 'light', swatch: '#f4f3eb', accent: '#9d4a1c' },
  { id: 'light-editorial', name: '报刊·钴蓝', tone: 'light', swatch: 'linear-gradient(180deg, #1d4ed8 0% 14%, #f3efe6 14%)', accent: '#1d4ed8' },
  { id: 'light-notes', name: '便签·稿纸', tone: 'light', swatch: 'linear-gradient(135deg, #746254 0% 35%, #f1debd 35% 65%, #fbf7ec 65%)', accent: '#746254' },
]

export const DEFAULT_THEME = 'dark-yellow'

export function isKnownTheme(id: string | null | undefined): id is string {
  return !!id && APP_THEMES.some(theme => theme.id === id)
}
