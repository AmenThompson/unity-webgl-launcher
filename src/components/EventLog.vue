<script setup lang="ts">
import { computed, useTemplateRef, watch } from 'vue'
import dayjs from 'dayjs'
import { NAlert, NButton, useMessage } from 'naive-ui'
import { Copy, Terminal } from 'lucide-vue-next'
import { copyText } from '../desktop'
import { errorText, type ResourceDiagnostic } from '../project'

const props = defineProps<{ events: string[]; diagnostics?: ResourceDiagnostic[]; live?: boolean }>()
const message = useMessage()
const output = useTemplateRef<HTMLElement>('output')
const updatedAt = computed(() => (props.events.length ? dayjs().format('HH:mm:ss') : ''))
watch(
  () => props.events,
  () => {
    const el = output.value
    if (el) el.scrollTop = el.scrollHeight
  },
  { flush: 'post' },
)

async function copyEvents() {
  try {
    await copyText(props.events.join('\n'))
    message.success('运行日志已复制')
  } catch (cause) {
    message.error(errorText(cause))
  }
}
</script>

<template>
  <section class="panel event-log" aria-labelledby="event-title">
    <header class="section-header log-header">
      <span class="section-icon"><Terminal aria-hidden="true" /></span>
      <h2 id="event-title">运行日志</h2>
      <span v-if="live" class="live-label">实时更新</span>
      <span v-if="updatedAt" class="updated">更新于 {{ updatedAt }}</span>
      <NButton
        class="copy-log"
        size="small"
        secondary
        :disabled="events.length === 0"
        @click="copyEvents"
        ><template #icon><Copy :size="14" aria-hidden="true" /></template>复制日志</NButton
      >
    </header>
    <NAlert v-if="diagnostics?.length" class="resource-alert" type="warning" title="资源加载提示">
      <ul>
        <li v-for="item in diagnostics" :key="`${item.status}:${item.path}`">
          <code class="mono">{{ item.status }} · {{ item.path }}</code>
          <span> {{ item.message }}</span>
        </li>
      </ul>
    </NAlert>
    <div ref="output" class="log-output mono" tabindex="0" role="log" aria-label="服务事件">
      <p v-if="!events.length" class="empty-line">暂无服务事件。</p>
      <div v-for="(event, index) in events" v-else :key="`${index}:${event}`" class="log-line">
        {{ event }}
      </div>
    </div>
  </section>
</template>

<style scoped>
.log-header {
  margin-bottom: 11px;
  flex-wrap: wrap;
}
.live-label {
  color: var(--mint-text);
  font-size: 12px;
}
.resource-alert {
  margin-bottom: 10px;
  overflow-wrap: anywhere;
}
.resource-alert ul {
  margin: 0;
  padding-left: 18px;
}
.updated {
  margin-left: auto;
  font-size: 12px;
  color: var(--muted);
}
.copy-log {
  margin-left: auto;
}
.updated + .copy-log {
  margin-left: 0;
}
.log-output {
  min-height: 65px;
  max-height: 154px;
  overflow: auto;
  border: 1px solid var(--border-soft);
  border-radius: 4px;
  background: var(--surface-inset);
  padding: 11px 12px;
  color: var(--text);
  font-size: 12px;
  line-height: 1.6;
}
.empty-line {
  color: var(--muted);
  margin: 0;
}
.log-line {
  white-space: pre-wrap;
  overflow-wrap: anywhere;
}
.log-line + .log-line {
  margin-top: 3px;
}
@media (max-width: 390px) {
  .updated {
    display: none;
  }
  .updated + .copy-log {
    margin-left: auto;
  }
}
</style>
