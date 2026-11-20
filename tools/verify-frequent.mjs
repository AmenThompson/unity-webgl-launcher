import assert from 'node:assert/strict'
import { dirname, join } from 'node:path'

export async function verifyFrequent({ webview, preview, invoke, source, report }) {
  assert.ok(process.env.WEBVIEW2_USER_DATA_FOLDER, 'Use an isolated WebView2 profile')
  const second = join(dirname(source), 'vmc850')
  const missing = join(dirname(source), 'missing-frequent-regression')
  await webview.evaluate(({ second, missing }) => {
    localStorage.setItem('unity-webgl.launcher.favorites.v1', JSON.stringify([second.toLowerCase(), missing]))
  }, { second, missing })
  await webview.reload()
  const separate = await webview.getByRole('link', { name: '收藏项目', exact: true }).count() > 0
  const favoritesLabel = separate ? '收藏项目' : '常用项目'
  await webview.getByRole('link', { name: favoritesLabel, exact: true }).click()
  await webview.getByRole('heading', { name: favoritesLabel, exact: true }).waitFor()
  if (separate) {
    assert.equal(await webview.locator('.n-tabs').count(), 0)
    assert.equal(await webview.locator('.entry').filter({ hasText: 'vmc580' }).count(), 0)
    await webview.screenshot({ path: join(report, 'favorites-page.png') })
  }
  const row = (name) => webview.locator('.entry').filter({ hasText: name })
  const action = (name) => webview.getByRole('button', { name, exact: true })
  await row('missing-frequent-regression').getByText('路径失效').waitFor()
  if (process.argv.includes('--frequent-icons')) {
    const icon = action('取消收藏 missing-frequent-regression')
    assert.equal(await icon.innerText(), '')
    assert.equal(await icon.getAttribute('title'), '取消收藏')
    await icon.locator('svg.lucide-star').waitFor()
    const headerBackground = await webview.locator('.topbar').evaluate((header) => {
      const style = getComputedStyle(header)
      return [style.backgroundImage, style.backgroundColor]
    })
    assert.deepEqual(headerBackground, ['none', 'rgba(0, 0, 0, 0)'])
    await webview.screenshot({ path: join(report, 'favorite-icons.png') })
  }
  if (process.argv.includes('--double-icons')) {
    const star = await action('取消收藏 vmc850').locator('svg').boundingBox()
    assert.ok(star && star.width === 34 && star.height === 34)
  }
  await row('missing-frequent-regression').click()
  assert.ok(await action('启动并打开').isDisabled())
  await action('取消收藏 missing-frequent-regression').click()
  await row('missing-frequent-regression').waitFor({ state: 'hidden' })
  await row('vmc850').click()
  await action('启动并打开').click()
  const reopen = action('在浏览器中重新打开')
  await reopen.waitFor()
  const initialUrl = (await invoke('project_state')).url
  assert.ok(initialUrl)
  assert.equal((await fetch(new URL('Build/vmc850.wasm', initialUrl), { method: 'HEAD' })).status, 200)
  await reopen.click()
  assert.equal((await invoke('project_state')).url, initialUrl)
  if (separate) {
    await webview.getByRole('link', { name: '最近项目', exact: true }).click()
    await webview.getByRole('heading', { name: '最近项目', exact: true }).waitFor()
    await action('清空历史').waitFor()
  } else {
    await webview.locator('.n-tabs').getByText('最近', { exact: true }).click()
  }
  if (process.argv.includes('--double-icons')) {
    const cross = await action('移除记录 vmc580').locator('svg').boundingBox()
    assert.ok(cross && cross.width === 28 && cross.height === 28)
  }
  await row('vmc580').click()
  if (process.argv.includes('--frequent-icons')) {
    const icon = action('移除记录 vmc580')
    assert.equal(await icon.innerText(), '')
    assert.equal(await icon.getAttribute('title'), '移除记录')
    await icon.locator('svg.lucide-x').waitFor()
  }
  assert.equal((await invoke('project_state')).url, initialUrl)
  await webview.screenshot({ path: join(report, 'frequent-projects.png') })
  await action('切换并打开').click()
  await reopen.waitFor()
  const nextUrl = (await invoke('project_state')).url
  assert.ok(nextUrl && nextUrl !== initialUrl)
  await assert.rejects(fetch(initialUrl))
  await preview.goto(nextUrl)
  await preview.waitForFunction(() => document.querySelector('#loading-cover')?.style.display === 'none' && document.querySelector('#unity-fullscreen-button')?.style.display !== 'none', null, { timeout: 120_000 })
  await action('停止当前项目').click()
  await action('停止当前项目').waitFor({ state: 'hidden' })
  await assert.rejects(fetch(nextUrl))
  return { previousFavorites: true, recent: true, invalidPathDisabled: true, opened: true, reopenedSameUrl: true, switchedAndRendered: true }
}
