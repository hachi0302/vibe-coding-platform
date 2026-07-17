import type { ClarifyingQuestion } from './types'

export type ClarificationAnswers = Record<string, string[]>

function sorted(values: string[]) {
  return [...values].sort()
}

export function recommendedAnswersFor(questions: ClarifyingQuestion[]): ClarificationAnswers {
  return Object.fromEntries(questions
    .map(question => [question.id, question.options
      .filter(option => option.recommended)
      .map(option => option.value)] as const)
    .filter(([, values]) => values.length > 0))
}

export function answersMatchRecommendations(
  questions: ClarifyingQuestion[],
  answers: ClarificationAnswers,
) {
  const recommended = recommendedAnswersFor(questions)
  return questions.every(question => {
    const selected = sorted(answers[question.id] ?? [])
    const expected = sorted(recommended[question.id] ?? [])
    return selected.length === expected.length
      && selected.every((value, index) => value === expected[index])
  })
}
