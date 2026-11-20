<script setup lang="ts">
import { computed, onMounted, shallowRef } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { NAlert, NButton, NInput, NSwitch, NTag } from 'naive-ui'
import { FolderOpen, Play, RefreshCw, Star } from 'lucide-vue-next'
import { desktop } from '../desktop'
import { errorText, formatLabels, projectPath, sameProjectRoot, type CatalogScan } from '../project'
import { useSession } from '../session'

const session = useSession()
const savedRoot = useLocalStorage('unity-webgl.catalog.root', '')
const directory = shallowRef(session.defaultDirectory || savedRoot.value)
const catalog = shallowRef<CatalogScan | null>(null)
const selectedEntry = shallowRef('')
const query = shallowRef('')
const scanBusy = shallowRef(false)
const scanError = shallowRef('')

const projects = computed(() => catalog.value?.projects ?? [])
const selected = computed(() => projects.value.find((project) => project.entry === selectedEntry.value))
const visibleProjects = computed(() => {
  const term = query.value.trim().toLowerCase()
  return projects.value
    .filter((project) => `${project.name} ${project.relativeEntry}`.toLowerCase().includes(term))
    .sort(
      (left, right) => Number(session.isFavorite(right.root)) - Number(session.isFavorite(left.root)),
    )
})
const selectedIsRunning = computed(
  () =>
    !!selected.value && session.running && sameProjectRoot(session.snapshot.root, selected.value.root),
)
const launchLabel = computed(() =>
  selectedIsRunning.value ? '重新打开' : session.running ? '切换并打开' : '启动并打开',
)

function selectEntry(entry: string) {
  selectedEntry.value = entry
}

async function runScan(pick = false) {
  if (!desktop.available || scanBusy.value || (pick && session.operation)) return
  scanBusy.value = true
  scanError.value = ''
  try {
    if (pick) {
      const result = await desktop.pickCatalog()
      if (!result) return
      directory.value = result
    }
    const path = projectPath(directory.value)
    if (!path) return
    const result = await desktop.scan(path)
    catalog.value = result
    savedRoot.value = result.root
    directory.value = result.root
    if (!result.projects.some((project) => project.entry === selectedEntry.value)) {
      selectEntry(
        (
          result.projects.find((project) => sameProjectRoot(session.snapshot.root, project.root)) ??
          result.projects.find((project) => !project.issues.length) ??
          result.projects[0]
        )?.entry ?? '',
      )
    }
  } catch (cause) {
    scanError.value = errorText(cause)
  } finally {
    scanBusy.value = false
  }
}

function launch() {
  if (!catalog.value || !selected.value) return
  const project = selected.value
  void session.launchCatalog(project, catalog.value.root, session.projectThreads(project.root))
}

onMounted(() => {
  if (directory.value) void runScan()
})
</script>

<template>
  <section class="catalog-intro"><h1>多项目</h1></section>
  <NAlert v-if="!desktop.available" type="info" class="catalog-gap"
    >请在 Windows 启动器中扫描本地目录。</NAlert
  >
  <NAlert v-if="scanError" type="error" class="catalog-gap" title="扫描失败" role="alert">{{
    scanError
  }}</NAlert>
  <NAlert
    v-if="session.notice"
    type="warning"
    class="catalog-gap"
    :title="session.notice.label"
    role="alert"
    closable
    @close="session.dismissNotice()"
    >{{ session.notice.text }}</NAlert
  >
  <NAlert v-if="catalog?.truncated || catalog?.skipped" class="catalog-gap" type="info"
    >{{ catalog?.truncated ? '已达到扫描范围上限，可选择更具体的子目录继续扫描。' : ''
    }}{{ catalog?.skipped ? `跳过 ${catalog.skipped} 个不可访问或不在扫描范围内的条目。` : '' }}</NAlert
  >
  <div class="catalog-grid">
    <section class="panel" aria-labelledby="found-heading">
      <header class="head">
        <h2 id="found-heading">发现的发布项目</h2>
        <span v-if="catalog" aria-live="polite" class="muted">{{ visibleProjects.length }} 个项目</span>
      </header>
      <form class="directory-form" @submit.prevent="runScan()">
        <label class="field" for="catalog-path">发布总目录</label>
        <NInput
          v-model:value="directory"
          :disabled="scanBusy"
          placeholder="输入包含一个或多个 Unity WebGL 发布的目录"
          :input-props="{ id: 'catalog-path' }"
          clearable
        />
        <div class="actions">
          <NButton
            secondary
            :disabled="!desktop.available || scanBusy || !!session.operation"
            @click="runScan(true)"
            ><template #icon><FolderOpen :size="16" /></template>选择总目录</NButton
          >
          <NButton
            secondary
            attr-type="submit"
            :loading="scanBusy"
            :disabled="!desktop.available || !projectPath(directory) || !!session.operation"
            ><template #icon><RefreshCw :size="15" /></template
            >{{ catalog ? '重新扫描' : '扫描项目' }}</NButton
          >
        </div>
      </form>
      <NInput
        v-if="projects.length"
        v-model:value="query"
        placeholder="搜索项目名称或路径"
        :input-props="{ 'aria-label': '搜索项目名称或路径' }"
        clearable
      />
      <p v-if="!catalog" class="muted">选择发布总目录以发现项目。</p>
      <p v-else-if="!projects.length" class="muted">
        该目录下未发现 Unity WebGL 发布。请选择包含完整发布文件的上级目录。
      </p>
      <p v-else-if="!visibleProjects.length" class="muted">没有匹配的项目。</p>
      <ul v-else class="project-list">
        <li v-for="project in visibleProjects" :key="project.entry" class="project-list-item">
          <button
            type="button"
            class="project-row"
            :aria-pressed="selectedEntry === project.entry"
            @click="selectEntry(project.entry)"
          >
            <FolderOpen class="project-icon" :size="18" aria-hidden="true" />
            <span class="project-title"
              ><strong>{{ project.name }}</strong
              ><small class="mono">{{ project.relativeEntry }}</small></span
            >
            <NTag
              v-if="session.running && sameProjectRoot(session.snapshot.root, project.root)"
              size="small"
              type="success"
              round
              >运行中</NTag
            >
            <NTag v-else-if="project.issues.length" size="small" type="warning" round>缺少资源</NTag>
          </button>
          <NButton
            class="favorite project-action"
            quaternary
            circle
            :aria-pressed="session.isFavorite(project.root)"
            :aria-label="`${session.isFavorite(project.root) ? '取消收藏' : '收藏'} ${project.name}`"
            @click="session.toggleFavorite(project.root)"
            ><Star :size="34" aria-hidden="true"
          /></NButton>
        </li>
      </ul>
    </section>
    <section class="panel detail" aria-labelledby="detail-heading">
      <header class="head"><h2 id="detail-heading">选中项目</h2></header>
      <template v-if="selected">
        <p class="selected-name">{{ selected.name }}</p>
        <dl>
          <div>
            <dt>入口文件</dt>
            <dd class="mono">{{ selected.entry }}</dd>
          </div>
          <div>
            <dt>压缩格式</dt>
            <dd>{{ formatLabels[selected.format] }}</dd>
          </div>
          <div>
            <dt>Decompression Fallback</dt>
            <dd>
              {{ selected.fallback === null ? '未确认' : selected.fallback ? '已启用' : '未启用' }}
            </dd>
          </div>
        </dl>
        <NAlert v-if="selected.issues.length" type="warning" title="项目需要检查" class="issues"
          ><ul>
            <li v-for="issue in selected.issues" :key="issue">{{ issue }}</li>
          </ul></NAlert
        >
      </template>
      <p v-else class="muted">从左侧列表选择项目。</p>
      <div class="thread-row">
        <label id="catalog-threads" for="catalog-thread-toggle">多线程支持</label>
        <NSwitch
          id="catalog-thread-toggle"
          :value="selected ? session.projectThreads(selected.root) : false"
          aria-labelledby="catalog-threads"
          :disabled="!selected || selectedIsRunning || !!selected.issues.length || !!session.operation"
          :title="selectedIsRunning ? '停止当前项目后可修改' : undefined"
          @update:value="selected && session.setProjectThreads(selected.root, $event)"
        />
      </div>
      <div class="actions detail-actions">
        <NButton v-if="session.running" secondary :disabled="!!session.operation" @click="session.stop()"
          >停止当前项目</NButton
        >
        <NButton
          v-if="selected"
          type="primary"
          class="launch"
          :disabled="(!selectedIsRunning && !!selected.issues.length) || scanBusy || !!session.operation"
          @click="launch"
          ><template #icon><Play :size="16" /></template>{{ launchLabel }}</NButton
        >
      </div>
    </section>
  </div>
</template>

<style scoped>
.catalog-intro {
  display: flex;
  align-items: center;
  min-height: var(--project-heading-height);
  margin-bottom: 14px;
}
.catalog-intro h1 {
  margin: 0;
  font-size: clamp(20px, 3vw, 24px);
  line-height: 1.35;
}
.catalog-gap {
  margin-bottom: 14px;
}
.directory-form {
  margin-bottom: 12px;
}
.field {
  display: block;
  margin-bottom: 6px;
  font-weight: 650;
}
.catalog-grid {
  display: grid;
  flex: 1 0 0;
  min-height: 350px;
  grid-template-columns: var(--project-columns);
  grid-template-rows: minmax(0, 1fr);
  gap: 14px;
}
.catalog-grid > .panel {
  min-height: 0;
  overflow: auto;
}
.head {
  display: flex;
  justify-content: space-between;
  align-items: baseline;
  gap: 8px;
  margin-bottom: 11px;
}
.head h2 {
  margin: 0;
  font-size: 17px;
}
.muted {
  color: var(--muted);
  font-size: 12px;
}
.project-list {
  display: grid;
  gap: 3px;
  padding: 2px 4px;
  margin: 10px -4px 0;
  list-style: none;
}
.project-list-item {
  display: flex;
  align-items: center;
  gap: 5px;
}
.favorite[aria-pressed='true'] svg {
  color: #8a620c;
  fill: #f3c96a;
}
.project-row {
  display: flex;
  align-items: center;
  gap: 11px;
  min-width: 0;
  width: 100%;
  text-align: left;
  padding: 10px 12px;
  border: 1px solid transparent;
  border-bottom-color: var(--border);
  border-radius: 5px;
  background: var(--surface-field);
  color: var(--text);
  cursor: pointer;
}
.project-row:hover {
  background: var(--surface-inset);
}
.project-row[aria-pressed='true'] {
  background: var(--accent-wash);
  border-color: var(--border);
  border-inline-start-color: var(--accent);
}
.project-icon {
  color: var(--accent);
  flex: none;
}
.project-title {
  flex: 1;
  min-width: 0;
  overflow-wrap: anywhere;
}
.project-title small {
  display: block;
  color: var(--muted);
}
.detail.panel {
  background: var(--inspector);
  display: flex;
  flex-direction: column;
}
.selected-name {
  margin: 0;
  font-size: 18px;
  font-weight: 700;
  overflow-wrap: anywhere;
}
.detail dl {
  margin: 9px 0;
  display: grid;
  gap: 7px;
}
.detail dt {
  color: var(--muted);
  font-size: 12px;
}
.detail dd {
  margin: 1px 0 0;
  overflow-wrap: anywhere;
}
.issues ul {
  padding-left: 17px;
  margin: 0;
  overflow-wrap: anywhere;
}
.thread-row {
  display: flex;
  justify-content: space-between;
  gap: 10px;
  margin: 5px 0;
}
.actions {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
  padding-top: 9px;
}
.detail-actions {
  margin-top: auto;
}
@media (max-width: 770px) {
  .catalog-grid {
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: repeat(2, minmax(290px, 1fr));
    min-height: 594px;
  }
}
</style>
