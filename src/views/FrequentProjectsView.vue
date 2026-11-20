<script setup lang="ts">
import { computed, shallowRef, watch } from 'vue'
import { useEventListener } from '@vueuse/core'
import { NAlert, NButton, NSwitch, NTag } from 'naive-ui'
import { FolderOpen, Play, Star, Trash2, X } from 'lucide-vue-next'
import { desktop } from '../desktop'
import { errorText, projectKey, sameProjectRoot } from '../project'
import { useSession } from '../session'

const session = useSession()
const props = defineProps<{ mode: 'favorites' | 'recent' }>()
const title = computed(() => (props.mode === 'favorites' ? '收藏项目' : '最近项目'))
const selectedPath = shallowRef('')
const validity = shallowRef(new Map<string, boolean>())
const checking = shallowRef(false)
const checkError = shallowRef('')
const paths = computed(() =>
  props.mode === 'favorites' ? session.favoritePaths : session.recentProjects.map(({ root }) => root),
)
const selected = computed(
  () => paths.value.find((path) => sameProjectRoot(path, selectedPath.value)) ?? paths.value[0],
)
const active = computed(() =>
  Boolean(selected.value && session.running && sameProjectRoot(session.snapshot.root, selected.value)),
)
const launchLabel = computed(() =>
  active.value ? '在浏览器中重新打开' : session.running ? '切换并打开' : '启动并打开',
)
let revision = 0

function nameOf(path: string) {
  return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path
}

async function checkPaths() {
  const current = paths.value.slice()
  const request = ++revision
  checkError.value = ''
  checking.value = desktop.available && current.length > 0
  if (!checking.value) return
  try {
    for (let start = 0; start < current.length && request === revision; start += 32) {
      const batch = current.slice(start, start + 32)
      const result = await desktop.checkRecent(batch)
      if (request !== revision) return
      const next = new Map(validity.value)
      batch.forEach((path, index) => next.set(projectKey(path), result[index] === true))
      validity.value = next
    }
  } catch (cause) {
    if (request === revision) checkError.value = errorText(cause)
  } finally {
    if (request === revision) checking.value = false
  }
}

function launch() {
  if (selected.value && (active.value || validity.value.get(projectKey(selected.value)))) {
    void session.launchSaved(selected.value)
  }
}

function remove(path: string) {
  if (props.mode === 'favorites') session.toggleFavorite(path)
  else session.removeRecent(path)
}

watch(paths, checkPaths, { immediate: true })
useEventListener(window, 'focus', checkPaths)
</script>

<template>
  <header class="saved-page-header">
    <h1 class="page-title">{{ title }}</h1>
    <NButton
      v-if="props.mode === 'recent' && paths.length"
      size="small"
      secondary
      @click="session.clearRecent()"
    >
      <template #icon><Trash2 :size="14" aria-hidden="true" /></template>清空历史
    </NButton>
  </header>
  <NAlert v-if="!desktop.available" class="notice" type="info"
    >请在 Windows 启动器中打开本地项目。</NAlert
  >
  <NAlert v-if="checkError" class="notice" type="warning" title="路径检查失败" role="alert">{{
    checkError
  }}</NAlert>
  <NAlert
    v-if="session.notice"
    class="notice"
    type="warning"
    :title="session.notice.label"
    role="alert"
    closable
    @close="session.dismissNotice()"
    >{{ session.notice.text }}</NAlert
  >
  <div class="layout">
    <section class="panel" :aria-label="title + '列表'">
      <p v-if="!paths.length" class="muted">
        {{
          props.mode === 'favorites'
            ? '在「多项目」列表中点击星标即可收藏。'
            : '成功启动的项目会出现在这里。'
        }}
      </p>
      <ul v-else class="entries">
        <li v-for="path in paths" :key="projectKey(path)" class="entry-row">
          <button
            type="button"
            class="entry"
            :aria-pressed="path === selected"
            @click="selectedPath = path"
          >
            <FolderOpen :size="17" aria-hidden="true" />
            <span class="entry-text"
              ><strong>{{ nameOf(path) }}</strong
              ><small class="mono">{{ path }}</small></span
            >
            <NTag
              v-if="session.running && sameProjectRoot(session.snapshot.root, path)"
              type="success"
              size="small"
              >运行中</NTag
            >
            <NTag v-else-if="validity.get(projectKey(path)) === false" type="warning" size="small"
              >路径失效</NTag
            >
          </button>
          <NButton
            class="entry-action project-action"
            size="tiny"
            quaternary
            circle
            :aria-label="`${props.mode === 'favorites' ? '取消收藏' : '移除记录'} ${nameOf(path)}`"
            :title="props.mode === 'favorites' ? '取消收藏' : '移除记录'"
            @click="remove(path)"
          >
            <Star
              v-if="props.mode === 'favorites'"
              :size="34"
              color="#a36921"
              fill="#f3c96a"
              aria-hidden="true"
            />
            <X v-else :size="28" aria-hidden="true" />
          </NButton>
        </li>
      </ul>
    </section>
    <section class="panel details" aria-labelledby="saved-selection">
      <h2 id="saved-selection">选中项目</h2>
      <template v-if="selected">
        <p class="project-name">{{ nameOf(selected) }}</p>
        <dl>
          <dt>发布路径</dt>
          <dd class="mono">{{ selected }}</dd>
        </dl>
        <p v-if="validity.get(projectKey(selected)) === false" class="muted">
          未找到可读取的项目入口。若目录已移动，可从新位置打开项目。
        </p>
        <div v-else-if="!active" class="thread-row">
          <label id="saved-threads-label" for="saved-threads">多线程支持</label>
          <NSwitch
            id="saved-threads"
            :value="session.projectThreads(selected)"
            aria-labelledby="saved-threads-label"
            :disabled="!!session.operation || !desktop.available"
            @update:value="session.setProjectThreads(selected, $event)"
          />
        </div>
      </template>
      <p v-else class="muted">从列表选中项目后即可打开。</p>
      <p v-if="session.running && session.snapshot.root" class="active" role="status">
        当前运行 <strong>{{ nameOf(session.snapshot.root) }}</strong>
      </p>
      <div class="actions">
        <NButton v-if="session.running" secondary :disabled="!!session.operation" @click="session.stop()"
          >停止当前项目</NButton
        >
        <NButton
          v-if="selected"
          type="primary"
          :disabled="
            !desktop.available ||
            !!session.operation ||
            (!active && (checking || validity.get(projectKey(selected)) !== true))
          "
          @click="launch"
          ><template #icon><Play :size="16" aria-hidden="true" /></template>{{ launchLabel }}</NButton
        >
      </div>
    </section>
  </div>
</template>

<style scoped>
.saved-page-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 8px;
  min-height: var(--project-heading-height);
  margin-bottom: 14px;
}
.page-title {
  margin: 0;
  font-size: clamp(20px, 3vw, 24px);
  line-height: 1.35;
}
.notice {
  margin-bottom: 14px;
  overflow-wrap: anywhere;
}
.layout {
  display: grid;
  grid-template-columns: var(--project-columns);
  gap: 14px;
  flex: 1 0 0;
  min-height: 345px;
  grid-template-rows: minmax(0, 1fr);
}
.layout > .panel {
  min-height: 0;
  overflow: auto;
}
.entries {
  list-style: none;
  margin: 10px 0 0;
  padding: 2px 3px;
  display: grid;
  gap: 3px;
  max-height: 415px;
  overflow: auto;
}
.entry-row {
  display: flex;
  align-items: center;
  gap: 5px;
  min-width: 0;
}
.entry {
  display: flex;
  align-items: center;
  flex: 1;
  min-width: 0;
  gap: 8px;
  width: 100%;
  padding: 9px 10px;
  text-align: left;
  color: var(--text);
  background: var(--surface-field);
  border: 1px solid transparent;
  border-bottom-color: var(--border);
  border-radius: 5px;
  cursor: pointer;
}
.entry svg {
  flex: none;
  color: var(--accent);
}
.entry[aria-pressed='true'] {
  background: var(--accent-wash);
  border-color: var(--border);
  border-inline-start-color: var(--accent);
}
.entry-text {
  min-width: 0;
  flex: 1;
  overflow-wrap: anywhere;
}
.entry-text small {
  display: block;
  color: var(--muted);
}
.details.panel {
  background: var(--inspector);
  display: flex;
  flex-direction: column;
}
.details h2 {
  margin: 0 0 10px;
  font-size: 17px;
}
.project-name {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  overflow-wrap: anywhere;
}
.details dl {
  margin: 10px 0;
}
.details dt,
.muted {
  color: var(--muted);
  font-size: 12px;
}
.details dd {
  margin: 2px 0 0;
  overflow-wrap: anywhere;
}
.thread-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  margin: 5px 0 10px;
}
.active {
  padding: 8px 10px;
  margin: 8px 0 0;
  color: var(--mint-text);
  border-inline-start: 2px solid var(--mint-text);
  background: var(--surface-field);
  overflow-wrap: anywhere;
}
.actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  flex-wrap: wrap;
  margin-top: auto;
  padding-top: 12px;
}
@media (max-width: 770px) {
  .layout {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: repeat(2, minmax(270px, 1fr));
    min-height: 554px;
  }
}
</style>
