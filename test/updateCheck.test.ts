import { beforeEach, describe, expect, it, vi } from 'vitest'

const { appVersionMock, checkUpdateMock } = vi.hoisted(() => ({
  appVersionMock: vi.fn(),
  checkUpdateMock: vi.fn(),
}))

vi.mock('../src/api', () => ({
  appVersion: appVersionMock,
  checkUpdate: checkUpdateMock,
  openUrl: vi.fn(),
}))
vi.mock('@tauri-apps/plugin-updater', () => ({ check: vi.fn() }))
vi.mock('@tauri-apps/plugin-process', () => ({ relaunch: vi.fn() }))

import {
  downloadAndInstallUpdate,
  latestVersion,
  runBackgroundCheck,
  updateAvailable,
  updateDownloaded,
  updateDownloading,
  updaterUpdate,
} from '../src/updateCheck'

describe('runBackgroundCheck', () => {
  beforeEach(() => {
    localStorage.clear()
    appVersionMock.mockReset().mockResolvedValue('0.1.0')
    checkUpdateMock.mockReset().mockResolvedValue({
      current: '0.1.0',
      latest: '0.1.0',
      hasUpdate: false,
    })
    latestVersion.value = null
    updateAvailable.value = false
    updateDownloaded.value = false
    updateDownloading.value = false
    updaterUpdate.value = null
  })

  it('ignores an update cache left by another desktop application', async () => {
    localStorage.setItem('updateCheck:v1', JSON.stringify({
      checkedAt: Date.now(),
      latest: '0.3.2',
    }))

    await runBackgroundCheck()

    expect(checkUpdateMock).toHaveBeenCalledOnce()
    expect(localStorage.getItem('updateCheck:v1')).toBeNull()
    expect(latestVersion.value).toBe('0.1.0')
    expect(updateAvailable.value).toBe(false)
  })

  it('retries an interrupted updater download and completes without surfacing its transport error', async () => {
    vi.useFakeTimers()
    const downloadAndInstall = vi.fn()
      .mockRejectedValueOnce(new Error('error decoding response body'))
      .mockResolvedValueOnce(undefined)
    updaterUpdate.value = { downloadAndInstall } as never

    const download = downloadAndInstallUpdate()
    await vi.advanceTimersByTimeAsync(750)
    await download

    expect(downloadAndInstall).toHaveBeenCalledTimes(2)
    expect(updateDownloaded.value).toBe(true)
    expect(updateDownloading.value).toBe(false)
    vi.useRealTimers()
  })
})
