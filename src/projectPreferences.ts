import { useLocalStorage } from '@vueuse/core'
import { projectKey } from './project'

export const recentLimit = 10
const settingsLimit = 256

export type RecentProject = { root: string }
type ThreadPreference = { root: string; enabled: boolean }

export function useProjectPreferences() {
  const recent = useLocalStorage<RecentProject[]>('unity-webgl.launcher.recent.v1', [])
  const threads = useLocalStorage<ThreadPreference[]>('unity-webgl.launcher.threads.v1', [])
  const favorites = useLocalStorage<string[]>('unity-webgl.launcher.favorites.v1', [])

  function remember(root: string) {
    const key = projectKey(root)
    if (key) {
      recent.value = [
        { root },
        ...recent.value.filter((project) => projectKey(project.root) !== key),
      ].slice(0, recentLimit)
    }
  }

  function removeRecent(root: string) {
    const key = projectKey(root)
    recent.value = recent.value.filter((project) => projectKey(project.root) !== key)
  }

  function clearRecent() {
    recent.value = []
  }

  function getThreads(root: string): boolean | undefined {
    const key = projectKey(root)
    return threads.value.find((item) => item.root === key)?.enabled
  }

  function saveThreads(root: string, enabled: boolean) {
    const key = projectKey(root)
    if (!key) return
    threads.value = [{ root: key, enabled }, ...threads.value.filter((item) => item.root !== key)].slice(
      0,
      settingsLimit,
    )
  }

  function isFavorite(root: string): boolean {
    return favorites.value.some((path) => projectKey(path) === projectKey(root))
  }

  function toggleFavorite(root: string) {
    const key = projectKey(root)
    if (!key) return
    favorites.value = isFavorite(root)
      ? favorites.value.filter((item) => projectKey(item) !== key)
      : [root, ...favorites.value]
  }

  return {
    recent,
    favorites,
    remember,
    removeRecent,
    clearRecent,
    getThreads,
    saveThreads,
    isFavorite,
    toggleFavorite,
  }
}
