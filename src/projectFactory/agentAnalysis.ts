import { extractProjectProfile } from './stackSelector'
import type { AgentAnalysisPayload, RequirementContext, StackRecommendation, StackRecommendationResult } from './types'

function withStatus(
  recommendation: Omit<StackRecommendation, 'status'>,
  status: StackRecommendation['status'],
): StackRecommendation {
  return { ...recommendation, status }
}

export function toStackRecommendationResult(
  analysis: AgentAnalysisPayload,
  context: RequirementContext,
): StackRecommendationResult {
  return {
    profile: extractProjectProfile(context),
    recommended: withStatus(analysis.recommended, 'recommended'),
    alternatives: analysis.alternatives.map(item => withStatus(item, 'alternative')),
    notRecommended: analysis.notRecommended.map(item => withStatus(item, 'not-recommended')),
    provider: analysis.provider,
    assumptions: analysis.assumptions,
    projectName: analysis.projectName,
    projectNameReason: analysis.projectNameReason,
  }
}
