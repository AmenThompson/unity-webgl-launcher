<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue'
import { useEventListener } from '@vueuse/core'
import { NButton, NTag } from 'naive-ui'
import { Clock3, Trash2, X } from 'lucide-vue-next'
import { desktop } from '../desktop'
import { projectKey } from '../project'
import type { RecentProject } from '../projectPreferences'

const props = defineProps<{ projects: RecentProject[]; disabled: boolean }>()
const emit = defineEmits<{ select: [root: string]; remove: [root: string]; clear: [] }>()

const validity = shallowRef(new Map<string, boolean>())
const paths = computed(() => props.projects.map(({ root }) => root))
let checkId = 0

async function checkRecent() {
  const id = ++checkId
  const current = paths.value
  if (!desktop.available || !current.length) {
    validity.value = new Map()
    return
  }
  try {
    const result = await desktop.checkRecent(current)
    if (id !== checkId) return
    validity.value = new Map(
      current.flatMap((root, index) => {
        const exists = result[index]
        return typeof exists === 'boolean' ? [[root, exists] as const] : []
      }),
    )
  } catch {
    if (id === checkId) validity.value = new Map()
  }
}

watch(paths, checkRecent, { immediate: true })
useEventListener(window, 'focus', checkRecent)
</script>

<template>
  <section v-if="projects.length" class="recent" aria-labelledby="recent-title">
    <header class="recent-heading">
      <Clock3 :size="15" aria-hidden="true" />
      <h3 id="recent-title">最近项目</h3>
      <NButton class="clear" size="small" secondary @click="emit('clear')">
        <template #icon><Trash2 :size="14" aria-hidden="true" /></template>
        清空历史
      </NButton>
    </header>
    <ul class="recent-list">
      <li v-for="entry in projects" :key="projectKey(entry.root)" class="recent-row">
        <button
          type="button"
          class="recent-select mono"
          :title="entry.root"
          :aria-label="`选择最近项目 ${entry.root}`"
          :disabled="disabled || validity.get(entry.root) === false"
          @click="emit('select', entry.root)"
        >
          {{ entry.root }}
        </button>
        <NTag v-if="validity.get(entry.root) === false" size="small" type="warning">路径失效</NTag>
        <NButton
          class="project-action"
          quaternary
          circle
          size="tiny"
          :aria-label="`移除最近项目 ${entry.root}`"
          title="移除最近项目"
          @click="emit('remove', entry.root)"
          ><X :size="28" aria-hidden="true"
        /></NButton>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.recent {
  margin-top: 14px;
}
.recent-heading {
  display: flex;
  align-items: center;
  gap: 7px;
  margin-bottom: 4px;
  color: var(--muted);
}
.recent-heading h3 {
  margin: 0;
  font-size: 13px;
  color: var(--text);
}
.clear {
  margin-left: auto;
}
.recent-list {
  display: grid;
  gap: 5px;
  margin: 0;
  padding: 2px;
  max-height: 132px;
  overflow-y: auto;
  list-style: none;
}
.recent-row {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}
.recent-select {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  padding: 4px 6px;
  border: 1px solid var(--border-soft);
  border-radius: 6px;
  background: var(--surface-field);
  color: var(--accent);
  cursor: pointer;
  font-size: 12px;
}
.recent-select:disabled {
  color: var(--muted);
  cursor: default;
}
</style>
