<script setup lang="ts">
import { onBeforeUnmount, onMounted, shallowRef } from 'vue'
import { NAlert, NButton } from 'naive-ui'
import { FlaskConical } from 'lucide-vue-next'
import ProjectForm from '../components/ProjectForm.vue'
import RuntimePanel from '../components/RuntimePanel.vue'
import { desktop } from '../desktop'
import { errorText, sameProjectRoot } from '../project'
import { useSession } from '../session'

const session = useSession()
const scanMessage = shallowRef('')
let mounted = true
onBeforeUnmount(() => {
  mounted = false
})
onMounted(async () => {
  const directory = session.defaultDirectory
  if (!desktop.available || !directory || session.running) return
  const initialPath = session.preferences.path
  const canApply = () =>
    mounted &&
    !session.running &&
    (!session.operation || session.operation === 'sync') &&
    session.preferences.path === initialPath
  try {
    const result = await desktop.scan(directory)
    if (!canApply()) return
    const usable = result.projects.filter((project) => !project.issues.length)
    const project = usable.find((candidate) => sameProjectRoot(candidate.root, result.root)) ?? usable[0]
    if (project) session.setPath(project.entry)
    else scanMessage.value = '默认路径下没有可用的 Unity WebGL 项目。'
  } catch (cause) {
    if (canApply()) scanMessage.value = `默认路径扫描失败：${errorText(cause)}`
  }
})
</script>

<template>
  <section class="launcher-intro" aria-labelledby="launcher-heading">
    <h1 id="launcher-heading">本地项目预览</h1>
    <NButton
      secondary
      :disabled="!desktop.available || session.running || session.operation !== null"
      :loading="session.operation === 'demo'"
      @click="session.demo()"
      ><template #icon><FlaskConical :size="16" aria-hidden="true" /></template>运行内置诊断</NButton
    >
  </section>
  <NAlert v-if="!desktop.available" class="banner" type="info" title="当前为网页预览" role="status"
    >文件选择和本地服务需要在 Windows 桌面应用中使用。</NAlert
  >
  <NAlert
    v-if="session.notice"
    class="banner"
    type="warning"
    :title="session.notice.label"
    closable
    role="alert"
    @close="session.dismissNotice()"
    >{{ session.notice.text }}</NAlert
  >
  <NAlert v-if="scanMessage" class="banner" type="warning">{{ scanMessage }}</NAlert>
  <div class="dashboard">
    <ProjectForm
      :prefs="session.preferences"
      :starting="session.operation === 'start'"
      :disabled="!desktop.available || session.operation !== null || session.running"
      :running="session.running"
      @path="session.setPath($event)"
      @threads="session.setProjectThreads(session.preferences.path, $event)"
      @pick="session.pick($event)"
      @start="session.start()"
    />
    <RuntimePanel
      :snapshot="session.snapshot"
      :operation="session.operation"
      @open="session.open()"
      @stop="session.stop()"
    />
  </div>
</template>

<style scoped>
.launcher-intro {
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 10px 18px;
  min-height: var(--project-heading-height);
  margin-bottom: 14px;
}
.launcher-intro h1 {
  margin: 0;
  font-size: clamp(20px, 3vw, 24px);
  line-height: 1.35;
  font-weight: 700;
}
.banner {
  margin-bottom: 17px;
  overflow-wrap: anywhere;
}
.dashboard {
  display: grid;
  grid-template-columns: minmax(0, 1.45fr) minmax(0, 1fr);
  gap: 17px;
  flex: 1 0 auto;
}
@media (max-width: 770px) {
  .dashboard {
    grid-template-columns: minmax(0, 1fr);
  }
}
</style>
