import { getVersion } from '@tauri-apps/api/app'
import { relaunch } from '@tauri-apps/plugin-process'
import { check, type Update } from '@tauri-apps/plugin-updater'

/// Throttle window for the silent boot-time check.
const BOOT_CHECK_INTERVAL_MS = 6 * 60 * 60 * 1000
const LAST_CHECKED_KEY = 'queryStudio.updater.lastChecked'

export type UpdaterState =
  | 'idle'
  | 'checking'
  | 'available'
  | 'downloading'
  | 'ready'
  | 'upToDate'
  | 'error'

interface UpdaterStore {
  state: Ref<UpdaterState>
  progress: Ref<number>
  currentVersion: Ref<string>
  latestVersion: Ref<string>
  releaseNotes: Ref<string>
  lastChecked: Ref<Date | null>
  errorMessage: Ref<string>
  pendingUpdate: Ref<Update | null>
}

let store: UpdaterStore | null = null

/// Map raw updater errors to something we'd actually want to show a user.
function friendlyUpdateError(raw: string): string {
  const lower = raw.toLowerCase()
  if (lower.includes('could not fetch a valid release json')
    || lower.includes('404')
    || lower.includes('not found')) {
    return 'No update manifest is published yet. Try again after the next release.'
  }
  if (lower.includes('signature')) {
    return 'Update signature could not be verified. The download was rejected for safety.'
  }
  if (lower.includes('network') || lower.includes('failed to send request')) {
    return 'Could not reach the update server. Check your internet connection.'
  }
  return raw
}

function getStore(): UpdaterStore {
  if (store) return store
  store = {
    state: ref<UpdaterState>('idle'),
    progress: ref(0),
    currentVersion: ref(''),
    latestVersion: ref(''),
    releaseNotes: ref(''),
    lastChecked: ref<Date | null>(null),
    errorMessage: ref(''),
    pendingUpdate: ref<Update | null>(null),
  }
  return store
}

/// Reactive, app-wide updater state.
///
/// Wraps `@tauri-apps/plugin-updater` so the whole UI shares one source of
/// truth. Safe to call from many components — the underlying `Update` handle
/// is cached so we never re-download a bundle we already have.
export function useAppUpdater() {
  const s = getStore()

  async function loadCurrentVersion(): Promise<void> {
    if (s.currentVersion.value) return
    try {
      s.currentVersion.value = await getVersion()
    }
    catch (err) {
      console.warn('Failed to read app version', err)
    }
  }

  /// Run a silent check at most once per `BOOT_CHECK_INTERVAL_MS`.
  /// Used on app launch so we don't hammer GitHub on every reload.
  async function maybeCheckOnBoot(): Promise<void> {
    const lastRaw = (typeof localStorage !== 'undefined')
      ? localStorage.getItem(LAST_CHECKED_KEY)
      : null
    const last = lastRaw ? Number(lastRaw) : 0
    if (Number.isFinite(last) && Date.now() - last < BOOT_CHECK_INTERVAL_MS) {
      return
    }
    await checkForUpdate({ silent: true })
  }

  async function checkForUpdate(opts: { silent?: boolean } = {}): Promise<void> {
    if (s.state.value === 'checking' || s.state.value === 'downloading') return

    await loadCurrentVersion()
    s.state.value = 'checking'
    s.errorMessage.value = ''

    try {
      const update = await check()
      s.lastChecked.value = new Date()
      if (typeof localStorage !== 'undefined') {
        localStorage.setItem(LAST_CHECKED_KEY, String(s.lastChecked.value.getTime()))
      }

      if (!update) {
        s.state.value = opts.silent ? 'idle' : 'upToDate'
        s.pendingUpdate.value = null
        s.latestVersion.value = s.currentVersion.value
        return
      }

      s.pendingUpdate.value = update
      s.latestVersion.value = update.version
      s.releaseNotes.value = update.body ?? ''
      s.state.value = 'available'
    }
    catch (err) {
      const msg = err instanceof Error ? err.message : String(err)
      // Silent boot-time checks shouldn't push the UI into an error state —
      // a 404 just means the current release has no `latest.json` yet, which
      // is normal during development and right after a manifest-less release.
      if (opts.silent) {
        s.state.value = 'idle'
        s.errorMessage.value = ''
        console.debug('Background update check skipped:', msg)
        return
      }
      s.state.value = 'error'
      s.errorMessage.value = friendlyUpdateError(msg)
      console.error('Update check failed', err)
    }
  }

  async function downloadAndInstall(): Promise<void> {
    const update = s.pendingUpdate.value
    if (!update) return

    s.state.value = 'downloading'
    s.progress.value = 0
    s.errorMessage.value = ''

    try {
      let downloaded = 0
      let contentLength = 0

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started':
            contentLength = event.data.contentLength ?? 0
            s.progress.value = 0
            break
          case 'Progress':
            downloaded += event.data.chunkLength
            if (contentLength > 0) {
              s.progress.value = Math.min(
                100,
                Math.round((downloaded / contentLength) * 100),
              )
            }
            break
          case 'Finished':
            s.progress.value = 100
            break
        }
      })

      s.state.value = 'ready'
    }
    catch (err) {
      s.state.value = 'error'
      s.errorMessage.value = friendlyUpdateError(
        err instanceof Error ? err.message : String(err),
      )
      console.error('Update install failed', err)
    }
  }

  async function installAndRelaunch(): Promise<void> {
    if (s.state.value !== 'ready') {
      await downloadAndInstall()
      if (s.state.value !== 'ready') return
    }
    await relaunch()
  }

  function dismiss(): void {
    if (s.state.value === 'available' || s.state.value === 'ready' || s.state.value === 'upToDate') {
      s.state.value = 'idle'
    }
  }

  return {
    state: readonly(s.state),
    progress: readonly(s.progress),
    currentVersion: readonly(s.currentVersion),
    latestVersion: readonly(s.latestVersion),
    releaseNotes: readonly(s.releaseNotes),
    lastChecked: readonly(s.lastChecked),
    errorMessage: readonly(s.errorMessage),
    loadCurrentVersion,
    maybeCheckOnBoot,
    checkForUpdate,
    downloadAndInstall,
    installAndRelaunch,
    dismiss,
  }
}
