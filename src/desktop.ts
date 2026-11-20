import { invoke, isTauri } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import type { CatalogScan, Preferences, Snapshot } from './project'

export const desktop = {
  available: isTauri(),
  state: () => invoke<Snapshot>('project_state'),
  start: (options: Preferences & { catalogRoot?: string }) =>
    invoke<Snapshot>('start_project', { ...options, mode: 'auto' }),
  scan: (path: string) => invoke<CatalogScan>('scan_projects', { path }),
  checkRecent: (paths: string[]) => invoke<boolean[]>('check_recent_projects', { paths }),
  pickCatalog: () =>
    open({ directory: true, multiple: false, title: '选择包含多个 Unity WebGL 发布的总目录' }),
  stop: () => invoke<Snapshot>('stop_project'),
  open: () => invoke<void>('open_project'),
  demo: () => invoke<Snapshot>('open_demo'),
  pick: (directory: boolean) =>
    open({
      directory,
      multiple: false,
      title: directory ? '选择包含 index.html 的项目文件夹' : '选择 Unity WebGL 入口文件',
      filters: directory ? undefined : [{ name: '网页入口', extensions: ['html', 'htm'] }],
    }),
}

export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text)
    return
  } catch {
    const active = document.activeElement
    const field = document.createElement('textarea')
    field.value = text
    field.readOnly = true
    field.className = 'clipboard-capture'
    document.body.append(field)
    field.select()
    try {
      if (!document.execCommand('copy')) throw new Error('复制失败，请手动选择并复制。')
    } finally {
      field.remove()
      if (active instanceof HTMLElement) active.focus()
    }
  }
}
