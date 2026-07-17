export type SameProjectClickAction = 'show-list' | 'collapse'

export function sameProjectClickAction(input: {
  viewingList: boolean
  hasOpenSession: boolean
  hasLiveChat: boolean
}): SameProjectClickAction {
  if (!input.viewingList && (input.hasOpenSession || input.hasLiveChat)) {
    return 'show-list'
  }
  return 'collapse'
}
