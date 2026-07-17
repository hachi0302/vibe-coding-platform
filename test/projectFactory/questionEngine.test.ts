import { describe, expect, it } from 'vitest'
import { answersMatchRecommendations, recommendedAnswersFor } from '../../src/projectFactory/questionEngine'

const questions = [
  {
    id: 'merchant-operations',
    label: '是否需要商家运营后台？',
    selectionMode: 'single' as const,
    options: [
      { value: 'required', label: '需要独立商家后台', recommended: true },
      { value: 'not-required', label: '第一期不需要' },
    ],
  },
  {
    id: 'payment-scope',
    label: '第一期需要哪些支付方式？',
    selectionMode: 'multiple' as const,
    options: [
      { value: 'wechat', label: '微信支付', recommended: true },
      { value: 'offline', label: '到店支付' },
    ],
  },
]

describe('project factory dynamic question helpers', () => {
  it('uses the AI-recommended choices as defaults', () => {
    expect(recommendedAnswersFor(questions)).toEqual({
      'merchant-operations': ['required'],
      'payment-scope': ['wechat'],
    })
  })

  it('does not request another analysis when the user keeps recommended answers', () => {
    const answers = recommendedAnswersFor(questions)

    expect(answersMatchRecommendations(questions, answers)).toBe(true)
  })

  it('requests refinement only after the user changes an AI-recommended answer', () => {
    expect(answersMatchRecommendations(questions, {
      'merchant-operations': ['not-required'],
      'payment-scope': ['wechat', 'offline'],
    })).toBe(false)
  })
})
