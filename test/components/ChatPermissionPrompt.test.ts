import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ChatPermissionPrompt from '../../src/components/ChatPermissionPrompt.vue'
import { vTooltip } from '../../src/tooltip'
import type { ChatPermissionRequest } from '../../src/types'

const req = (over: Partial<ChatPermissionRequest> = {}): ChatPermissionRequest => ({
  requestId: 'r1',
  toolName: 'Bash',
  input: { command: 'rm -rf build' },
  ...over,
})

const mountPrompt = (request: ChatPermissionRequest) =>
  mount(ChatPermissionPrompt, {
    props: { request },
    global: { directives: { tooltip: vTooltip } },
  })

describe('ChatPermissionPrompt', () => {
  it('shows the tool name in the title and the command preview', () => {
    const w = mountPrompt(req())
    expect(w.find('.perm-title').text()).toContain('Bash')
    expect(w.find('.perm-cmd').text()).toBe('rm -rf build')
  })

  it('renders the CLI description when present, omits the preview when absent', () => {
    const w = mountPrompt(req({ toolName: 'Foo', input: {}, description: 'Does a thing' }))
    expect(w.find('.perm-cmd').exists()).toBe(false)
    expect(w.find('.perm-desc').text()).toBe('Does a thing')
  })

  it('emits allow-once and deny for the always-present buttons', async () => {
    const w = mountPrompt(req())
    await w.find('.perm-allow').trigger('click')
    await w.find('.perm-deny').trigger('click')
    expect(w.emitted('choose')).toEqual([['allow-once'], ['deny']])
  })

  it('hides the always-allow button when the CLI offers no rule suggestions', () => {
    const w = mountPrompt(req())
    expect(w.find('.perm-always').exists()).toBe(false)
  })

  it('shows and emits always-allow only when suggestions are offered', async () => {
    const w = mountPrompt(
      req({ permissionSuggestions: [{ type: 'addRules', rules: [], behavior: 'allow', destination: 'localSettings' }] }),
    )
    const always = w.find('.perm-always')
    expect(always.exists()).toBe(true)
    await always.trigger('click')
    expect(w.emitted('choose')).toEqual([['always-allow']])
  })

  it('renders Codex approval details without Claude-specific title text', () => {
    const w = mount(ChatPermissionPrompt, {
      props: {
        agent: 'codex',
        request: req({
          toolName: 'shell',
          input: {
            command: 'rtk rm -rf ai_completion',
            environment: 'local',
            reason: 'Needs elevated permissions',
          },
        }),
      },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(w.find('.perm-title').text()).toBe('Would you like to run the following command?')
    expect(w.find('.perm-cmd').text()).toBe('rtk rm -rf ai_completion')
    expect(w.text()).toContain('Environment')
    expect(w.text()).toContain('local')
    expect(w.text()).toContain('Reason')
    expect(w.text()).toContain('Needs elevated permissions')
  })
})
