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
import 'katex/dist/katex.min.css'
import './style.css'
import './themes/terminal.css'
import './themes/notes.css'
import './themes/notes-studio.css'
import './themes/editorial.css'
import './themes/presentation.css'
import { DEFAULT_THEME, isKnownTheme } from './themes/catalog'

const savedTheme = localStorage.getItem('app-theme')
const theme = isKnownTheme(savedTheme) ? savedTheme : DEFAULT_THEME
document.documentElement.setAttribute('data-theme', theme)

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

app.use(pinia)
app.use(router)
app.mount('#app')
