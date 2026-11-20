<script setup lang="ts">
import { onBeforeUnmount, onMounted, shallowRef } from 'vue'
import { BookOpen } from 'lucide-vue-next'
import axios from 'axios'
import MarkdownContent from '../components/MarkdownContent.vue'
import helpUrl from '../assets/help.md?url&no-inline'
import helpFallback from '../assets/help.md?raw'

const help = shallowRef(helpFallback)
const request = new AbortController()

onMounted(async () => {
  try {
    const response = await axios.get<string>(helpUrl, {
      responseType: 'text',
      signal: request.signal,
      timeout: 4000,
    })
    if (typeof response.data === 'string' && response.data.trimStart().startsWith('## '))
      help.value = response.data
  } catch {
    /* The in-bundle copy works when local asset fetching is unavailable. */
  }
})
onBeforeUnmount(() => request.abort())
</script>

<template>
  <section class="panel page-panel" aria-labelledby="guide-title">
    <header class="section-header">
      <span class="section-icon"><BookOpen aria-hidden="true" /></span>
      <h1 id="guide-title">使用指南</h1>
    </header>
    <MarkdownContent :source="help" />
  </section>
</template>

<style scoped>
h1 {
  margin: 0;
  font-size: clamp(20px, 3vw, 27px);
}
</style>
