import { computed, shallowRef } from 'vue'
import { defineStore } from 'pinia'
import { useLocalStorage } from '@vueuse/core'
import { desktop } from './desktop'
import { useProjectPreferences } from './projectPreferences'
import {
  defaults,
  emptySnapshot,
  errorText,
  normalizePreferences,
  projectPath,
  sameProjectRoot,
  type CatalogProject,
  type Preferences,
  type Snapshot,
} from './project'

type Operation = 'start' | 'stop' | 'open' | 'demo' | 'pick' | 'sync'
type Notice = { label: string; text: string }

export const useSession = defineStore('session', () => {
  const preferences = useLocalStorage<Preferences>(
    'unity-webgl.launcher.preferences.v1',
    { ...defaults },
    {
      serializer: {
        read: (value) => {
          try {
            return normalizePreferences(JSON.parse(value))
          } catch {
            return { ...defaults }
          }
        },
        write: (value) => JSON.stringify(value),
      },
    },
  )
  const defaultDirectory = useLocalStorage('unity-webgl.launcher.default-directory.v1', '')
  const snapshot = shallowRef<Snapshot>(emptySnapshot)
  const operation = shallowRef<Operation | null>(null)
  const notice = shallowRef<Notice | null>(null)
  const running = computed(() => Boolean(snapshot.value.url))
  const projectPreferences = useProjectPreferences()
  let revision = 0
  let refreshPending = false

  function projectThreads(path: string) {
    return (
      projectPreferences.getThreads(path) ??
      (sameProjectRoot(preferences.value.path, path) ? preferences.value.threads : false)
    )
  }

  function setProjectThreads(path: string, threads: boolean) {
    if (!projectPath(path)) return
    projectPreferences.saveThreads(path, threads)
    if (sameProjectRoot(preferences.value.path, path)) {
      preferences.value = { ...preferences.value, threads }
    }
  }

  function setPath(path: string) {
    const threads = projectPath(path) ? projectThreads(path) : false
    preferences.value = { path, threads }
  }

  async function refreshLive() {
    if (!desktop.available || operation.value || !running.value || refreshPending) return
    const before = snapshot.value
    const startedAt = revision
    refreshPending = true
    try {
      const next = await desktop.state()
      if (
        revision !== startedAt ||
        operation.value ||
        snapshot.value !== before ||
        next.url !== before.url
      )
        return
      if (
        JSON.stringify(next.events) !== JSON.stringify(before.events) ||
        JSON.stringify(next.diagnostics) !== JSON.stringify(before.diagnostics)
      ) {
        snapshot.value = next
      }
    } catch {
      // Live polling must leave the most recent service state and notices intact.
    } finally {
      refreshPending = false
    }
  }

  async function execute(kind: Operation, label: string, work: () => Promise<void>) {
    if (!desktop.available || operation.value) return
    operation.value = kind
    revision += 1
    notice.value = null
    try {
      await work()
    } catch (cause) {
      notice.value = { label, text: errorText(cause) }
      if (kind === 'start' || kind === 'stop' || kind === 'demo') {
        // A failed command can still leave updated service state and diagnostics.
        try {
          snapshot.value = await desktop.state()
        } catch {
          /* Keep the last confirmed snapshot. */
        }
      }
    } finally {
      operation.value = null
    }
  }

  async function startAndOpen(options: Preferences & { catalogRoot?: string }) {
    snapshot.value = await desktop.start(options)
    if (!snapshot.value.url) return
    const root = snapshot.value.root
    if (root) {
      projectPreferences.remember(root)
      setProjectThreads(options.path, options.threads)
      setProjectThreads(root, options.threads)
    }
    try {
      await desktop.open()
    } catch (cause) {
      notice.value = { label: '服务已启动，浏览器未打开', text: errorText(cause) }
    }
  }

  function start() {
    const path = projectPath(preferences.value.path)
    if (!path || running.value) return
    return execute('start', '启动失败', () => startAndOpen({ ...preferences.value, path }))
  }

  async function launchCatalog(project: CatalogProject, catalogRoot: string, threads: boolean) {
    if (!catalogRoot) return
    if (running.value && sameProjectRoot(snapshot.value.root, project.root)) {
      await open()
      return
    }
    if (project.issues.length) return
    await execute('start', running.value ? '切换失败' : '启动失败', () =>
      startAndOpen({ path: project.entry, catalogRoot, threads }),
    )
  }

  function launchSaved(root: string) {
    const path = projectPath(root)
    if (!path) return
    if (running.value && sameProjectRoot(snapshot.value.root, path)) return open()
    const entry = `${path.replace(/[\\/]+$/, '')}\\index.html`
    return execute('start', running.value ? '切换失败' : '启动失败', () =>
      startAndOpen({ path: entry, catalogRoot: path, threads: projectThreads(path) }),
    )
  }

  function pick(directory: boolean) {
    if (running.value) return
    return execute('pick', '无法打开选择窗口', async () => {
      const selected = await desktop.pick(directory)
      if (selected) setPath(selected)
    })
  }

  function open() {
    if (running.value) return execute('open', '无法打开浏览器', desktop.open)
  }

  function stop() {
    if (running.value)
      return execute('stop', '停止失败', async () => {
        snapshot.value = await desktop.stop()
      })
  }

  function demo() {
    if (!running.value)
      return execute('demo', '诊断启动失败', async () => {
        snapshot.value = await desktop.demo()
      })
  }

  function sync() {
    return execute('sync', '无法读取服务状态', async () => {
      snapshot.value = await desktop.state()
    })
  }

  function dismissNotice() {
    notice.value = null
  }

  return {
    preferences,
    defaultDirectory,
    recentProjects: projectPreferences.recent,
    favoritePaths: projectPreferences.favorites,
    launchSaved,
    removeRecent: projectPreferences.removeRecent,
    clearRecent: projectPreferences.clearRecent,
    isFavorite: projectPreferences.isFavorite,
    toggleFavorite: projectPreferences.toggleFavorite,
    projectThreads,
    setProjectThreads,
    setPath,
    refreshLive,
    snapshot,
    operation,
    notice,
    running,
    start,
    launchCatalog,
    pick,
    open,
    stop,
    demo,
    sync,
    dismissNotice,
  }
})
