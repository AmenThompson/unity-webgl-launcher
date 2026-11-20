<script setup lang="ts">
import { computed } from 'vue'
import { NButton, NInput, NSwitch } from 'naive-ui'
import { FileCode, FolderOpen, Play, SlidersHorizontal } from 'lucide-vue-next'
import { projectPath, type Preferences } from '../project'

const props = defineProps<{
  prefs: Preferences
  disabled: boolean
  starting: boolean
  running: boolean
}>()
const emit = defineEmits<{
  pick: [directory: boolean]
  start: []
  path: [value: string]
  threads: [value: boolean]
}>()
const ready = computed(() => !props.disabled && Boolean(projectPath(props.prefs.path)))
</script>

<template>
  <form
    class="panel project-form"
    aria-labelledby="project-config"
    @submit.prevent="ready && emit('start')"
  >
    <header class="section-header">
      <span class="section-icon"><SlidersHorizontal aria-hidden="true" /></span>
      <h2 id="project-config">项目配置</h2>
    </header>
    <label class="field-label" for="project-path">Unity WebGL 路径</label>
    <div class="path-row">
      <NInput
        class="path-input"
        :value="prefs.path"
        :disabled="disabled"
        type="text"
        placeholder="粘贴发布目录或 index.html 路径"
        :input-props="{ id: 'project-path', spellcheck: false, 'aria-describedby': 'path-hint' }"
        clearable
        @update:value="emit('path', $event)"
      />
      <div class="picker-actions">
        <NButton secondary :disabled="disabled" @click="emit('pick', true)"
          ><template #icon><FolderOpen :size="16" aria-hidden="true" /></template>选择文件夹</NButton
        >
        <NButton secondary :disabled="disabled" @click="emit('pick', false)"
          ><template #icon><FileCode :size="16" aria-hidden="true" /></template>选择入口文件</NButton
        >
      </div>
    </div>
    <slot />
    <div class="mode-field">
      <p class="field-label">自动识别构建格式</p>
      <p id="path-hint" class="field-hint">支持 Brotli、Gzip、未压缩及 Unity 内置解压回退。</p>
    </div>
    <div class="thread-row">
      <div>
        <label id="threads-label" class="field-label" for="threads-toggle">多线程支持</label>
        <p id="threads-hint" class="field-hint">仅在 Unity 构建启用线程时开启，以发送跨源隔离响应头。</p>
      </div>
      <NSwitch
        id="threads-toggle"
        :value="prefs.threads"
        :disabled="disabled"
        aria-labelledby="threads-label"
        aria-describedby="threads-hint"
        @update:value="emit('threads', $event)"
      />
    </div>
    <div class="form-footer">
      <p v-if="running" class="field-hint">停止当前服务后可修改配置。</p>
      <NButton type="primary" attr-type="submit" size="large" :disabled="!ready" :loading="starting"
        ><template #icon><Play :size="17" aria-hidden="true" /></template>启动并打开</NButton
      >
    </div>
  </form>
</template>

<style scoped>
.project-form {
  display: flex;
  flex-direction: column;
}
.field-label {
  display: block;
  margin: 0 0 8px;
  padding: 0;
  color: var(--text);
  font-size: 13px;
  font-weight: 650;
}
.field-hint {
  margin: 7px 0 0;
  color: var(--muted);
  font-size: 12px;
  line-height: 1.6;
}
.path-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr);
  gap: 8px;
}
.path-input {
  width: 100%;
  min-width: 0;
}
.picker-actions {
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 8px;
  margin-left: auto;
}
.mode-field {
  margin-top: 18px;
}
.mode-field .field-label {
  margin-bottom: 4px;
}
.mode-field .field-hint {
  margin: 0;
}
.thread-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 14px;
  margin-top: 16px;
  padding: 12px;
  border: 1px solid var(--border-soft);
  border-radius: 9px;
  background: var(--surface-inset);
}
.thread-row .field-label,
.thread-row .field-hint {
  margin: 0;
}
.thread-row .field-hint {
  margin-top: 3px;
}
.form-footer {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  flex-wrap: wrap;
  gap: 8px 12px;
  margin-top: auto;
  padding-top: 17px;
}
.form-footer .field-hint {
  margin: 0;
}
</style>
