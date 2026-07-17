import { describe, expect, it, vi, beforeEach } from 'vitest'
import { defineComponent, h, ref, nextTick } from 'vue'
import { mount } from '@vue/test-utils'

// useGitBranch 唯一的副作用是调后端 gitCurrentBranch —— mock 掉它，断言取值 / 容错 / 重取。
const { gitCurrentBranchMock } = vi.hoisted(() => ({ gitCurrentBranchMock: vi.fn() }))
vi.mock('../src/api', () => ({ gitCurrentBranch: gitCurrentBranchMock }))

import { useGitBranch } from '../src/gitBranch'

// 最小宿主组件：composable 的 watch 需要在组件作用域里跑。把 branch ref 暴露出来断言。
function host(cwd = ref<string | undefined>(undefined)) {
  return defineComponent({
    setup() {
      const branch = useGitBranch(() => cwd.value)
      return { branch }
    },
    render() {
      return h('div', this.branch ?? '')
    },
  })
}

const flush = () => new Promise((r) => setTimeout(r, 0))
const branchOf = (w: ReturnType<typeof mount>) =>
  (w.vm as unknown as { branch: string | null }).branch

describe('useGitBranch', () => {
  beforeEach(() => gitCurrentBranchMock.mockReset())

  it('fetches the branch for the initial cwd', async () => {
    gitCurrentBranchMock.mockResolvedValue('main')
    const wrapper = mount(host(ref('/work/proj')))
    await flush()
    expect(gitCurrentBranchMock).toHaveBeenCalledWith('/work/proj')
    expect(branchOf(wrapper)).toBe('main')
    wrapper.unmount()
  })

  it('stays null without a cwd and never calls the backend', async () => {
    const wrapper = mount(host(ref(undefined)))
    await flush()
    expect(gitCurrentBranchMock).not.toHaveBeenCalled()
    expect(branchOf(wrapper)).toBeNull()
    wrapper.unmount()
  })

  it('refetches when the cwd changes (switching sessions)', async () => {
    gitCurrentBranchMock.mockResolvedValueOnce('main').mockResolvedValueOnce('feature/x')
    const cwd = ref<string | undefined>('/a')
    const wrapper = mount(host(cwd))
    await flush()
    expect(branchOf(wrapper)).toBe('main')
    cwd.value = '/b'
    await nextTick()
    await flush()
    expect(gitCurrentBranchMock).toHaveBeenLastCalledWith('/b')
    expect(branchOf(wrapper)).toBe('feature/x')
    wrapper.unmount()
  })

  it('is null for a non-git cwd (backend resolves null)', async () => {
    // 后端对非 git 仓库返回 None → null；composable 原样透出，调用方据此不渲染分支块。
    gitCurrentBranchMock.mockResolvedValue(null)
    const wrapper = mount(host(ref('/tmp/not-a-repo')))
    await flush()
    expect(gitCurrentBranchMock).toHaveBeenCalledWith('/tmp/not-a-repo')
    expect(branchOf(wrapper)).toBeNull()
    wrapper.unmount()
  })

  it('clears the branch when cwd becomes empty', async () => {
    gitCurrentBranchMock.mockResolvedValue('main')
    const cwd = ref<string | undefined>('/a')
    const wrapper = mount(host(cwd))
    await flush()
    expect(branchOf(wrapper)).toBe('main')
    cwd.value = undefined
    await nextTick()
    await flush()
    expect(branchOf(wrapper)).toBeNull()
    wrapper.unmount()
  })
})
