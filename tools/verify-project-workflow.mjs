import assert from 'node:assert/strict'
import { dirname, join } from 'node:path'

export async function verifyProjectWorkflow({ webview, invoke, source, report }) {
  assert.ok(process.env.WEBVIEW2_USER_DATA_FOLDER, 'Use an isolated WebView2 profile for this test')
  const link = (name) => webview.getByRole('link', { name, exact: true })
  const button = (name) => webview.getByRole('button', { name, exact: true })
  await link('单项目').click()
  await webview.getByRole('heading', { name: '最近项目' }).waitFor()
  const path = webview.getByRole('textbox', { name: 'Unity WebGL 路径' })
  const recent = webview.getByRole('button', { name: /^选择最近项目 .*vmc580$/i })
  await recent.click()
  assert.equal((await path.inputValue()).toLowerCase(), source.toLowerCase())
  await button('启动并打开').click()
  await button('复制预览地址').waitFor()
  const active = await invoke('project_state')
  const missing = 'Build/missing-regression.wasm'
  const response = await fetch(new URL(`${missing}?private=ignore`, active.url))
  assert.equal(response.status, 404)
  const logPage = link('运行日志')
  if (await logPage.count()) {
    assert.equal(await webview.getByRole('log', { name: '服务事件' }).count(), 0)
    await logPage.click()
  }
  await webview.getByText(`404 · ${missing}`, { exact: true }).waitFor()
  await webview
    .getByRole('log', { name: '服务事件' })
    .getByText(/missing-regression\.wasm/)
    .waitFor()
  await webview.screenshot({ path: join(report, 'recent-and-diagnostics.png') })
  if (await logPage.count()) await link('单项目').click()
  await button('停止服务').click()
  await button('复制预览地址').waitFor({ state: 'hidden' })

  await link('多项目').click()
  await webview.getByRole('textbox', { name: '发布总目录' }).fill(dirname(source))
  await webview.getByRole('button', { name: /^(扫描项目|重新扫描)$/ }).click()
  const first = webview.getByRole('button', { name: /^vmc580\s+vmc580\/index\.html/ })
  const second = webview.getByRole('button', { name: /^vmc850\s+vmc850\/index\.html/ })
  await button('收藏 vmc850').click()
  await button('取消收藏 vmc850').waitFor()
  assert.match(await webview.locator('.project-row').first().innerText(), /^vmc850/)

  await first.click()
  const threads = webview.getByRole('switch', { name: '多线程支持' })
  if ((await threads.getAttribute('aria-checked')) !== 'true') await threads.click()
  assert.equal(await threads.getAttribute('aria-checked'), 'true')
  await second.click()
  assert.equal(await threads.getAttribute('aria-checked'), 'false')
  await first.click()
  assert.equal(await threads.getAttribute('aria-checked'), 'true')
  await webview.reload()
  await button('取消收藏 vmc850').waitFor()
  await first.click()
  assert.equal(await threads.getAttribute('aria-checked'), 'true')
  await webview.screenshot({ path: join(report, 'favorites.png') })
  await link('单项目').click()
  await recent.click()
  assert.equal(
    await webview.getByRole('switch', { name: '多线程支持' }).getAttribute('aria-checked'),
    'true',
  )
  await webview.getByRole('button', { name: /^移除最近项目 .*vmc580$/i }).click()
  await recent.waitFor({ state: 'hidden' })

  const checks = await invoke('check_recent_projects', {
    paths: [source, join(source, 'no-such-project')],
  })
  assert.deepEqual(checks, [true, false])
  const unavailable = join(source, 'no-such-project')
  await webview.evaluate((root) => {
    const key = 'unity-webgl.launcher.recent.v1'
    const items = JSON.parse(localStorage.getItem(key) ?? '[]')
    localStorage.setItem(key, JSON.stringify([{ root }, ...items]))
  }, unavailable)
  await webview.reload()
  await webview.getByText('路径失效', { exact: true }).waitFor()
  assert.ok(await button(`选择最近项目 ${unavailable}`).isDisabled())
  return {
    recent: true,
    liveMissingWasm: true,
    independentThreadsPersisted: true,
    favoritesPersisted: true,
    invalidPath: true,
  }
}
