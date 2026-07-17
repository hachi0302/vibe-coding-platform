import { describe, expect, it } from 'vitest'
import { buildChatHistory } from '../src/chatInputHistory'
import type { Block, Msg } from '../src/types'

const txt = (text: string): Block => ({ kind: 'text', text, isError: false })
const user = (blocks: Block[], over: Partial<Msg> = {}): Msg => ({
  role: 'user',
  sidechain: false,
  blocks,
  ...over,
})
const assistant = (text: string): Msg => ({ role: 'assistant', sidechain: false, blocks: [txt(text)] })

describe('buildChatHistory', () => {
  it('extracts user prompts in order, oldest → newest', () => {
    const h = buildChatHistory([user([txt('first')]), assistant('hi'), user([txt('second')])])
    expect(h.map((e) => e.text)).toEqual(['first', 'second'])
  })

  it('skips assistant, sidechain and system-injected (metaKind) messages', () => {
    const h = buildChatHistory([
      user([txt('keep')]),
      assistant('nope'),
      user([txt('sub')], { sidechain: true }),
      user([txt('meta')], { metaKind: 'compact' }),
    ])
    expect(h.map((e) => e.text)).toEqual(['keep'])
  })

  it('filters the "[Request interrupted by user]" marker', () => {
    const h = buildChatHistory([user([txt('real')]), user([txt('[Request interrupted by user]')])])
    expect(h.map((e) => e.text)).toEqual(['real'])
  })

  it('joins multiple text blocks and trims', () => {
    const h = buildChatHistory([user([txt('  line one'), txt('line two  ')])])
    expect(h[0].text).toBe('line one\nline two')
  })

  it('restores inline (data:) images as re-sendable attachments', () => {
    const dataUrl = 'data:image/png;base64,AAAA'
    const h = buildChatHistory([user([{ kind: 'image', isError: false, imageSrc: dataUrl }, txt('look')])])
    expect(h).toHaveLength(1)
    expect(h[0].text).toBe('look')
    expect(h[0].images).toEqual([{ dataUrl, mediaType: 'image/png', data: 'AAAA', name: 'image' }])
  })

  it('drops non-data (remote URL) images it cannot re-send', () => {
    const h = buildChatHistory([user([{ kind: 'image', isError: false, imageSrc: 'https://x/y.png' }, txt('hi')])])
    expect(h[0].images).toEqual([])
    expect(h[0].text).toBe('hi')
  })

  it('restores file attachments with basename + isDir', () => {
    const h = buildChatHistory([
      user([
        { kind: 'file', isError: false, filePath: '/work/proj/src/a.ts', isDir: false },
        { kind: 'file', isError: false, filePath: '/work/proj/docs', isDir: true },
      ]),
    ])
    expect(h[0].files).toEqual([
      { path: '/work/proj/src/a.ts', name: 'a.ts', isDir: false },
      { path: '/work/proj/docs', name: 'docs', isDir: true },
    ])
  })

  it('keeps an image-only message (no text) as a recallable entry', () => {
    const h = buildChatHistory([user([{ kind: 'image', isError: false, imageSrc: 'data:image/png;base64,ZZ' }])])
    expect(h).toHaveLength(1)
    expect(h[0].text).toBe('')
    expect(h[0].images).toHaveLength(1)
  })

  it('drops fully-empty user messages', () => {
    const h = buildChatHistory([user([txt('   ')]), user([])])
    expect(h).toEqual([])
  })

  it('reduces slash-command pseudo-XML back to the typed command', () => {
    const markup = '<command-name>/effort</command-name>\n<command-message>effort</command-message>\n<command-args></command-args>'
    const h = buildChatHistory([user([txt(markup)])])
    expect(h.map((e) => e.text)).toEqual(['/effort'])
  })

  it('keeps slash-command args when reducing the markup', () => {
    const markup = '<command-name>/review</command-name>\n<command-message>review</command-message>\n<command-args>src/foo.ts</command-args>'
    const h = buildChatHistory([user([txt(markup)])])
    expect(h.map((e) => e.text)).toEqual(['/review src/foo.ts'])
  })

  it('skips local-command caveat plumbing messages', () => {
    const caveat = '<local-command-caveat>Caveat: tool output below.</local-command-caveat>'
    const h = buildChatHistory([user([txt(caveat)]), user([txt('real')])])
    expect(h.map((e) => e.text)).toEqual(['real'])
  })
})
