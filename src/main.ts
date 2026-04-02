import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import HomeView from './views/HomeView.vue'
import WorkflowView from './views/WorkflowView.vue'
import SettingsView from './views/SettingsView.vue'
import FirstRunWizard from './views/FirstRunWizard.vue'
import 'virtual:uno.css'
import '@unocss/reset/tailwind.css'
import './style.css'

// Keynote slide components
import GlassCard from './components/GlassCard.vue'
import TagBadge from './components/TagBadge.vue'
import CardGrid from './components/CardGrid.vue'
import ImageCarousel from './components/ImageCarousel.vue'
import StepNumber from './components/StepNumber.vue'
import KeynoteCoverSlide from './components/KeynoteCoverSlide.vue'
import KeynoteOverviewSlide from './components/KeynoteOverviewSlide.vue'
import KeynoteFeatureGridSlide from './components/KeynoteFeatureGridSlide.vue'
import KeynoteSpotlightSlide from './components/KeynoteSpotlightSlide.vue'
import KeynoteSplitLayersSlide from './components/KeynoteSplitLayersSlide.vue'
import KeynoteSectionListSlide from './components/KeynoteSectionListSlide.vue'
import KeynoteFocusExampleSlide from './components/KeynoteFocusExampleSlide.vue'
import KeynoteOutcomeGridSlide from './components/KeynoteOutcomeGridSlide.vue'
import KeynoteCenterGridSlide from './components/KeynoteCenterGridSlide.vue'
import KeynoteTimelineSlide from './components/KeynoteTimelineSlide.vue'
import KeynoteStepFlowSlide from './components/KeynoteStepFlowSlide.vue'
import KeynoteProcessSlide from './components/KeynoteProcessSlide.vue'
import KeynoteCompareSlide from './components/KeynoteCompareSlide.vue'
import KeynoteIssueStackSlide from './components/KeynoteIssueStackSlide.vue'
import KeynoteSwotSlide from './components/KeynoteSwotSlide.vue'

const savedTheme = localStorage.getItem('app-theme') || 'dark-yellow'
document.documentElement.setAttribute('data-theme', savedTheme)

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', component: HomeView },
    { path: '/new', component: WorkflowView },
    { path: '/project/:id', component: WorkflowView },
    { path: '/settings', component: SettingsView },
    { path: '/wizard', component: FirstRunWizard },
  ],
})

const pinia = createPinia()
const app = createApp(App)

// Global keynote components
app.component('GlassCard', GlassCard)
app.component('TagBadge', TagBadge)
app.component('CardGrid', CardGrid)
app.component('ImageCarousel', ImageCarousel)
app.component('StepNumber', StepNumber)
app.component('KeynoteCoverSlide', KeynoteCoverSlide)
app.component('KeynoteOverviewSlide', KeynoteOverviewSlide)
app.component('KeynoteFeatureGridSlide', KeynoteFeatureGridSlide)
app.component('KeynoteSpotlightSlide', KeynoteSpotlightSlide)
app.component('KeynoteSplitLayersSlide', KeynoteSplitLayersSlide)
app.component('KeynoteSectionListSlide', KeynoteSectionListSlide)
app.component('KeynoteFocusExampleSlide', KeynoteFocusExampleSlide)
app.component('KeynoteOutcomeGridSlide', KeynoteOutcomeGridSlide)
app.component('KeynoteCenterGridSlide', KeynoteCenterGridSlide)
app.component('KeynoteTimelineSlide', KeynoteTimelineSlide)
app.component('KeynoteStepFlowSlide', KeynoteStepFlowSlide)
app.component('KeynoteProcessSlide', KeynoteProcessSlide)
app.component('KeynoteCompareSlide', KeynoteCompareSlide)
app.component('KeynoteIssueStackSlide', KeynoteIssueStackSlide)
app.component('KeynoteSwotSlide', KeynoteSwotSlide)

app.use(pinia)
app.use(router)
app.mount('#app')
