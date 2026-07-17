import { describe, expect, it } from 'vitest'
import { sameProjectClickAction } from '../src/projectSelection'

describe('sameProjectClickAction', () => {
  it('switches to list when a read view is open in front', () => {
    expect(
      sameProjectClickAction({
        viewingList: false,
        hasOpenSession: true,
        hasLiveChat: false,
      }),
    ).toBe('show-list')
  })

  it('switches to list when a live chat is open in front', () => {
    expect(
      sameProjectClickAction({
        viewingList: false,
        hasOpenSession: false,
        hasLiveChat: true,
      }),
    ).toBe('show-list')
  })

  it('collapses only after the list is already showing', () => {
    expect(
      sameProjectClickAction({
        viewingList: true,
        hasOpenSession: true,
        hasLiveChat: false,
      }),
    ).toBe('collapse')
  })

  it('collapses when there is no background view to preserve', () => {
    expect(
      sameProjectClickAction({
        viewingList: false,
        hasOpenSession: false,
        hasLiveChat: false,
      }),
    ).toBe('collapse')
  })
})
