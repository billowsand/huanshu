export const DEFAULT_FONT = 'Microsoft YaHei'

export const fontRoles = [
  { key: 'interface', label: '界面字体', desc: '菜单、按钮、表单和设置页面', sample: '幻述 Studio · 保存设置' },
  { key: 'title', label: '标题字体', desc: '幻灯片主标题、封面及章节标题', sample: '让每一个想法清晰呈现' },
  { key: 'subtitle', label: '副标题字体', desc: '封面副标题、章节说明和英文副标题', sample: '从灵感到表达 · Ideas into Stories' },
  { key: 'body', label: '正文字体', desc: '幻灯片内容、卡片正文和 Markdown 预览', sample: '将 Markdown 文档转化为精美演示文稿。' },
  { key: 'caption', label: '注释与标签字体', desc: '脚注、页尾说明、标签和辅助信息', sample: '资料来源 · 重点提示 · 更新于今日' },
  { key: 'number', label: '数字与序号字体', desc: '章节编号、步骤序号和时间轴年份', sample: '01 02 03 · 2026 · 1234567890' },
  { key: 'code', label: '代码与编辑器字体', desc: 'Markdown 编辑器、代码块和技术信息', sample: '## Hello 幻述 · const value = 42' },
] as const

export type FontRole = typeof fontRoles[number]['key']
export type FontSettings = Record<FontRole, string>

export interface SystemFont {
  name: string
  label: string
  aliases: string[]
}

function searchKey(value: string) {
  return value.trim().toLocaleLowerCase().replace(/\s+/g, '')
}

export function matchesFont(font: SystemFont, query: string) {
  const key = searchKey(query)
  return [font.name, font.label, ...font.aliases].some(name => searchKey(name).includes(key))
}

export function findSystemFont(fonts: SystemFont[], name: string) {
  const key = searchKey(name)
  return fonts.find(font => [font.name, font.label, ...font.aliases].some(alias => searchKey(alias) === key))
}

export function normalizeFonts(value?: Partial<FontSettings> | null): FontSettings {
  return Object.fromEntries(fontRoles.map(({ key }) => {
    const name = value?.[key]
    return [key, typeof name === 'string' && name.trim() ? name.trim() : DEFAULT_FONT]
  })) as FontSettings
}

// Treat input as one literal family name, including quotes and CSS punctuation.
export function fontFamily(name: string): string {
  const escaped = (name.trim() || DEFAULT_FONT)
    .replace(/\\/g, '\\\\').replace(/"/g, '\\"')
    .replace(/[\x00-\x1f\x7f]/g, ' ')
  return `"${escaped}", "Microsoft YaHei", sans-serif`
}

export function applyFonts(value: FontSettings) {
  const fonts = normalizeFonts(value)
  for (const { key } of fontRoles) {
    document.documentElement.style.setProperty(`--font-${key}`, fontFamily(fonts[key]))
  }
  // Inline variables also keep theme changes from replacing user preferences.
  document.documentElement.style.setProperty('--font-sans', 'var(--font-interface)')
  document.documentElement.style.setProperty('--font-mono', 'var(--font-code)')
}
