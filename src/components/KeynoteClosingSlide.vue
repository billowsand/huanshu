<script setup lang="ts">
import SlideReveal from './SlideReveal.vue'

defineProps<{
  badges?: string[]
  title: string
  subtitle?: string
  note?: string
}>()
</script>

<template>
  <div relative z-10 flex flex-col justify-center h-full px-12>
    <SlideReveal v-if="badges?.length" :delay="100" flex items-center gap-3 mb-6>
      <div
        v-for="badge in badges"
        :key="badge"
        class="amber-badge tag-pill"
      >
        {{ badge }}
      </div>
    </SlideReveal>

    <SlideReveal :delay="180">
      <div class="closing-kicker">
        END
      </div>
      <h1 class="closing-title">
        {{ title }}
      </h1>
      <div v-if="subtitle" class="closing-subtitle">
        {{ subtitle }}
      </div>
    </SlideReveal>

    <SlideReveal :delay="320" class="divider-amber" w-72 mt-5 />

    <SlideReveal v-if="note" :delay="420" class="closing-note">
      {{ note }}
    </SlideReveal>
  </div>

  <div absolute right-14 bottom-10 class="closing-mark">
    Auto Slidev Studio
  </div>
</template>

<style scoped>
.closing-kicker {
  font-size: 0.78rem;
  letter-spacing: 0.52em;
  text-transform: uppercase;
  color: rgba(245, 158, 11, 0.75);
  margin-bottom: 1rem;
}

.closing-title {
  font-size: 2.8rem;
  line-height: 1.08;
  font-weight: 800;
  color: var(--text);
  letter-spacing: -0.04em;
}

.closing-subtitle {
  margin-top: 1rem;
  font-size: 1.18rem;
  line-height: 1.7;
  color: rgba(226, 232, 240, 0.84);
  max-width: 40rem;
}

.closing-note {
  margin-top: 1.2rem;
  max-width: 34rem;
  font-size: 0.94rem;
  line-height: 1.75;
  color: rgba(148, 163, 184, 0.92);
}

.closing-mark {
  font-size: 0.72rem;
  letter-spacing: 0.16em;
  text-transform: uppercase;
  color: rgba(148, 163, 184, 0.54);
}
</style>
