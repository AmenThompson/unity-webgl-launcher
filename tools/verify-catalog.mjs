import assert from 'node:assert/strict'

export async function verifyCatalog({ webview, preview, invoke, directory, screenshot }) {
  const navigation = webview.getByRole('navigation', { name: '主导航' })
  const labels = await navigation.getByRole('link').allTextContents()
  assert.deepEqual(labels.slice(0, 2).map((label) => label.trim()), ['单项目', '多项目'])
  await navigation.getByRole('link', { name: '多项目', exact: true }).click()
  await webview.getByRole('heading', { level: 1, name: '多项目' }).waitFor()
  const input = webview.getByRole('textbox', { name: '发布总目录' })
  await input.fill(directory)
  await webview.getByRole('button', { name: /^(扫描项目|重新扫描)$/ }).click()
  const row = (name) =>
    webview.getByRole('button', { name: new RegExp(`^${name}\\s+${name}/index\\.html`) })
  const first = row('vmc580')
  const second = row('vmc850')
  await first.waitFor()
  await second.waitFor()
  if (process.argv.includes('--double-icons')) {
    const star = await webview.getByRole('button', { name: /^(收藏|取消收藏) vmc580$/ }).locator('svg').boundingBox()
    assert.ok(star && star.width === 34 && star.height === 34)
  }
  const search = webview.getByRole('textbox', { name: '搜索项目名称或路径' })
  await search.fill('vmc580')
  await second.waitFor({ state: 'hidden' })
  await search.clear()
  const listPanel = webview.getByRole('region', { name: '发现的发布项目' })
  const initialHeight = (await listPanel.boundingBox()).height
  await first.click()
  await webview.getByRole('button', { name: '启动并打开' }).click()
  const runBadge = (projectRow) => labels.includes('收藏项目')
    ? projectRow.getByText('运行中', { exact: true })
    : webview.getByText(`当前运行 ${projectRow === first ? 'vmc580' : 'vmc850'}`)
  await runBadge(first).waitFor()
  const firstState = await invoke('project_state')
  await waitScene(preview, firstState.url)

  await second.click()
  await webview.getByRole('button', { name: '切换并打开' }).waitFor()
  if (process.argv.includes('--panel-heights')) {
    assert.ok(Math.abs((await listPanel.boundingBox()).height - initialHeight) < 2)
  }
  assert.equal((await invoke('project_state')).url, firstState.url)
  await assert.rejects(
    invoke('start_project', {
      path: `${directory}\\missing\\index.html`,
      catalogRoot: directory,
      mode: 'auto',
      threads: false,
    }),
  )
  assert.equal((await invoke('project_state')).url, firstState.url)
  assert.equal((await fetch(firstState.url)).status, 200)
  await webview.screenshot({ path: screenshot })

  await webview.getByRole('button', { name: '切换并打开' }).click()
  await runBadge(second).waitFor()
  const nextState = await invoke('project_state')
  assert.notEqual(nextState.url, firstState.url)
  await assert.rejects(fetch(firstState.url))
  await waitScene(preview, nextState.url)
  await webview.getByRole('button', { name: '重新打开' }).click()
  assert.equal((await invoke('project_state')).url, nextState.url)
  await webview.getByRole('button', { name: '停止当前项目' }).click()
  await webview.getByRole('button', { name: '停止当前项目' }).waitFor({ state: 'hidden' })
  await assert.rejects(fetch(nextState.url))
  return {
    projectCount: await webview.locator('.project-row').count(),
    filtered: true,
    failedSwitchPreserved: true,
    scenes: ['vmc580', 'vmc850'],
    oldAddressClosed: true,
  }
}

async function waitScene(page, address) {
  assert.ok(address)
  await page.goto(address)
  await page.waitForFunction(
    () =>
      document.querySelector('#loading-cover')?.style.display === 'none' &&
      document.querySelector('#unity-fullscreen-button')?.style.display !== 'none',
    null,
    { timeout: 120_000 },
  )
}
