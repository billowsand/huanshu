<script setup lang="ts">
import SlideTitle from './SlideTitle.vue'
import SlideReveal from './SlideReveal.vue'
import { renderRichText } from '../utils/richText'

defineProps<{
  badges?: string[]
  title: string
  subtitle?: string
  enTitle?: string
}>()
</script>

<template>
  <div class="cover-layout" relative z-10 flex flex-col items-center justify-center h-full gap-6>
    <SlideReveal v-if="badges?.length" :delay="100" flex items-center gap-3 mb-2>
      <div
        v-for="badge in badges"
        :key="badge"
        class="amber-badge tag-pill"
      >
        <span v-html="renderRichText(badge)"></span>
      </div>
    </SlideReveal>
    <SlideReveal :delay="200" text-center w-full min-w-0>
      <SlideTitle class="cover-title" :title="title" />
      <div v-if="subtitle" class="cover-subtitle">
        <span v-html="renderRichText(subtitle)"></span>
      </div>
    </SlideReveal>
    <SlideReveal :delay="350" class="divider-amber" w-64 mt-2 />
    <SlideReveal v-if="enTitle" :delay="450" class="cover-en-title text-center">
      <span v-html="renderRichText(enTitle)"></span>
    </SlideReveal>
  </div>

  <div absolute bottom-0 left-0 right-0 h-px class="divider-amber" />
</template>
