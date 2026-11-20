<script setup lang="ts">
import { shallowRef } from 'vue'
import { NAlert, NButton, NInput } from 'naive-ui'
import { FolderOpen, Settings } from 'lucide-vue-next'
import { desktop } from '../desktop'
import { errorText, projectPath } from '../project'
import { useSession } from '../session'

const session = useSession()
const draft = shallowRef(session.defaultDirectory)
const busy = shallowRef(false)
const error = shallowRef('')
const saved = shallowRef(false)

async function pickDirectory() {
  busy.value = true
  error.value = ''
  try {
    const selected = await desktop.pickCatalog()
    if (selected) draft.value = selected
  } catch (cause) {
    error.value = errorText(cause)
  } finally {
    busy.value = false
  }
}

async function saveDirectory() {
  if (!desktop.available || busy.value) return
  busy.value = true
  error.value = ''
  saved.value = false
  try {
    const path = projectPath(draft.value)
    const result = path ? await desktop.scan(path) : null
    session.defaultDirectory = result?.root ?? ''
    draft.value = session.defaultDirectory
    saved.value = true
  } catch (cause) {
    error.value = errorText(cause)
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <section class="panel page-panel" aria-labelledby="settings-title">
    <header class="section-header">
      <span class="section-icon"><Settings aria-hidden="true" /></span>
      <h1 id="settings-title">设置</h1>
    </header>
    <form class="directory-settings" @submit.prevent="saveDirectory">
      <label for="default-directory">默认路径</label>
      <NInput
        v-model:value="draft"
        :disabled="busy || !desktop.available"
        :input-props="{ id: 'default-directory', 'aria-describedby': 'directory-hint' }"
        placeholder="选择 Unity WebGL 发布目录或包含多个项目的总目录"
        clearable
        @update:value="saved = false"
      />
      <p id="directory-hint">进入单项目或多项目时自动扫描此目录。留空并保存可取消默认路径。</p>
      <div class="directory-actions">
        <NButton secondary :disabled="busy || !desktop.available" @click="pickDirectory">
          <template #icon><FolderOpen :size="16" aria-hidden="true" /></template>选择文件夹
        </NButton>
        <NButton type="primary" attr-type="submit" :disabled="busy || !desktop.available"
          >保存设置</NButton
        >
      </div>
      <NAlert v-if="error" type="error" role="alert">{{ error }}</NAlert>
      <p v-else-if="saved" role="status">
        {{ session.defaultDirectory ? '默认路径已保存。' : '默认路径已清除。' }}
      </p>
    </form>
  </section>
</template>

<style scoped>
h1 {
  margin: 0;
  font-size: clamp(20px, 3vw, 27px);
}
.directory-settings {
  display: grid;
  gap: 8px;
  max-width: 760px;
}
.directory-settings label {
  font-weight: 650;
}
.directory-settings p {
  margin: 0;
  color: var(--muted);
}
.directory-actions {
  display: flex;
  justify-content: flex-end;
  flex-wrap: wrap;
  gap: 8px;
}
</style>
