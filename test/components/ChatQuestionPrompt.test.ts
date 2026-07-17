import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ChatQuestionPrompt from '../../src/components/ChatQuestionPrompt.vue'
import type { ChatQuestionItem, ChatQuestionRequest } from '../../src/types'
import type { QuestionSelection } from '../../src/chatQuestion'

const single: ChatQuestionItem = {
  question: 'Pick a language',
  header: 'Language',
  options: [
    { label: 'TypeScript', description: 'typed JS' },
    { label: 'Rust', description: 'systems' },
  ],
}
const multi: ChatQuestionItem = {
  question: 'Which have you used?',
  multiSelect: true,
  options: [{ label: 'Python' }, { label: 'Go' }, { label: 'Rust' }],
}

const req = (questions: ChatQuestionItem[]): ChatQuestionRequest => ({ requestId: 'r1', questions })
const mountPrompt = (request: ChatQuestionRequest) => mount(ChatQuestionPrompt, { props: { request } })
const submitDisabled = (w: ReturnType<typeof mountPrompt>) =>
  (w.find('.q-submit').element as HTMLButtonElement).disabled
const firstSubmit = (w: ReturnType<typeof mountPrompt>) =>
  w.emitted('submit')![0][0] as QuestionSelection[]

describe('ChatQuestionPrompt', () => {
  it('renders the title and question text', () => {
    const w = mountPrompt(req([single]))
    expect(w.find('.q-title').text().length).toBeGreaterThan(0)
    expect(w.find('.q-text').text()).toBe('Pick a language')
  })

  it('keeps submit disabled until a selection is made, then emits the picked label', async () => {
    const w = mountPrompt(req([single]))
    expect(submitDisabled(w)).toBe(true)
    await w.findAll('button.q-opt')[1].trigger('click') // Rust
    expect(submitDisabled(w)).toBe(false)
    await w.find('.q-submit').trigger('click')
    expect(firstSubmit(w)).toEqual([{ labels: ['Rust'], otherText: undefined }])
  })

  it('single-select is exclusive — a second pick replaces the first', async () => {
    const w = mountPrompt(req([single]))
    const opts = w.findAll('button.q-opt')
    await opts[0].trigger('click') // TypeScript
    await opts[1].trigger('click') // Rust
    await w.find('.q-submit').trigger('click')
    expect(firstSubmit(w)).toEqual([{ labels: ['Rust'], otherText: undefined }])
  })

  it('multi-select accumulates picks', async () => {
    const w = mountPrompt(req([multi]))
    const opts = w.findAll('button.q-opt')
    await opts[0].trigger('click') // Python
    await opts[1].trigger('click') // Go
    await w.find('.q-submit').trigger('click')
    expect(firstSubmit(w)).toEqual([{ labels: ['Python', 'Go'], otherText: undefined }])
  })

  it('auto-selects Other when typing in its always-visible input', async () => {
    const w = mountPrompt(req([single]))
    const input = w.find('.q-other-input')
    expect(input.exists()).toBe(true) // input is always visible, no toggle needed
    await input.setValue('Zig')
    await w.find('.q-submit').trigger('click')
    expect(firstSubmit(w)).toEqual([{ labels: [], otherText: 'Zig' }])
  })

  it('selecting Other via its toggle is exclusive for single-select', async () => {
    const w = mountPrompt(req([single]))
    await w.findAll('button.q-opt')[0].trigger('click') // TypeScript
    await w.find('.q-other-toggle').trigger('click') // Other clears the structured pick
    await w.find('.q-other-input').setValue('Zig')
    await w.find('.q-submit').trigger('click')
    expect(firstSubmit(w)).toEqual([{ labels: [], otherText: 'Zig' }])
  })

  it('shows one question at a time and advances with Next', async () => {
    const w = mountPrompt(req([single, multi]))
    // only the first question is rendered (no all-on-one-page stacking)
    expect(w.findAll('.q-item')).toHaveLength(1)
    expect(w.find('.q-text').text()).toBe('Pick a language')
    // not the last question → a Next button (not Submit) is shown, disabled until answered
    expect(w.find('.q-submit').exists()).toBe(false)
    expect((w.find('.q-next').element as HTMLButtonElement).disabled).toBe(true)
    await w.findAll('button.q-opt')[1].trigger('click') // Rust
    expect((w.find('.q-next').element as HTMLButtonElement).disabled).toBe(false)
    await w.find('.q-next').trigger('click')
    // second (last) question → Submit appears, Next is gone
    expect(w.find('.q-text').text()).toBe('Which have you used?')
    expect(w.find('.q-next').exists()).toBe(false)
    await w.findAll('button.q-opt')[0].trigger('click') // Python
    await w.find('.q-submit').trigger('click')
    expect(firstSubmit(w)).toEqual([
      { labels: ['Rust'], otherText: undefined },
      { labels: ['Python'], otherText: undefined },
    ])
  })

  it('Back returns to the previous question with its answer intact', async () => {
    const w = mountPrompt(req([single, multi]))
    expect(w.find('.q-back').exists()).toBe(false) // no Back on the first question
    await w.findAll('button.q-opt')[0].trigger('click') // TypeScript
    await w.find('.q-next').trigger('click')
    expect(w.find('.q-back').exists()).toBe(true)
    await w.find('.q-back').trigger('click')
    expect(w.find('.q-text').text()).toBe('Pick a language')
    // the earlier pick is preserved → Next is immediately enabled again
    expect((w.find('.q-next').element as HTMLButtonElement).disabled).toBe(false)
  })

  it('emits cancel when the cancel button is clicked', async () => {
    const w = mountPrompt(req([single]))
    await w.find('.q-cancel').trigger('click')
    expect(w.emitted('cancel')).toBeTruthy()
  })

  it('shows a preview pane for single-select-with-preview and follows hover', async () => {
    const sp: ChatQuestionItem = {
      question: 'Pick a layout',
      options: [
        { label: 'A', preview: 'AAA' },
        { label: 'B', preview: 'BBB' },
      ],
    }
    const w = mountPrompt(req([sp]))
    expect(w.find('.q-preview').exists()).toBe(true)
    expect(w.find('.q-preview pre').text()).toBe('AAA') // defaults to first previewable option
    await w.findAll('button.q-opt')[1].trigger('mouseenter')
    expect(w.find('.q-preview pre').text()).toBe('BBB')
  })

  it('omits the preview pane for multi-select even when options carry previews', () => {
    const mp: ChatQuestionItem = {
      question: 'q',
      multiSelect: true,
      options: [{ label: 'A', preview: 'X' }],
    }
    const w = mountPrompt(req([mp]))
    expect(w.find('.q-preview').exists()).toBe(false)
  })
})
