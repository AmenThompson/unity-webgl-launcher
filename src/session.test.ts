import { beforeEach, describe, expect, it, vi } from 'vitest'
import { createPinia, setActivePinia } from 'pinia'
import { desktop } from './desktop'
import { emptySnapshot, normalizePreferences, type CatalogProject, type Snapshot } from './project'
import { useSession } from './session'

vi.mock('./desktop', () => ({
  desktop: {
    available: true,
    start: vi.fn(),
    open: vi.fn(),
    pick: vi.fn(),
    stop: vi.fn(),
    state: vi.fn(),
    demo: vi.fn(),
  },
}))

const active: Snapshot = {
  url: 'http://127.0.0.1:41729/',
  root: 'D:\\构建',
  events: ['服务已启动'],
  format: 'gzip',
  fallback: true,
}

const other: CatalogProject = {
  name: '第二个项目',
  root: 'D:\\合集\\第二个项目',
  entry: 'D:\\合集\\第二个项目\\index.html',
  relativeEntry: '第二个项目/index.html',
  format: 'disabled',
  fallback: false,
  issues: [],
}

beforeEach(() => {
  setActivePinia(createPinia())
  vi.resetAllMocks()
  vi.mocked(desktop.start).mockResolvedValue(active)
  vi.mocked(desktop.stop).mockResolvedValue(emptySnapshot)
  vi.mocked(desktop.state).mockResolvedValue(emptySnapshot)
})

describe('launcher workflow', () => {
  it('keeps editable settings when native selection is cancelled', async () => {
    const session = useSession()
    session.preferences.path = 'D:\\构建'
    vi.mocked(desktop.pick).mockResolvedValue(null)
    await session.pick(true)
    expect(desktop.pick).toHaveBeenCalledWith(true)
    expect(session.preferences.path).toBe('D:\\构建')
  })

  it('starts the selected build and then opens the default browser', async () => {
    const session = useSession()
    session.preferences = { path: ' "D:\\构建" ', threads: true }
    await session.start()
    expect(desktop.start).toHaveBeenCalledWith({ path: 'D:\\构建', threads: true })
    expect(desktop.open).toHaveBeenCalledOnce()
    expect(vi.mocked(desktop.start)).toHaveBeenCalledBefore(vi.mocked(desktop.open))
    expect(session.snapshot).toEqual(active)
    await session.stop()
    expect(session.running).toBe(false)
  })

  it('keeps the live URL available to retry when opening the browser fails', async () => {
    const session = useSession()
    session.preferences.path = 'D:\\构建'
    vi.mocked(desktop.open).mockRejectedValueOnce('浏览器不可用')
    await session.start()
    expect(session.snapshot.url).toEqual(active.url)
    expect(session.notice).toEqual({ label: '服务已启动，浏览器未打开', text: '浏览器不可用' })
    await session.open()
    expect(desktop.open).toHaveBeenCalledTimes(2)
    expect(session.notice).toBeNull()
  })

  it('recovers server diagnostics after a failed service command', async () => {
    const session = useSession()
    session.preferences.path = 'D:\\missing'
    vi.mocked(desktop.start).mockRejectedValueOnce('找不到入口文件')
    vi.mocked(desktop.state).mockResolvedValueOnce({ ...emptySnapshot, events: ['启动失败'] })
    await session.start()
    expect(desktop.open).not.toHaveBeenCalled()
    expect(session.snapshot.events).toEqual(['启动失败'])
    expect(session.notice?.text).toBe('找不到入口文件')
    expect(session.operation).toBeNull()
  })

  it('prevents a second launch while the current launch is pending', async () => {
    const session = useSession()
    session.preferences.path = 'D:\\构建'
    const first = session.start()
    await session.start()
    await session.demo()
    await first
    expect(desktop.start).toHaveBeenCalledOnce()
    expect(desktop.demo).not.toHaveBeenCalled()
  })
})

describe('project catalog', () => {
  it('switches via the new entry without stopping the current service first', async () => {
    const session = useSession()
    vi.mocked(desktop.state).mockResolvedValue(active)
    await session.sync()
    const replacement = { ...active, root: other.root, url: 'http://127.0.0.1:51000/' }
    vi.mocked(desktop.start).mockResolvedValue(replacement)
    await session.launchCatalog(other, 'D:\\合集', false)
    expect(desktop.start).toHaveBeenCalledWith({
      path: other.entry,
      catalogRoot: 'D:\\合集',
      threads: false,
    })
    expect(desktop.stop).not.toHaveBeenCalled()
    expect(session.snapshot).toEqual(replacement)
    expect(desktop.open).toHaveBeenCalledOnce()
  })

  it('keeps the running snapshot when the next project fails preflight', async () => {
    const session = useSession()
    vi.mocked(desktop.state).mockResolvedValue(active)
    vi.mocked(desktop.start).mockRejectedValue('缺少资源')
    await session.sync()
    await session.launchCatalog(other, 'D:\\合集', false)
    expect(session.snapshot).toEqual(active)
    expect(session.notice).toEqual({ label: '切换失败', text: '缺少资源' })
    expect(desktop.stop).not.toHaveBeenCalled()
    expect(desktop.open).not.toHaveBeenCalled()
  })

  it('reopens the running project without changing its port', async () => {
    const session = useSession()
    vi.mocked(desktop.state).mockResolvedValue(active)
    await session.sync()
    await session.launchCatalog({ ...other, root: 'd:/构建/' }, 'D:\\合集', false)
    expect(desktop.start).not.toHaveBeenCalled()
    expect(desktop.open).toHaveBeenCalledOnce()
  })
})

describe('project preferences and live polling', () => {
  it('stores successful starts by canonical root and deduplicates file and folder aliases', async () => {
    const session = useSession()
    session.setPath('D:\\构建\\index.html')
    await session.start()
    await session.stop()
    session.setPath('d:/构建/')
    await session.start()
    expect(session.recentProjects).toEqual([{ root: active.root }])
    session.removeRecent('d:/构建/index.html')
    expect(session.recentProjects).toEqual([])
  })

  it('retains the ten latest successful projects and clears history independently', async () => {
    const session = useSession()
    for (let index = 0; index < 11; index++) {
      const root = `D:\\项目${index}`
      vi.mocked(desktop.start).mockResolvedValueOnce({ ...active, root })
      session.setPath(root)
      await session.start()
      await session.stop()
    }
    expect(session.recentProjects).toHaveLength(10)
    expect(session.recentProjects[0]?.root).toBe('D:\\项目10')
    expect(session.recentProjects.at(-1)?.root).toBe('D:\\项目1')
    session.toggleFavorite('D:\\项目10')
    session.clearRecent()
    expect(session.recentProjects).toEqual([])
    expect(session.isFavorite('d:/项目10/index.html')).toBe(true)
  })

  it('retains project-specific threading and defaults to off for a new path', () => {
    const session = useSession()
    session.setPath('D:\\构建\\index.html')
    session.setProjectThreads(session.preferences.path, true)
    session.setPath(other.root)
    expect(session.preferences.threads).toBe(false)
    session.setPath('d:/构建/')
    expect(session.preferences.threads).toBe(true)
    expect(session.projectThreads(other.entry)).toBe(false)
  })

  it('silently refreshes log events and discards updates begun before a stop', async () => {
    const session = useSession()
    vi.mocked(desktop.state).mockResolvedValueOnce(active)
    await session.sync()
    vi.mocked(desktop.state).mockResolvedValueOnce({ ...active, events: ['GET / 200'] })
    await session.refreshLive()
    expect(session.snapshot.events).toEqual(['GET / 200'])
    let complete: (snapshot: Snapshot) => void = () => undefined
    vi.mocked(desktop.state).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve
        }),
    )
    const pending = session.refreshLive()
    await session.refreshLive()
    await session.stop()
    complete({ ...active, events: ['stale'] })
    await pending
    expect(session.snapshot).toEqual(emptySnapshot)
    expect(session.notice).toBeNull()
  })
})

it('ignores the manual compression preference saved by older versions', () => {
  expect(normalizePreferences({ path: null, mode: 'zip', threads: 1 })).toEqual({
    path: '',
    threads: false,
  })
  expect(normalizePreferences({ path: 'D:\\构建', mode: 'fallback', threads: true })).toEqual({
    path: 'D:\\构建',
    threads: true,
  })
})
