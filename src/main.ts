import { createApp } from 'vue'
import { createPinia } from 'pinia'
import { createRouter, createWebHashHistory } from 'vue-router'
import App from './App.vue'
import LauncherView from './views/LauncherView.vue'
import './assets/base.css'

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: '/', name: 'launcher', component: LauncherView },
    { path: '/catalog', name: 'catalog', component: () => import('./views/CatalogView.vue') },
    {
      path: '/frequent',
      name: 'frequent',
      component: () => import('./views/FrequentProjectsView.vue'),
      props: { mode: 'favorites' },
    },
    {
      path: '/recent',
      name: 'recent',
      component: () => import('./views/FrequentProjectsView.vue'),
      props: { mode: 'recent' },
    },
    { path: '/logs', name: 'logs', component: () => import('./views/LogsView.vue') },
    { path: '/settings', name: 'settings', component: () => import('./views/SettingsView.vue') },
    { path: '/help', name: 'help', component: () => import('./views/HelpView.vue') },
    { path: '/about', name: 'about', component: () => import('./views/AboutView.vue') },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
  scrollBehavior: () => ({ top: 0 }),
})

createApp(App).use(createPinia()).use(router).mount('#app')
