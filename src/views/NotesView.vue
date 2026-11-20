<script setup lang="ts">
import { defineAsyncComponent, shallowRef } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { NAlert, NButton } from 'naive-ui'
import { NotebookPen } from 'lucide-vue-next'
import MarkdownContent from '../components/MarkdownContent.vue'

const NotesEditor = defineAsyncComponent(() => import('../components/NotesEditor.vue'))
const notes = useLocalStorage('unity-webgl.launcher.notes.v1', '')
const editing = shallowRef(false)
</script>

<template>
  <section class="panel page-panel" aria-labelledby="notes-title">
    <header class="section-header notes-header">
      <span class="section-icon"><NotebookPen aria-hidden="true" /></span>
      <h1 id="notes-title">本地笔记</h1>
      <NButton
        class="edit-button"
        secondary
        type="primary"
        :aria-expanded="editing"
        @click="editing = !editing"
      >
        {{ editing ? '完成编辑' : '编辑笔记' }}
      </NButton>
    </header>
    <p class="privacy-note">笔记自动保存在本机，支持 Markdown。</p>
    <Suspense v-if="editing">
      <NotesEditor v-model="notes" />
      <template #fallback><NAlert type="info" role="status">正在加载编辑器…</NAlert></template>
    </Suspense>
    <MarkdownContent v-if="notes.trim()" :source="notes" aria-label="笔记预览" />
    <p v-else-if="!editing" class="privacy-note">暂无笔记。</p>
  </section>
</template>

<style scoped>
.notes-header {
  margin-bottom: 8px;
  flex-wrap: wrap;
}
.notes-header h1 {
  margin: 0;
  font-size: clamp(20px, 3vw, 27px);
}
.edit-button {
  margin-left: auto;
}
.privacy-note {
  margin: 8px 0 12px;
  color: var(--muted);
}
</style>
