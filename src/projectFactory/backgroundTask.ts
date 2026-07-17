export type BackgroundTaskKind = 'analysis' | 'initialization'

export interface BackgroundTaskSummary {
  kind: BackgroundTaskKind
  title: string
  detail: string
  percent: number
  elapsedSeconds: number
}
