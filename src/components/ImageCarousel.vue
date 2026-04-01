<script setup>
import { ref, onMounted, onUnmounted } from 'vue'
import { convertFileSrc } from '@tauri-apps/api/core'

const props = defineProps({
  images: { type: Array, required: true },
  interval: { type: Number, default: 3000 },
})

// 让 Vite 在构建时静态分析并打包 figure 目录下所有图片
const modules = import.meta.glob('/figure/**/*.{png,jpg,jpeg,gif,webp}', { eager: true, query: '?url', import: 'default' })

const src = (name) => {
  if (/^(asset:|blob:|data:|https?:)/.test(name))
    return name
  if (/^(\/|[A-Za-z]:[\\/])/.test(name))
    return convertFileSrc(name)
  const key = name.startsWith('/') ? name : `/figure/${name}`
  return modules[key] ?? key
}

const idx = ref(0)
let timer

onMounted(() => {
  if (props.images.length > 1)
    timer = setInterval(() => { idx.value = (idx.value + 1) % props.images.length }, props.interval)
})

onUnmounted(() => clearInterval(timer))
</script>

<template>
  <div relative w-full h-full flex items-center justify-center>
    <img :src="src(images[idx])" object-contain max-h-full max-w-full rounded-lg />
    <div v-if="images.length > 1" absolute bottom-2 left-0 right-0 flex justify-center gap-1>
      <div
        v-for="(_, i) in images" :key="i"
        :class="i === idx ? 'bg-amber-400 w-4' : 'bg-white/30 w-2'"
        h-1.5 rounded-full style="transition: all 0.3s ease;"
      />
    </div>
  </div>
</template>
