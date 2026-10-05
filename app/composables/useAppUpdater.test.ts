import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { ref, readonly } from 'vue'

const metadata = { rid: 41, currentVersion: '0.0.7', version: '0.0.8', body: 'Release notes', rawJson: {} }
let nextUpdate: typeof metadata | null
let installError: Error | null
let invoke: ReturnType<typeof vi.fn>

beforeEach(() => {
  vi.resetModules()
  nextUpdate = metadata
  installError = null
  invoke = vi.fn(async (command: string, args: any) => {
    if (command === 'plugin:app|version') return '0.0.7'
    if (command === 'plugin:updater|check') return nextUpdate
    if (command === 'plugin:updater|download_and_install') {
      if (installError) throw installError
      args.onEvent.onmessage({ event: 'Started', data: { contentLength: 100 } })
      args.onEvent.onmessage({ event: 'Progress', data: { chunkLength: 100 } })
      args.onEvent.onmessage({ event: 'Finished' })
      return
    }
    if (command === 'plugin:process|restart') return
    throw new Error(`Unexpected native command: ${command}`)
  })
  // Exercise the installed Tauri Update/Resource classes, replacing only native IPC.
  vi.stubGlobal('window', { __TAURI_INTERNALS__: { invoke, transformCallback: () => 1 } })
  vi.stubGlobal('ref', ref)
  vi.stubGlobal('readonly', readonly)
  vi.stubGlobal('localStorage', { getItem: () => null, setItem: vi.fn() })
})

afterEach(() => { vi.unstubAllGlobals(); vi.restoreAllMocks() })

describe('app updater resource handle', () => {
  it('installs from the shared handle and relaunches only after success', async () => {
    const { useAppUpdater } = await import('./useAppUpdater')
    const banner = useAppUpdater()
    const settings = useAppUpdater()
    await banner.checkForUpdate()
    expect(settings.state.value).toBe('available')
    expect(settings.latestVersion.value).toBe('0.0.8')
    await settings.installAndRelaunch()
    expect(banner.errorMessage.value).toBe('')
    expect(banner.state.value).toBe('ready')
    expect(banner.progress.value).toBe(100)
    expect(invoke).toHaveBeenCalledWith('plugin:updater|download_and_install', expect.objectContaining({ rid: 41 }), undefined)
    expect(invoke).toHaveBeenCalledWith('plugin:process|restart', {}, undefined)
  })

  it('does not relaunch when installation fails', async () => {
    installError = new Error('signature verification failed')
    vi.spyOn(console, 'error').mockImplementation(() => {})
    const { useAppUpdater } = await import('./useAppUpdater')
    const updater = useAppUpdater()
    await updater.checkForUpdate()
    await updater.installAndRelaunch()
    expect(updater.state.value).toBe('error')
    expect(updater.errorMessage.value).toContain('signature could not be verified')
    expect(invoke.mock.calls.some(([command]) => command === 'plugin:process|restart')).toBe(false)
  })

  it('clears a previous handle when a subsequent check finds no update', async () => {
    const { useAppUpdater } = await import('./useAppUpdater')
    const updater = useAppUpdater()
    await updater.checkForUpdate()
    nextUpdate = null
    await updater.checkForUpdate()
    await updater.downloadAndInstall()
    expect(updater.state.value).toBe('upToDate')
    expect(invoke.mock.calls.some(([command]) => command === 'plugin:updater|download_and_install')).toBe(false)
  })
})
