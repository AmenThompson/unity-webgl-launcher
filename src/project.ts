export type BuildFormat = 'brotli' | 'gzip' | 'disabled' | 'mixed' | 'unknown'
export type Preferences = { path: string; threads: boolean }
export type CatalogProject = {
  name: string
  root: string
  entry: string
  relativeEntry: string
  format: BuildFormat
  fallback: boolean | null
  issues: string[]
}
export type CatalogScan = {
  root: string
  projects: CatalogProject[]
  truncated: boolean
  skipped: number
}

export function projectKey(value: string): string {
  return projectPath(value)
    .replaceAll('/', '\\')
    .replace(/\\+$/, '')
    .replace(/\\index\.html$/i, '')
    .toLowerCase()
}

export function sameProjectRoot(left: string | null, right: string): boolean {
  return left !== null && projectKey(left) === projectKey(right)
}
export type ResourceDiagnostic = { status: number; path: string; message: string }
export type Snapshot = {
  diagnostics?: ResourceDiagnostic[]
  url: string | null
  root: string | null
  events: string[]
  format: BuildFormat | null
  fallback: boolean | null
}

export const defaults: Preferences = { path: '', threads: false }
export const emptySnapshot: Snapshot = {
  url: null,
  root: null,
  events: [],
  format: null,
  fallback: null,
}

export const formatLabels: Record<BuildFormat, string> = {
  brotli: 'Brotli',
  gzip: 'Gzip',
  disabled: 'Disabled（未压缩）',
  mixed: '混合格式',
  unknown: '未识别',
}

export function normalizePreferences(raw: unknown): Preferences {
  if (!raw || typeof raw !== 'object') return { ...defaults }
  const data = raw as Partial<Preferences>
  return {
    path: typeof data.path === 'string' ? data.path : defaults.path,
    threads: typeof data.threads === 'boolean' ? data.threads : defaults.threads,
  }
}

export function projectPath(value: string): string {
  return value.trim().replace(/^"(.*)"$/, '$1')
}

export function errorText(cause: unknown): string {
  if (cause instanceof Error) return cause.message
  if (typeof cause === 'string') return cause
  return '操作失败，请重试。'
}
