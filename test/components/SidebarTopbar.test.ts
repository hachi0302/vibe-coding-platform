import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import SidebarTopbar from '../../src/components/SidebarTopbar.vue'
import { vTooltip } from '../../src/tooltip'

const factory = (props: Partial<InstanceType<typeof SidebarTopbar>['$props']> = {}) =>
  mount(SidebarTopbar, {
    props: { showTrash: false, hasTrash: false, ...props },
    global: { directives: { tooltip: vTooltip } },
  })

describe('SidebarTopbar', () => {
  // 左 1 颗（toggle）+ 右 3 颗（stats / trash / more）= 4。refresh 按钮搬到了
  // Sidebar 内部「{agent} · N projects」那一行，跟 agent 切换更贴近。
  // history / pricing 收进 more 的 dropdown，不算独立 .top-btn。
  it('renders the toggle, stats, trash and more buttons', () => {
    expect(factory().findAll('.top-btn')).toHaveLength(4)
  })

  it('emits toggle-sidebar / open-stats / open-trash on the matching click', async () => {
    const wrapper = factory()
    const [toggle] = wrapper.findAll('.topbar-icons')[0].findAll('.top-btn')
    const rightIcons = wrapper.findAll('.topbar-icons')[1].findAll('.top-btn')
    // 右组顺序：stats / trash / more
    const [stats, trash] = rightIcons
    await toggle.trigger('click')
    await stats.trigger('click')
    await trash.trigger('click')

    expect(wrapper.emitted('toggle-sidebar')).toHaveLength(1)
    expect(wrapper.emitted('open-stats')).toHaveLength(1)
    expect(wrapper.emitted('open-trash')).toHaveLength(1)
  })

  // More 菜单：点 .top-btn（右组最后一颗）展开，dropdown 里点 history / pricing 分别发对应事件。
  it('opens a dropdown from the more button and emits open-history / open-pricing', async () => {
    const wrapper = factory()
    const moreBtn = wrapper.findAll('.topbar-icons')[1].findAll('.top-btn')[2]
    // 初始无 dropdown
    expect(wrapper.find('.topbar-more-menu').exists()).toBe(false)
    await moreBtn.trigger('click')
    expect(wrapper.find('.topbar-more-menu').exists()).toBe(true)

    const items = wrapper.findAll('.topbar-more-item')
    expect(items.length).toBe(2)
    await items[0].trigger('click')
    expect(wrapper.emitted('open-history')).toHaveLength(1)
    // dropdown 关掉了，下面 pricing 要重新打开 more 菜单
    expect(wrapper.find('.topbar-more-menu').exists()).toBe(false)
    await moreBtn.trigger('click')
    await wrapper.findAll('.topbar-more-item')[1].trigger('click')
    expect(wrapper.emitted('open-pricing')).toHaveLength(1)
  })

  it('highlights the trash button when the trash view is open', () => {
    expect(factory({ showTrash: true }).find('.topbar-trash-btn').classes()).toContain('active')
    expect(factory({ showTrash: false }).find('.topbar-trash-btn').classes()).not.toContain('active')
  })

  it('highlights the stats button when the stats view is open', () => {
    const wrapper = factory({ showStats: true })
    // 第二组按钮顺序：stats / trash / more —— stats 在索引 0。
    const stats = wrapper.findAll('.topbar-icons')[1].findAll('.top-btn')[0]
    expect(stats.classes()).toContain('active')
  })

  // more 按钮的 active 高亮表示「相关 view 已打开（history 或 pricing 之一）」，
  // 即使 dropdown 没展开 —— 给用户一个视觉提示：当前主区是 More 菜单里的某项。
  it('highlights the more button when history or pricing view is open', () => {
    const historyOpen = factory({ showHistory: true })
    expect(historyOpen.findAll('.topbar-icons')[1].findAll('.top-btn')[2].classes()).toContain('active')
    const pricingOpen = factory({ showPricing: true })
    expect(pricingOpen.findAll('.topbar-icons')[1].findAll('.top-btn')[2].classes()).toContain('active')
  })

  it('shows the trash dot only when there is trashed content', () => {
    expect(factory({ hasTrash: true }).find('.trash-dot').exists()).toBe(true)
    expect(factory({ hasTrash: false }).find('.trash-dot').exists()).toBe(false)
  })
})
