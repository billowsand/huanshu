import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import UnoCSS from 'unocss/vite'
import { presetWind3, presetAttributify, presetIcons } from 'unocss'
import { fileURLToPath, URL } from 'url'
import { carbonCollection, carbonSafelist } from './iconify'

export default defineConfig({
  plugins: [
    vue(),
    UnoCSS({
      safelist: carbonSafelist,
      presets: [
        presetWind3({ dark: 'class' }),
        presetAttributify(),
        presetIcons({
          prefix: 'i-',
          extraProperties: { display: 'inline-block', 'vertical-align': 'middle' },
          collections: {
            carbon: carbonCollection,
          },
        }),
      ],
    }),
  ],
  resolve: {
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
  build: {
    outDir: 'src-tauri/dist-studio',
    emptyOutDir: true,
  },
  server: {
    port: 5173,
    strictPort: true,
  },
})
