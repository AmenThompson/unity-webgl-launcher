import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import { mkdir, writeFile } from 'node:fs/promises'
import { createServer } from 'node:net'
import { resolve } from 'node:path'
import { setTimeout as pause } from 'node:timers/promises'
import { chromium } from 'playwright-core'
import { verifyCompression } from './verify-compression.mjs'
import { verifyCatalog } from './verify-catalog.mjs'
import { verifyProjectWorkflow } from './verify-project-workflow.mjs'
import { verifyFrequent } from './verify-frequent.mjs'

const [exeArg, unityArg, reportArg] = process.argv.slice(2)
assert.ok(
  exeArg && unityArg && reportArg,
  'Usage: node tools/verify-webgl.mjs <exe> <unity-dir> <report-dir>',
)
const report = resolve(reportArg)
await mkdir(report, { recursive: true })

const reservation = createServer().listen(0, '127.0.0.1')
await once(reservation, 'listening')
const { port } = reservation.address()
await new Promise((finish, fail) => reservation.close((err) => (err ? fail(err) : finish())))
const endpoint = `http://127.0.0.1:${port}`
const app = spawn(resolve(exeArg), [], {
  cwd: process.env.SystemRoot,
  env: {
    ...process.env,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port}`,
  },
  stdio: 'ignore',
})

async function connectWebView() {
  for (let i = 0; i < 120; i++) {
    if (app.exitCode !== null) throw new Error(`应用提前退出 (${app.exitCode})`)
    try {
      const response = await fetch(`${endpoint}/json/list`)
      if (response.ok && (await response.json()).some((page) => /tauri\.localhost/.test(page.url))) {
        return chromium.connectOverCDP(endpoint)
      }
    } catch {
      // WebView2 has not opened the debug port yet.
    }
    await pause(250)
  }
  throw new Error('WebView2 调试连接超时')
}

const outcome = {}
let desktop
let browser
try {
  desktop = await connectWebView()
  let webview
  for (let attempt = 0; attempt < 60 && !webview; attempt++) {
    webview = desktop
      .contexts()
      .flatMap((context) => context.pages())
      .find((page) => /tauri\.localhost/.test(page.url()))
    if (!webview) await pause(250)
  }
  assert.ok(webview, '应用 WebView 页面存在')
  await webview.getByRole('button', { name: /启动并打开|启动项目/ }).waitFor()
  if (process.argv.includes('--workspace-theme')) {
    const shell = await webview.evaluate(() => {
      const sidebar = document.querySelector('.topbar')
      const main = document.querySelector('.workspace')
      return {
        sideBySide: sidebar.getBoundingClientRect().right <= main.getBoundingClientRect().left + 1,
        sidebar: getComputedStyle(sidebar).backgroundColor,
        page: getComputedStyle(document.body).backgroundColor,
        primary: getComputedStyle(document.documentElement).getPropertyValue('--primary').trim(),
      }
    })
    assert.deepEqual(shell, {
      sideBySide: true,
      sidebar: 'rgb(250, 250, 253)',
      page: 'rgb(255, 255, 255)',
      primary: '#5652d9',
    })
    outcome.workspaceTheme = shell
  }
  if (process.argv.includes('--compression-matrix')) {
    const visual = await webview.evaluate(() => ({
      colorScheme: getComputedStyle(document.documentElement).colorScheme,
      background: getComputedStyle(document.body).backgroundImage,
    }))
    assert.equal(visual.colorScheme, 'light')
    assert.match(visual.background, /linear-gradient/)
    outcome.lightGradientTheme = true
  }
  if (process.argv.includes('--brand-icon')) {
    await webview.locator('.brand-mark').evaluate((image) => image.decode())
    const icon = await webview.evaluate(async () => {
      const brand = document.querySelector('.brand-mark')
      const favicon = document.querySelector('link[rel="icon"]')
      if (!brand?.naturalWidth || !favicon?.href) return null
      const response = await fetch(favicon.href)
      return {
        sameAsset: brand.currentSrc === favicon.href,
        loaded: response.ok && (await response.text()).includes('开放式六边形'),
      }
    })
    assert.deepEqual(icon, { sameAsset: true, loaded: true })
    outcome.brandIcon = icon
  }
  if (process.argv.includes('--compact-ui')) {
    await webview.getByRole('heading', { name: '本地项目预览' }).waitFor()
    assert.equal(await webview.locator('.brand').innerText(), '')
    assert.equal(await webview.getByText('服务未启动', { exact: true }).count(), 1)
    assert.equal(await webview.getByRole('button', { name: '停止服务' }).count(), 0)
    assert.equal(await webview.getByRole('heading', { name: '运行日志' }).count(), 0)
    outcome.compactUi = true
  }
  if (process.argv.includes('--picker-layout')) {
    await webview.getByText('自动识别构建格式', { exact: true }).waitFor()
    await webview
      .getByText('支持 Brotli、Gzip、未压缩及 Unity 内置解压回退。', { exact: true })
      .waitFor()
    const input = await webview.locator('.path-input').boundingBox()
    const folder = await webview.getByRole('button', { name: '选择文件夹' }).boundingBox()
    const entry = await webview.getByRole('button', { name: '选择入口文件' }).boundingBox()
    assert.ok(input && folder && entry)
    assert.ok(folder.y > input.y + input.height && entry.y > input.y + input.height)
    assert.ok(entry.x > folder.x + folder.width && Math.abs(folder.y - entry.y) <= 2)
    assert.ok(Math.abs(entry.x + entry.width - (input.x + input.width)) <= 2)
    outcome.pickerLayout = true
  }
  if (process.argv.includes('--about-notes')) {
    assert.ok(process.env.WEBVIEW2_USER_DATA_FOLDER, 'Use an isolated WebView2 profile for this test')
    const version = /_(\d+\.\d+\.\d+)_x64\.exe$/i.exec(exeArg)?.[1]
    assert.ok(version)
    await webview.evaluate(() =>
      localStorage.setItem('unity-webgl.launcher.notes.v1', '**旧版测试笔记**'),
    )
    await webview.getByRole('link', { name: '本地笔记' }).click()
    await webview.getByRole('heading', { name: '本地笔记' }).waitFor()
    await webview.getByText('旧版测试笔记', { exact: true }).waitFor()
    await webview.getByRole('button', { name: '编辑笔记' }).click()
    const editor = webview.locator('.notes-editor .cm-content[contenteditable="true"]')
    await editor.click()
    await webview.keyboard.press('Control+a')
    await webview.keyboard.insertText('# 保存检查')
    await webview.getByRole('button', { name: '完成编辑' }).click()
    await webview.getByRole('link', { name: '关于软件' }).click()
    await webview.getByRole('heading', { name: '关于软件' }).waitFor()
    assert.equal(await webview.getByTestId('release-version').innerText(), version)
    await webview.screenshot({ path: resolve(report, 'about.png') })
    await webview.getByRole('link', { name: '使用指南' }).click()
    await webview.getByRole('heading', { name: '使用指南' }).waitFor()
    assert.equal(await webview.getByRole('heading', { name: '本地笔记' }).count(), 0)
    await webview.getByRole('link', { name: '本地笔记' }).click()
    await webview.getByRole('heading', { name: '保存检查' }).waitFor()
    await webview.reload()
    await webview.getByRole('heading', { name: '保存检查' }).waitFor()
    await webview.screenshot({ path: resolve(report, 'notes.png') })
    await webview.getByRole('link', { name: '单项目' }).click()
    outcome.aboutNotes = { releaseVersion: version, legacyNoteLoaded: true, savedAfterReload: true }
  }
  await webview.screenshot({ path: resolve(report, 'desktop.png') })
  const invoke = (command, args) =>
    webview.evaluate(
      ([name, params]) => window.__TAURI_INTERNALS__.invoke(name, params),
      [command, args],
    )

  browser = await chromium.launch({ channel: 'msedge', headless: false, args: ['--no-first-run'] })
  const preview = await browser.newPage({ viewport: { width: 1200, height: 850 } })
  preview.on('dialog', (event) => {
    outcome.unexpectedDialog = event.message()
    void event.dismiss()
  })

  const currentUrl = () => invoke('project_state').then((state) => state.url)
  const start = webview.getByRole('button', { name: '启动并打开' })
  const stop = webview.getByRole('button', { name: '停止服务' })
  const pathField = webview.getByRole('textbox', { name: 'Unity WebGL 路径' })
  await pathField.fill(`${resolve(unityArg)}\\__missing__`)
  await start.click()
  await webview.getByText(/无法访问所选路径/).waitFor()
  outcome.invalidPathFeedback = true
  await webview.getByRole('button', { name: '运行内置诊断' }).click()
  await webview.getByRole('button', { name: '复制预览地址' }).waitFor()
  const demoUrl = await currentUrl()
  assert.ok(demoUrl)
  await preview.goto(demoUrl)
  const scene = preview.locator('#scene')
  const status = preview.getByRole('status')
  await preview.getByText(/WebGL 已渲染/).waitFor()
  assert.match(await status.textContent(), /跨源隔离已启用/)
  const startAngle = await status.textContent()
  await scene.focus()
  await preview.keyboard.press('ArrowRight')
  assert.notEqual(await status.textContent(), startAngle)
  const keyAngle = await status.textContent()
  await preview.bringToFront()
  const box = await scene.boundingBox()
  assert.ok(box)
  await preview.mouse.move(box.x + box.width / 2, box.y + box.height / 2)
  await preview.mouse.down()
  await preview.mouse.move(box.x + box.width / 2 + 90, box.y + box.height / 2, { steps: 8 })
  await preview.mouse.up()
  assert.notEqual(await status.textContent(), keyAngle)
  await preview.screenshot({ path: resolve(report, 'webgl-demo.png') })
  await preview.bringToFront()
  await preview.getByRole('button', { name: '全屏' }).click()
  await preview.waitForFunction(
    () =>
      document.fullscreenElement?.id === 'scene' ||
      document.querySelector('#status')?.textContent.includes('无法全屏'),
    null,
    { timeout: 5000 },
  )
  assert.equal(
    await preview.evaluate(() => document.fullscreenElement?.id),
    'scene',
    await status.textContent(),
  )
  await preview.evaluate(() => document.exitFullscreen())
  outcome.demo = {
    keyboard: true,
    pointer: true,
    fullscreen: true,
    crossOriginIsolated: true,
    rendered: true,
  }
  await stop.click()
  await webview.getByRole('button', { name: '复制预览地址' }).waitFor({ state: 'hidden' })
  assert.equal(await currentUrl(), null)
  await assert.rejects(fetch(demoUrl))

  await pathField.fill(resolve(unityArg, 'index.html'))
  await start.click()
  await webview.getByRole('button', { name: '复制预览地址' }).waitFor()
  const unityState = await invoke('project_state')
  const unityUrl = unityState.url
  assert.ok(unityUrl)
  if (process.argv.includes('--button-layout')) {
    const group = webview.locator('.runtime-actions')
    const open = group.getByRole('button', { name: '打开浏览器', exact: true })
    const clear = webview.getByRole('button', { name: '清空历史', exact: true })
    await clear.locator('svg.lucide-trash-2').waitFor()
    const layout = await group.boundingBox()
    const first = await open.boundingBox()
    const second = await stop.boundingBox()
    assert.ok(layout && first && second)
    assert.ok(Math.abs(first.y - second.y) < 2 && first.x < second.x)
    assert.ok(Math.abs(layout.x + layout.width - second.x - second.width) < 2)
    await webview.screenshot({ path: resolve(report, 'button-layout.png') })
    outcome.buttonLayout = true
  }
  if (process.argv.includes('--compression-matrix')) {
    assert.equal(unityState.format, 'disabled')
    assert.equal(unityState.fallback, false)
    await webview.getByText('Disabled（未压缩）', { exact: true }).waitFor()
  }
  const unityResponses = new Map()
  preview.on('response', (response) => {
    if (response.url().startsWith(unityUrl)) unityResponses.set(response.url(), response.status())
  })
  await preview.goto(unityUrl)
  const unityCanvas = preview.locator('#unity-canvas')
  await unityCanvas.waitFor({ state: 'visible' })
  await preview.waitForFunction(
    () =>
      document.querySelector('#loading-cover')?.style.display === 'none' &&
      document.querySelector('#unity-fullscreen-button')?.style.display !== 'none',
    null,
    { timeout: 120_000 },
  )
  assert.equal(outcome.unexpectedDialog, undefined)
  for (const ext of ['.loader.js', '.framework.js', '.data', '.wasm']) {
    assert.ok(
      [...unityResponses].some(([url, status]) => url.endsWith(ext) && status === 200),
      `加载实际 Unity ${ext}`,
    )
  }
  await preview.screenshot({ path: resolve(report, 'unity-scene.png') })
  const unityFullscreen = preview.locator('#unity-fullscreen-button')
  await preview.bringToFront()
  await unityFullscreen.click()
  await preview.waitForFunction(() => !!document.fullscreenElement, null, { timeout: 8000 })
  await preview.evaluate(() => document.exitFullscreen())
  await unityCanvas.click({ position: { x: 300, y: 270 } })
  await preview.keyboard.press('ArrowLeft')
  outcome.unity = {
    renderedAndLoaderResolved: true,
    fullscreen: true,
    resourceResponses: [...unityResponses].map(([url, status]) => [url.replace(unityUrl, ''), status]),
  }
  await stop.click()
  await webview.getByRole('button', { name: '复制预览地址' }).waitFor({ state: 'hidden' })
  assert.equal(await currentUrl(), null)
  await assert.rejects(fetch(unityUrl))
  outcome.stopClosesListener = true
  if (process.argv.includes('--frequent')) {
    outcome.frequent = await verifyFrequent({
      webview,
      preview,
      invoke,
      source: resolve(unityArg),
      report,
    })
  }
  if (process.argv.includes('--compression-matrix')) {
    outcome.compression = await verifyCompression({ invoke, preview, source: resolve(unityArg), report })
  }
  if (process.argv.includes('--project-workflow')) {
    outcome.projectWorkflow = await verifyProjectWorkflow({
      webview,
      invoke,
      source: resolve(unityArg),
      report,
    })
  }
  if (process.argv.includes('--catalog')) {
    outcome.catalog = await verifyCatalog({
      webview,
      preview,
      invoke,
      directory: resolve(unityArg, '..'),
      screenshot: resolve(report, 'project-catalog.png'),
    })
  }
  if (process.argv.includes('--log-page')) {
    const navigation = webview.getByRole('navigation', { name: '主导航' })
    const navLabels = (await navigation.getByRole('link').allTextContents()).map((value) => value.trim())
    assert.equal(navLabels[navLabels.indexOf('常用项目') + 1], '运行日志')
    const checkGap = async (panel) => {
      await panel.waitFor()
      const content = await panel.boundingBox()
      const footer = await webview.locator('.app-footer').boundingBox()
      assert.ok(content && footer && Math.abs(footer.y - content.y - content.height - 14) < 2)
    }
    await navigation.getByRole('link', { name: '多项目' }).click()
    await webview.getByRole('heading', { name: '多项目' }).waitFor()
    await checkGap(webview.locator('.catalog-grid'))
    assert.equal(await webview.getByText('切换成功后，上一项目的预览地址将失效。').count(), 0)
    const threads = webview.getByRole('switch', { name: '多线程支持' })
    await threads.waitFor()
    const first = webview.getByRole('button', { name: /^vmc580\s+vmc580\/index\.html/ })
    await first.click()
    await webview.getByRole('button', { name: '启动并打开' }).click()
    await webview.getByText('当前运行 vmc580').waitFor()
    assert.ok(await threads.isVisible())
    assert.match(await threads.getAttribute('class'), /n-switch--disabled/)
    await checkGap(webview.locator('.catalog-grid'))
    const catalogWidths = await webview.locator('.catalog-grid > .panel').evaluateAll(
      (panels) => panels.map((panel) => panel.getBoundingClientRect().width),
    )
    await webview.screenshot({ path: resolve(report, 'catalog-layout.png') })
    await navigation.getByRole('link', { name: '常用项目' }).click()
    await checkGap(webview.locator('.layout'))
    if (process.argv.includes('--column-layout')) {
      const frequentWidths = await webview.locator('.layout > .panel').evaluateAll(
        (panels) => panels.map((panel) => panel.getBoundingClientRect().width),
      )
      assert.equal(catalogWidths.length, 2)
      assert.equal(frequentWidths.length, 2)
      for (const [index, width] of catalogWidths.entries()) {
        assert.ok(Math.abs(width - frequentWidths[index]) < 1)
      }
      assert.ok(Math.abs(catalogWidths[0] / catalogWidths[1] - 1.2) < 0.02)
      outcome.projectColumnWidths = { catalog: catalogWidths, frequent: frequentWidths }
    }
    assert.equal(await webview.getByRole('log', { name: '服务事件' }).count(), 0)
    await webview.screenshot({ path: resolve(report, 'frequent-layout.png') })
    await navigation.getByRole('link', { name: '运行日志' }).click()
    await webview.getByRole('log', { name: '服务事件' }).getByText(/本地服务已启动/).waitFor()
    const logPanel = webview.locator('.page-panel')
    await checkGap(logPanel)
    if (process.argv.includes('--column-layout')) {
      const panel = await logPanel.boundingBox()
      const output = await logPanel.locator('.log-output').boundingBox()
      const footer = await webview.locator('.app-footer').boundingBox()
      assert.ok(panel && output && footer)
      const innerGap = panel.y + panel.height - output.y - output.height
      const outerGap = footer.y - panel.y - panel.height
      assert.ok(Math.abs(innerGap - outerGap) < 1)
      outcome.logBottomGaps = { inner: innerGap, outer: outerGap }
    }
    await webview.screenshot({ path: resolve(report, 'logs-layout.png') })
    outcome.logPage = true
  }
  if (process.argv.includes('--panel-heights')) {
    const footer = webview.locator('.app-footer')
    const checkGap = async (panel) => {
      await panel.waitFor()
      const content = await panel.boundingBox()
      const bottom = await footer.boundingBox()
      assert.ok(content && bottom)
      assert.ok(Math.abs(bottom.y - content.y - content.height - 14) < 2)
    }
    await webview.getByRole('link', { name: '常用项目', exact: true }).click()
    await checkGap(webview.locator('.saved-log'))
    const panels = await webview.locator('.layout > .panel').all()
    assert.equal(panels.length, 2)
    const [left, right] = await Promise.all(panels.map((panel) => panel.boundingBox()))
    assert.ok(left && right && left.height >= 323 && Math.abs(left.height - right.height) < 2)
    await webview.screenshot({ path: resolve(report, 'frequent-layout.png') })
    for (const label of ['使用指南', '本地笔记', '关于软件']) {
      await webview.getByRole('link', { name: label, exact: true }).click()
      await webview.getByRole('heading', { name: label, exact: true }).waitFor()
      await checkGap(webview.locator('.page-panel'))
    }
    await webview.screenshot({ path: resolve(report, 'about-layout.png') })
    outcome.panelHeights = true
  }
  if (process.argv.includes('--separate-project-pages')) {
    const nav = webview.getByRole('navigation', { name: '主导航' })
    await nav.getByRole('link', { name: '单项目', exact: true }).click()
    const single = webview.locator('.dashboard > .panel')
    await single.first().waitFor()
    const singleTops = await single.evaluateAll((panels) => panels.map((panel) => panel.getBoundingClientRect().top))
    assert.equal(singleTops.length, 2)
    assert.equal(await webview.getByRole('heading', { name: '最近项目' }).count(), 0)
    await webview.screenshot({ path: resolve(report, 'single-layout.png') })
    await nav.getByRole('link', { name: '多项目', exact: true }).click()
    const multi = webview.locator('.catalog-grid > .panel')
    await multi.first().waitFor()
    const multiTops = await multi.evaluateAll((panels) => panels.map((panel) => panel.getBoundingClientRect().top))
    assert.equal(multiTops.length, 2)
    assert.ok(singleTops.every((top, index) => Math.abs(top - multiTops[index]) < 1))
    assert.equal(await webview.locator('.active-status').count(), 0)
    outcome.separateProjectPages = { singleTops, multiTops }
  }
  if (process.argv.includes('--recent-layout')) {
    const nav = webview.getByRole('navigation', { name: '主导航' })
    assert.equal(await nav.getByRole('link', { name: '本地笔记', exact: true }).count(), 0)
    const footer = webview.locator('.app-footer')
    for (const [name, selector] of [['单项目', '.dashboard'], ['多项目', '.catalog-grid']]) {
      await nav.getByRole('link', { name, exact: true }).click()
      const panels = webview.locator(`${selector} > .panel`)
      await panels.first().waitFor()
      const footerTop = (await footer.boundingBox()).y
      const gaps = await panels.evaluateAll((items, y) => items.map((item) => y - item.getBoundingClientRect().bottom), footerTop)
      assert.equal(gaps.length, 2)
      assert.ok(gaps.every((gap) => Math.abs(gap - 14) < 1))
    }
    await nav.getByRole('link', { name: '最近项目', exact: true }).click()
    const header = webview.locator('.saved-page-header')
    const title = await header.getByRole('heading', { name: '最近项目' }).boundingBox()
    const clear = header.getByRole('button', { name: '清空历史' })
    const button = await clear.boundingBox()
    const headerBox = await header.boundingBox()
    assert.ok(title && button && headerBox)
    assert.ok(button.x > title.x + title.width)
    assert.ok(Math.abs(button.x + button.width - headerBox.x - headerBox.width) < 1)
    assert.ok(Math.abs(button.y + button.height / 2 - title.y - title.height / 2) < 1)
    await webview.screenshot({ path: resolve(report, 'recent-layout.png') })
    await clear.click()
    await webview.getByText('成功启动的项目会出现在这里。').waitFor()
    outcome.recentLayout = true
  }
  if (process.argv.includes('--default-directory')) {
    assert.ok(process.env.WEBVIEW2_USER_DATA_FOLDER, 'Use an isolated profile for this test')
    const nav = webview.getByRole('navigation', { name: '主导航' })
    const links = (await nav.getByRole('link').allTextContents()).map((item) => item.trim())
    assert.equal(links[links.indexOf('运行日志') + 1], '设置')
    await nav.getByRole('link', { name: '使用指南' }).click()
    await webview.getByRole('heading', { name: '使用指南' }).waitFor()
    const guideSize = await webview.locator('.page-panel').boundingBox()
    await nav.getByRole('link', { name: '设置' }).click()
    const settings = webview.getByRole('textbox', { name: '默认路径' })
    await settings.waitFor()
    const settingsSize = await webview.locator('.page-panel').boundingBox()
    assert.ok(guideSize && settingsSize)
    assert.ok(Math.abs(guideSize.width - settingsSize.width) < 1 && Math.abs(guideSize.height - settingsSize.height) < 1)
    const root = resolve(unityArg, '..')
    await settings.fill(resolve(root, '__missing_default_dir__'))
    await webview.getByRole('button', { name: '保存设置' }).click()
    await webview.getByRole('alert').getByText(/无法读取发布总目录/).waitFor()
    await settings.fill(root)
    await webview.getByRole('button', { name: '保存设置' }).click()
    await webview.getByText('默认路径已保存。').waitFor()
    await webview.screenshot({ path: resolve(report, 'settings.png') })
    await webview.reload()
    await settings.waitFor()
    assert.equal((await settings.inputValue()).toLowerCase(), root.toLowerCase())
    await nav.getByRole('link', { name: '多项目' }).click()
    await webview.locator('.project-row').filter({ hasText: 'vmc850' }).first().waitFor()
    assert.equal((await webview.getByRole('textbox', { name: '发布总目录' }).inputValue()).toLowerCase(), root.toLowerCase())
    await nav.getByRole('link', { name: '单项目' }).click()
    await webview.waitForFunction((expected) => document.querySelector('#project-path')?.value.toLowerCase() === expected.toLowerCase(), resolve(unityArg, 'index.html'))
    assert.equal((await invoke('project_state')).url, null)
    await nav.getByRole('link', { name: '设置' }).click()
    await settings.fill('')
    await webview.getByRole('button', { name: '保存设置' }).click()
    await webview.getByText('默认路径已清除。').waitFor()
    outcome.defaultDirectory = true
  }
} finally {
  await writeFile(resolve(report, 'webgl-results.json'), `${JSON.stringify(outcome, null, 2)}\n`)
  await browser?.close()
  if (app.exitCode === null && app.pid) app.kill()
  await desktop?.close().catch(() => {})
}
console.log(JSON.stringify(outcome, null, 2))
