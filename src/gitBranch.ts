import { ref, watch } from 'vue'
import { gitCurrentBranch } from './api'

/**
 * 给定（响应式）会话 cwd，返回「当前 git 分支名」的响应式 ref。
 *
 * 无 cwd / 非 git 仓库 / 读不到时为 `null`（调用方据此 `v-if` 不渲染分支块）。
 * cwd 变化（切会话）时自动重取。ChatView 头部与 ChatComposer 底栏共用这一份逻辑，
 * 避免各写一遍取分支 + 容错。
 *
 * @param getCwd 取当前 cwd 的 getter —— 传 getter 而非 ref，既能接 `computed`/`ref`，
 *               也能接 `() => props.cwd || props.session.cwd`。
 */
export function useGitBranch(getCwd: () => string | undefined) {
  const branch = ref<string | null>(null)
  async function refresh() {
    const cwd = getCwd()
    if (!cwd) {
      branch.value = null
      return
    }
    try {
      branch.value = await gitCurrentBranch(cwd)
    } catch {
      branch.value = null
    }
  }
  watch(getCwd, refresh, { immediate: true })
  return branch
}
