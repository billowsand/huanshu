import { defineConfig, presetWind3, presetAttributify, presetIcons } from 'unocss'
import { carbonCollection, carbonSafelist } from './iconify'

export default defineConfig({
  safelist: carbonSafelist,
  presets: [
    presetWind3({ dark: 'class' }),
    presetAttributify(),
    presetIcons({
      prefix: 'i-',
      extraProperties: {
        display: 'inline-block',
        'vertical-align': 'middle',
      },
      collections: {
        carbon: carbonCollection,
      },
    }),
  ],
})
