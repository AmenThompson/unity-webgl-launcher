<script setup lang="ts">
import { NButton, useMessage } from 'naive-ui'
import { Activity, Copy, ExternalLink, Globe, Square } from 'lucide-vue-next'
import { copyText } from '../desktop'
import { errorText, formatLabels, type Snapshot } from '../project'

const props = defineProps<{ snapshot: Snapshot; operation: string | null }>()
const emit = defineEmits<{ open: []; stop: [] }>()
const message = useMessage()

async function copyUrl() {
  if (!props.snapshot.url) return
  try {
    await copyText(props.snapshot.url)
    message.success('预览地址已复制')
  } catch (cause) {
    message.error(errorText(cause))
  }
}
</script>

<template>
  <section class="panel runtime-panel" aria-labelledby="runtime-title">
    <header class="section-header">
      <span class="section-icon"><Activity aria-hidden="true" /></span>
      <h2 id="runtime-title">预览服务</h2>
    </header>
    <div class="service-state" :class="{ active: snapshot.url }">
      <span class="service-icon"><Globe :size="19" aria-hidden="true" /></span>
      <div class="state-text" role="status">
        <strong>{{ snapshot.url ? '服务运行中' : '服务未启动' }}</strong>
        <p v-if="!snapshot.url">启动项目后可查看访问地址和识别结果。</p>
      </div>
    </div>
    <dl v-if="snapshot.url" class="runtime-details">
      <div class="detail-row">
        <dt>预览地址</dt>
        <dd class="address">
          <code class="mono">{{ snapshot.url }}</code><NButton
            v-if="snapshot.url"
            quaternary
            circle
            aria-label="复制预览地址"
            title="复制预览地址"
            @click="copyUrl"
            ><template #icon><Copy :size="15" aria-hidden="true" /></template
          ></NButton>
        </dd>
      </div>
      <div class="detail-row">
        <dt>服务目录</dt>
        <dd>
          <code class="mono" :title="snapshot.root ?? undefined">{{ snapshot.root }}</code>
        </dd>
      </div>
      <div v-if="snapshot.format" class="detail-row">
        <dt>自动识别的压缩格式</dt>
        <dd>{{ formatLabels[snapshot.format] }}</dd>
      </div>
      <div v-if="snapshot.format" class="detail-row">
        <dt>Decompression Fallback</dt>
        <dd>{{ snapshot.fallback === null ? '未确认' : snapshot.fallback ? '已启用' : '未启用' }}</dd>
      </div>
    </dl>
    <div v-if="snapshot.url" class="runtime-actions">
      <NButton
        secondary
        :disabled="operation !== null"
        :loading="operation === 'open'"
        @click="emit('open')"
        ><template #icon><ExternalLink :size="16" aria-hidden="true" /></template>打开浏览器</NButton
      >
      <NButton
        secondary
        type="error"
        :disabled="operation !== null"
        :loading="operation === 'stop'"
        @click="emit('stop')"
        ><template #icon><Square :size="14" aria-hidden="true" /></template>停止服务</NButton
      >
    </div>
  </section>
</template>

<style scoped>
.runtime-panel.panel {
  background: var(--inspector);
  display: flex;
  flex-direction: column;
}
.service-state {
  display: flex;
  align-items: center;
  gap: 9px;
  padding: 10px;
  background: var(--surface-field);
  border: 1px solid var(--border);
  border-radius: 5px;
}
.service-state.active {
  border-inline-start-color: var(--mint-text);
  color: var(--mint-text);
}
.service-icon {
  display: grid;
  place-items: center;
  width: 34px;
  height: 34px;
  flex: none;
  background: var(--accent-wash);
  border-radius: 8px;
  color: var(--accent);
}
.service-state.active .service-icon {
  background: #ccf0e6;
  color: var(--mint-text);
}
.state-text strong {
  font-size: 13px;
}
.state-text p {
  margin: 2px 0 0;
  font-size: 12px;
  line-height: 1.5;
  color: var(--muted);
}
.runtime-details {
  display: grid;
  gap: 12px;
  margin: 16px 0;
}
.detail-row dt {
  font-size: 12px;
  color: var(--muted);
  margin-bottom: 4px;
}
.detail-row dd {
  margin: 0;
  padding: 7px 9px;
  min-height: 34px;
  border: 1px solid var(--border-soft);
  border-radius: 7px;
  background: var(--surface-field);
}
.detail-row code {
  color: var(--text);
  font-size: 12px;
  overflow-wrap: anywhere;
}
.detail-row .address {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-block: 0;
}
.runtime-actions {
  margin-top: auto;
  padding-top: 6px;
  display: flex;
  flex-wrap: wrap;
  justify-content: flex-end;
  gap: 7px;
}
</style>
