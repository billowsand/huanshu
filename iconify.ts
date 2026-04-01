import { createRequire } from 'module'

type IconSet = {
  icons: Record<string, unknown>
}

const require = createRequire(import.meta.url)
const carbonIconSet = require('@iconify-json/carbon/icons.json') as IconSet

export const carbonCollection = () => Promise.resolve(carbonIconSet as object)

// Runtime slide JSON can contain any Carbon icon name, so safelist the
// generated utility classes instead of relying on static template scanning.
export const carbonSafelist = Object.keys(carbonIconSet.icons).map((name) => `i-carbon:${name}`)
