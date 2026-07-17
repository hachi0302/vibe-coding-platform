// 模型向用户提的结构化选择题（Claude `AskUserQuestion`，走 `--permission-prompt-tool stdio`
// 的 `can_use_tool` 控制协议）的纯逻辑：把用户在卡片上的选择构造成 CLI 控制协议的 decision，
// 以及一些判定（是否多选 / 是否并排预览 / 是否答完）。不依赖 Vue / Tauri，便于单测；
// ChatView / ChatQuestionPrompt 与 chatSessions 共用。
//
// 答案编码（经实测确认，见会话记录）：
//   answers[问题文本] = "选项label"        —— 单选
//   answers[问题文本] = "labelA, labelB"   —— 多选（逗号+空格拼接）
//   Other 自填文本也并入 answers[问题文本]（而非顶层 `response`）—— 顶层 `response` 会在
//     tool_result 文案里「覆盖」掉结构化答案，多问题场景会吞掉别的答案，故统一走 answers。

import type { ChatQuestionItem, ChatQuestionRequest } from './types'

/** 单条提问的用户选择：选中的结构化选项 label（单选 0–1 个、多选任意个）+ 可选的 Other 自填。 */
export interface QuestionSelection {
  /** 选中的结构化选项 `label`。 */
  labels: string[]
  /** 「Other」自填文本（选了 Other 才有；与 `labels` 一起逗号拼接成最终答案）。 */
  otherText?: string
}

/** 把一条选择折叠成最终答案串：结构化 label + Other 文本，去空后以 `, ` 拼接。 */
function answerText(sel: QuestionSelection): string {
  const parts = sel.labels.map((s) => s.trim()).filter(Boolean)
  const other = sel.otherText?.trim()
  if (other) parts.push(other)
  return parts.join(', ')
}

/** 该问题是否已作答（有任何结构化选项或非空 Other 文本）。submit 据此逐题门控。 */
export function questionAnswered(sel: QuestionSelection | undefined): boolean {
  return !!sel && answerText(sel).length > 0
}

/** 是否每条提问都已作答 —— 全部答完才允许提交。 */
export function allQuestionsAnswered(
  req: ChatQuestionRequest,
  selections: QuestionSelection[],
): boolean {
  return req.questions.every((_, i) => questionAnswered(selections[i]))
}

/** 该题是否走「并排预览」布局 —— 仅单选题、且至少一个选项带非空 `preview`。 */
export function questionHasPreview(q: ChatQuestionItem): boolean {
  return (
    !q.multiSelect &&
    q.options.some((o) => typeof o.preview === 'string' && o.preview.trim().length > 0)
  )
}

/**
 * 把用户的选择构造成 CLI 控制协议的 `decision`（作答）：
 *   `{behavior:'allow', updatedInput:{questions:<原样带回>, answers:{<问题文本>:<答案串>}}}`
 * 没作答的问题不进 `answers`（CLI 会当作「未回答该题」）。
 */
export function buildQuestionDecision(
  req: ChatQuestionRequest,
  selections: QuestionSelection[],
): Record<string, unknown> {
  const answers: Record<string, string> = {}
  req.questions.forEach((q, i) => {
    const text = answerText(selections[i] ?? { labels: [] })
    if (text) answers[q.question] = text
  })
  return {
    behavior: 'allow',
    updatedInput: { questions: req.questions, answers },
  }
}

/**
 * 取消作答的 `decision`：`{behavior:'deny', message, interrupt:false}` —— 把「用户没回答」
 * 反馈给模型，但不打断本轮（模型可换个方式继续）。
 */
export function buildQuestionCancelDecision(): Record<string, unknown> {
  return {
    behavior: 'deny',
    message: 'The user declined to answer the question.',
    interrupt: false,
  }
}
