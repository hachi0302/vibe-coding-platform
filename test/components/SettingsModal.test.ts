import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'

const { appVersionMock, checkAppUpdateMock } = vi.hoisted(() => ({
  appVersionMock: vi.fn(),
  checkAppUpdateMock: vi.fn(),
}))
vi.mock('../../src/api', () => ({
  appVersion: appVersionMock,
}))
vi.mock('../../src/updateCheck', async (importOriginal) => {
  const orig: any = await importOriginal()
  return { ...orig, checkAppUpdate: checkAppUpdateMock }
})

import SettingsModal from '../../src/components/SettingsModal.vue'
import { vTooltip } from '../../src/tooltip'
import { lang, setLang, setTheme, theme } from '../../src/settings'

beforeEach(() => {
  setLang('en')
  setTheme('system')
  appVersionMock.mockReset().mockResolvedValue('9.9.9')
  checkAppUpdateMock.mockReset()
})
afterEach(() => {
  setLang('en')
  setTheme('system')
})

type Props = InstanceType<typeof SettingsModal>['$props']
const factory = (props: Partial<Props> = {}) =>
  mount(SettingsModal, {
    props: { cacheBytes: 0, ...props } as Props,
    global: { directives: { tooltip: vTooltip } },
    attachTo: document.body,
  })

describe('SettingsModal', () => {
  it('shows a human-readable cache size', () => {
    expect(factory({ cacheBytes: 2048 }).find('.set-section-tail').text()).toBe('2.0 KB')
  })

  it('shows "0 B" and the clear button is always enabled', () => {
    const wrapper = factory({ cacheBytes: 0 })
    expect(wrapper.find('.set-section-tail').text()).toBe('0 B')
    expect(wrapper.find('.btn.danger').attributes('disabled')).toBeUndefined()
  })

  it('enables the clear button and emits clearCache when there is cached data', async () => {
    const wrapper = factory({ cacheBytes: 4096 })
    const clearBtn = wrapper.find('.btn.danger')
    expect(clearBtn.attributes('disabled')).toBeUndefined()
    await clearBtn.trigger('click')
    expect(wrapper.emitted('clearCache')).toHaveLength(1)
  })

  it('emits close only from the X button, not the overlay backdrop', async () => {
    const wrapper = factory()
    await wrapper.find('.overlay').trigger('click')
    expect(wrapper.emitted('close')).toBeUndefined()
    await wrapper.find('.modal-close').trigger('click')
    expect(wrapper.emitted('close')).toHaveLength(1)
  })

  it('switches language via the custom dropdown', async () => {
    const wrapper = factory()
    const dropdowns = wrapper.findAll('.set-dropdown-btn')
    await dropdowns[0].trigger('click')
    const items = wrapper.findAll('.set-dropdown-item')
    expect(items.length).toBeGreaterThanOrEqual(4)
    await items[1].trigger('click') // 简体中文
    expect(lang.value).toBe('zh')
  })

  it('switches theme via the custom dropdown', async () => {
    const wrapper = factory()
    const dropdowns = wrapper.findAll('.set-dropdown-btn')
    await dropdowns[1].trigger('click')
    const items = wrapper.findAll('.set-dropdown-item')
    // find the Dracula option (last one)
    await items[items.length - 1].trigger('click')
    expect(theme.value).toBe('dracula')
  })

  it('loads the app version on mount', async () => {
    // 版本与更新操作现在住在「Updates」tab 里
    const wrapper = factory({ initialTab: 'updates' })
    await flushPromises()
    expect(appVersionMock).toHaveBeenCalled()
    expect(wrapper.text()).toContain('v9.9.9')
  })

  it('reports when an update is available', async () => {
    checkAppUpdateMock.mockResolvedValue({ hasUpdate: true, latest: '2.0.0', current: '1.0.0' })
    const wrapper = factory({ initialTab: 'updates' })
    await flushPromises()

    const checkBtn = wrapper.find('.set-update-cta .btn')
    await checkBtn.trigger('click')
    await flushPromises()

    expect(checkAppUpdateMock).toHaveBeenCalled()
    expect(wrapper.text()).toContain('2.0.0')
  })

  it('reports when the app is up to date', async () => {
    checkAppUpdateMock.mockResolvedValue({ hasUpdate: false, latest: '1.0.0', current: '1.0.0' })
    const wrapper = factory({ initialTab: 'updates' })
    await flushPromises()

    const checkBtn = wrapper.find('.set-update-cta .btn')
    await checkBtn.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('latest version')
  })

  it('surfaces a failed update check', async () => {
    checkAppUpdateMock.mockRejectedValue(new Error('offline'))
    const wrapper = factory({ initialTab: 'updates' })
    await flushPromises()

    const checkBtn = wrapper.find('.set-update-cta .btn')
    await checkBtn.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('Update check failed')
  })
})
