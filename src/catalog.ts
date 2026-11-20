import { computed, onMounted, onScopeDispose, shallowRef } from 'vue'
import { useLocalStorage } from '@vueuse/core'
import { desktop } from './desktop'
import { errorText, projectPath, sameProjectRoot, type CatalogScan } from './project'

export function useCatalog() {
  const savedDirectory = useLocalStorage('unity-webgl.launcher.catalog-directory.v1', '')
  const directory = shallowRef(savedDirectory.value)
  const scannedInput = shallowRef('')
  const scan = shallowRef<CatalogScan | null>(null)
  const pending = shallowRef<'pick' | 'scan' | null>(null)
  const error = shallowRef('')
  let disposed = false
  onScopeDispose(() => { disposed = true })

  const normalizedDirectory = computed(() => projectPath(directory.value))
  const currentScan = computed(() =>
    scan.value && (sameProjectRoot(scannedInput.value, normalizedDirectory.value) ||
      sameProjectRoot(scan.value.root, normalizedDirectory.value)) ? scan.value : null,
  )

  function updateDirectory(value: string) {
    directory.value = value
    error.value = ''
  }

  async function scanDirectory() {
    const path = normalizedDirectory.value
    if (!desktop.available || pending.value || !path) return
    pending.value = 'scan'
    error.value = ''
    // Do not reuse an earlier listing if refreshing the same path now fails.
    scan.value = null
    try {
      const result = await desktop.scan(path)
      if (disposed) return
      scannedInput.value = path
      directory.value = result.root
      savedDirectory.value = result.root
      scan.value = result
    } catch (cause) {
      if (!disposed) error.value = errorText(cause)
    } finally {
      if (!disposed) pending.value = null
    }
  }

  async function pickDirectory() {
    if (!desktop.available || pending.value) return
    pending.value = 'pick'
    error.value = ''
    let selected: string | null = null
    try {
      selected = await desktop.pickCatalog()
    } catch (cause) {
      if (!disposed) error.value = errorText(cause)
    } finally {
      if (!disposed) pending.value = null
    }
    if (selected && !disposed) {
      updateDirectory(selected)
      await scanDirectory()
    }
  }

  onMounted(() => { void scanDirectory() })

  return { directory, normalizedDirectory, currentScan, pending, error, updateDirectory, scanDirectory, pickDirectory }
}
