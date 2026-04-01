<script setup lang="ts">
import SlideReveal from './SlideReveal.vue'

defineProps<{
  badges?: string[]
  title: string
  subtitle?: string
  enTitle?: string
}>()
</script>

<template>
  <div relative z-10 flex flex-col items-center justify-center h-full gap-6>
    <SlideReveal v-if="badges?.length" :delay="100" flex items-center gap-3 mb-2>
      <div
        v-for="badge in badges"
        :key="badge"
        class="amber-badge tag-pill"
      >
        {{ badge }}
      </div>
    </SlideReveal>
    <SlideReveal :delay="200" text-center>
      <h1 class="cover-title">
        {{ title }}
      </h1>
      <div v-if="subtitle" class="cover-subtitle">
        {{ subtitle }}
      </div>
    </SlideReveal>
    <SlideReveal :delay="350" class="divider-amber" w-64 mt-2 />
    <SlideReveal v-if="enTitle" :delay="450" class="cover-en-title text-center">
      {{ enTitle }}
    </SlideReveal>
  </div>

  <div absolute bottom-0 left-0 right-0 h-px class="divider-amber" />
</template>
