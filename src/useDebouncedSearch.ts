// 搜索输入防抖：搜索框打字时不希望每个字符都触发一次重计算 / 重渲染。
//
// 推荐用法（IME 安全）：
//   const { draft, commit, onInput, onCompositionStart, onCompositionEnd } =
//     useDebouncedSearch(sharedSearchRef, 200)
//   <input
//     :value="draft"
//     @input="onInput"
//     @compositionstart="onCompositionStart"
//     @compositionend="onCompositionEnd"
//   />
//   <button @click="commit('')">clear</button>
//
//   - `draft` 立刻跟随 input 显示，但 IME 组合中（中文 / 日文拼音）会暂存到 ref 后
//     不触发 watch —— 等 compositionend 才同步进 `target`，避免半成品输入误触发搜索。
//   - 静止 `delay` ms 后才把值同步到 `target` —— 视图的 filter/computed 才会重跑。
//   - `commit(value)` 取消挂起的定时器并直接写 target，同时把 draft 也对齐。
//   - 外部把 target 重置（切项目 / 切视图）→ watch 把 draft 拉回来。
//
// 也兼容老用法 `v-model="draft"`：v-model 本身就会在 IME 中跳过更新，但 watch 仍会
// 在 compositionend 时跑一次，相当于和 onInput 路径一致。
import { onUnmounted, ref, watch, type Ref } from 'vue'

export function useDebouncedSearch(target: Ref<string>, delay = 180) {
  const draft = ref(target.value)
  // 当前是否处在 IME 组合输入中 —— 组合中跳过同步到 target。
  let composing = false
  let timer = 0

  function commit(value: string) {
    window.clearTimeout(timer)
    timer = 0
    draft.value = value
    target.value = value
  }

  // 打字 → 推延 `delay` 写入共享 ref。组合中不安排同步。
  watch(draft, (v) => {
    if (composing) return
    if (v === target.value) return
    window.clearTimeout(timer)
    timer = window.setTimeout(() => {
      timer = 0
      target.value = v
    }, delay)
  })

  // 外部重置（如切换项目时 resetSessionsToolbar 把 target 设为空）→ draft 跟随
  watch(target, (v) => {
    if (v === draft.value) return
    window.clearTimeout(timer)
    timer = 0
    draft.value = v
  })

  // 直接 @input：覆盖 v-model 的默认行为，给我们手动管控的能力。
  function onInput(e: Event) {
    const v = (e.target as HTMLInputElement).value
    draft.value = v
  }
  function onCompositionStart() {
    composing = true
  }
  function onCompositionEnd(e: Event) {
    composing = false
    const v = (e.target as HTMLInputElement).value
    // 组合结束时同步一次，让 watch 走正常的 debounce 流程。
    if (v !== draft.value) {
      draft.value = v
    } else {
      // value 没变（典型场景）—— 手动 schedule 一次，因为组合期间 watch 没跑。
      window.clearTimeout(timer)
      timer = window.setTimeout(() => {
        timer = 0
        target.value = draft.value
      }, delay)
    }
  }

  onUnmounted(() => window.clearTimeout(timer))

  return { draft, commit, onInput, onCompositionStart, onCompositionEnd }
}
