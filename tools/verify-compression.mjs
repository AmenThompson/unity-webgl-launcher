import assert from 'node:assert/strict'
import { createReadStream, createWriteStream } from 'node:fs'
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { basename, join } from 'node:path'
import { pipeline } from 'node:stream/promises'
import {
  brotliCompressSync,
  brotliDecompressSync,
  constants,
  createBrotliCompress,
  createGzip,
  gunzipSync,
  gzipSync,
} from 'node:zlib'

// Test copies only. The Unity editor project and original export stay untouched.
export async function verifyCompression({ invoke, preview, source, report }) {
  const scratch = await mkdtemp(join(tmpdir(), 'unity-launcher-matrix-'))
  const results = []
  try {
    const original = await readFile(join(source, 'index.html'), 'utf8')
    const paths = [
      ...original.matchAll(/(?:dataUrl|frameworkUrl|codeUrl)\s*:\s*buildUrl\s*\+\s*"\/([^"/]+)"/g),
    ].map((match) => match[1])
    assert.equal(paths.length, 3, '需要与 vmc580 类似的未压缩测试导出模板')
    for (const [format, suffix, encoding] of [
      ['brotli', '.br', 'br'],
      ['gzip', '.gz', 'gzip'],
    ]) {
      const root = join(scratch, `${format}-浏览回归`)
      await cp(source, root, { recursive: true })
      let html = original
      for (const filename of paths) {
        const uncompressed = join(root, 'Build', filename)
        const compress =
          format === 'brotli'
            ? createBrotliCompress({ params: { [constants.BROTLI_PARAM_QUALITY]: 3 } })
            : createGzip()
        await pipeline(
          createReadStream(uncompressed),
          compress,
          createWriteStream(`${uncompressed}${suffix}`),
        )
        await rm(uncompressed)
        html = html.replace(`"/${filename}"`, `"/${filename}${suffix}"`)
      }
      await writeFile(join(root, 'index.html'), html)
      const state = await invoke('start_project', { path: root, mode: 'auto', threads: false })
      assert.equal(state.format, format)
      assert.equal(state.fallback, false)
      for (const filename of paths) {
        const response = await fetch(new URL(`Build/${filename}${suffix}`, state.url), {
          method: 'HEAD',
        })
        assert.equal(response.status, 200)
        assert.equal(response.headers.get('content-encoding'), encoding)
      }
      await preview.goto(state.url)
      await preview.waitForFunction(
        () =>
          document.querySelector('#loading-cover')?.style.display === 'none' &&
          document.querySelector('#unity-fullscreen-button')?.style.display !== 'none',
        null,
        { timeout: 120_000 },
      )
      await preview.screenshot({ path: join(report, `unity-${format}.png`) })
      results.push({ format, fallback: false, realUnityScene: true })
      await invoke('stop_project')
    }

    const wasmBytes = Buffer.from('0061736d01000000', 'hex')
    for (const [format, compress, decompress] of [
      ['brotli', brotliCompressSync, brotliDecompressSync],
      ['gzip', gzipSync, gunzipSync],
    ]) {
      const root = join(scratch, `${format}-fallback`)
      await mkdir(join(root, 'Build'), { recursive: true })
      const filename = 'fixture.wasm.unityweb'
      const compressed = compress(wasmBytes)
      await writeFile(join(root, 'Build', filename), compressed)
      await writeFile(
        join(root, 'index.html'),
        `<script>const buildUrl = 'Build'; const config = { codeUrl: buildUrl + '/${filename}' };</script>`,
      )
      const state = await invoke('start_project', {
        path: join(root, 'index.html'),
        mode: 'auto',
        threads: false,
      })
      assert.equal(state.format, format)
      assert.equal(state.fallback, true)
      const response = await fetch(new URL(`Build/${filename}`, state.url))
      assert.equal(response.status, 200)
      assert.equal(response.headers.get('content-encoding'), null)
      assert.match(response.headers.get('content-type'), /^application\/wasm/)
      const received = Buffer.from(await response.arrayBuffer())
      assert.deepEqual(received, compressed, basename(root))
      assert.deepEqual(decompress(received), wasmBytes)
      assert.ok(await WebAssembly.compile(decompress(received)))
      await preview.goto(state.url)
      const browserDecodedGzip =
        format === 'gzip' &&
        (await preview.evaluate(async (name) => {
          const response = await fetch(`Build/${name}`)
          const decoded = response.body.pipeThrough(new DecompressionStream('gzip'))
          const wasmResponse = new Response(decoded, { headers: { 'Content-Type': 'application/wasm' } })
          return (await WebAssembly.compileStreaming(wasmResponse)) instanceof WebAssembly.Module
        }, filename))
      if (format === 'gzip') assert.equal(browserDecodedGzip, true)
      results.push({ format, fallback: true, rawPayloadAndWasmValidated: true, browserDecodedGzip })
      await invoke('stop_project')
    }
    return results
  } finally {
    await invoke('stop_project').catch(() => {})
    await rm(scratch, { recursive: true, force: true, maxRetries: 3 })
  }
}
