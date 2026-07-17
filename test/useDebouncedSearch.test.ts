import { describe, expect, it } from 'vitest'
import { defineComponent, h, ref } from 'vue'
import { mount } from '@vue/test-utils'
import { useDebouncedSearch } from '../src/useDebouncedSearch'

// 用一个最小的宿主组件来挂 useDebouncedSearch（composable 需要 onUnmounted 在
// 组件作用域里调用）。组件提供一个 input 让我们打字，和一个清空按钮。
// 这里直接走 composable 暴露的 onInput / onCompositionStart / onCompositionEnd，
// 方便测试 IME 路径。
function host(target = ref(''), delay = 80) {
  return defineComponent({
    setup() {
      const helpers = useDebouncedSearch(target, delay)
      return { ...helpers, target }
    },
    render() {
      return h('div', [
        h('input', {
          class: 'box',
          value: this.draft,
          onInput: this.onInput,
          onCompositionstart: this.onCompositionStart,
          onCompositionend: this.onCompositionEnd,
        }),
        h('button', { class: 'clear', onClick: () => this.commit('') }, 'x'),
      ])
    },
  })
}

const tick = (ms: number) => new Promise((r) => setTimeout(r, ms))

describe('useDebouncedSearch', () => {
  it('does not write to the shared ref until the delay elapses', async () => {
    const target = ref('')
    const wrapper = mount(host(target, 60))
    await wrapper.find('.box').setValue('a')
    expect(target.value).toBe('') // 还没过防抖
    await tick(80)
    expect(target.value).toBe('a')
    wrapper.unmount()
  })

  it('collapses several rapid edits into the final value', async () => {
    const target = ref('')
    const wrapper = mount(host(target, 60))
    const box = wrapper.find('.box')
    await box.setValue('a')
    await box.setValue('ab')
    await box.setValue('abc')
    await tick(80)
    expect(target.value).toBe('abc')
    wrapper.unmount()
  })

  it('commit() bypasses the debounce and clears the pending timer', async () => {
    const target = ref('hello')
    const wrapper = mount(host(target, 60))
    await wrapper.find('.box').setValue('world')
    await wrapper.find('.clear').trigger('click')
    expect(target.value).toBe('') // 立刻生效，没等防抖
    // 再等一会儿确保挂起的定时器没在事后把 'world' 写回去
    await tick(80)
    expect(target.value).toBe('')
    wrapper.unmount()
  })

  it('external reset of the target pulls the draft back in sync', async () => {
    const target = ref('foo')
    const wrapper = mount(host(target, 60))
    target.value = ''
    await tick(0)
    expect((wrapper.vm as unknown as { draft: string }).draft).toBe('')
    wrapper.unmount()
  })

  it('does not commit while IME composition is active', async () => {
    const target = ref('')
    const wrapper = mount(host(target, 60))
    const box = wrapper.find('.box')
    // 模拟 IME 序列：compositionstart → 多次 input（半成品中文拼音）→ compositionend
    await box.trigger('compositionstart')
    ;(box.element as HTMLInputElement).value = 'n'
    await box.trigger('input')
    ;(box.element as HTMLInputElement).value = 'ni'
    await box.trigger('input')
    // 给足够长的时间 —— 组合中绝不应该写到 target
    await tick(100)
    expect(target.value).toBe('')
    // compositionend 后：写一次最终值，再过防抖才生效
    ;(box.element as HTMLInputElement).value = '你'
    await box.trigger('compositionend')
    await tick(80)
    expect(target.value).toBe('你')
    wrapper.unmount()
  })
})
