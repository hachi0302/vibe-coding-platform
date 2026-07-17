import type { Agent } from './types'

export type TerminalProcessState = 'spawning' | 'alive' | 'exited' | 'error'
export type TerminalTurnState = 'idle' | 'working' | 'blocked' | 'review' | 'error' | 'unknown'
export type TerminalTurnSignalSource =
  | 'session-jsonl'
  | 'session-live-tail'
  | 'pty-input'
  | 'pty-exit'
  | 'hook'

export type TerminalTurnEventState = 'started' | 'completed' | 'blocked' | 'failed'
export type TabStatusKind =
  | 'working'
  | 'done'
  | 'blocked'
  | 'error'
  | 'exited'
  | 'unknown'
  | 'none'

type StatusTab = {
  processState: TerminalProcessState
  status: 'spawning' | 'running' | 'exited' | 'error'
  turnState: TerminalTurnState
  turnStateSource: TerminalTurnSignalSource | null
  turnStateUpdatedAt: number
  agent: Agent
  sessionPath: string
}

const pendingTurnStates = new Map<
  string,
  { state: TerminalTurnEventState; source: TerminalTurnSignalSource; updatedAt: number }
>()

function turnStateKey(agent: Agent, sessionPath: string) {
  return `${agent}\0${sessionPath}`
}

function completedState(isActive: boolean): TerminalTurnState {
  return isActive ? 'idle' : 'review'
}

export function isSlashCommandInput(line: string): boolean {
  return line.trimStart().startsWith('/')
}

export function shouldTerminalInputStartTurn(agent: Agent, line: string): boolean {
  void agent
  if (isSlashCommandInput(line)) return false
  return line.trim().length > 0
}

function stripTerminalControlSequences(data: string): string {
  let out = ''
  for (let i = 0; i < data.length; i++) {
    const ch = data[i]
    if (ch !== '\x1b' && ch !== '\x9b') {
      out += ch
      continue
    }

    if (ch === '\x9b') {
      i += 1
      while (i < data.length && !/[\x40-\x7e]/.test(data[i])) i += 1
      continue
    }

    const next = data[i + 1]
    if (next === '[') {
      i += 2
      while (i < data.length && !/[\x40-\x7e]/.test(data[i])) i += 1
    } else if (next === ']') {
      i += 2
      while (i < data.length) {
        if (data[i] === '\x07') break
        if (data[i] === '\x1b' && data[i + 1] === '\\') {
          i += 1
          break
        }
        i += 1
      }
    } else if (next && /[PX^_]/.test(next)) {
      i += 2
      while (i < data.length) {
        if (data[i] === '\x1b' && data[i + 1] === '\\') {
          i += 1
          break
        }
        i += 1
      }
    } else if (next) {
      i += 1
    }
  }
  return out
}

export function applyTerminalInputLineState(
  current: string,
  data: string,
): { nextLine: string; submittedLines: string[] } {
  let line = current
  const submittedLines: string[] = []
  for (const ch of stripTerminalControlSequences(data)) {
    if (ch === '\r' || ch === '\n') {
      submittedLines.push(line)
      line = ''
    } else if (ch === '\b' || ch === '\x7f') {
      line = line.slice(0, -1)
    } else if (ch === '\x15') {
      line = ''
    } else if (ch >= ' ') {
      line += ch
    }
  }
  return { nextLine: line, submittedLines }
}

export function statusKind(tab: StatusTab): TabStatusKind {
  if (tab.turnState === 'error' || tab.processState === 'error') return 'error'
  if (tab.processState === 'exited') return 'exited'
  if (tab.turnState === 'blocked') return 'blocked'
  if (tab.processState === 'spawning' || tab.turnState === 'working') return 'working'
  if (tab.turnState === 'review') return 'done'
  if (tab.turnState === 'idle') return 'none'
  return 'unknown'
}

export function setProcessState(tab: StatusTab, state: TerminalProcessState) {
  tab.processState = state
  tab.status = state === 'alive' ? 'running' : state
}

export function setTurnState(
  tab: StatusTab,
  state: TerminalTurnState,
  source: TerminalTurnSignalSource,
  updatedAt = Date.now(),
) {
  tab.turnState = state
  tab.turnStateSource = source
  tab.turnStateUpdatedAt = updatedAt
}

export function applyTurnSignal(
  tab: StatusTab,
  state: TerminalTurnEventState,
  source: TerminalTurnSignalSource,
  isActive: boolean,
) {
  if (state === 'started') {
    if (tab.turnState !== 'blocked' && tab.turnState !== 'error') {
      setTurnState(tab, 'working', source)
    }
    return
  }
  if (state === 'completed') {
    setTurnState(tab, completedState(isActive), source)
    return
  }
  if (state === 'blocked') {
    setTurnState(tab, 'blocked', source)
    return
  }
  setTurnState(tab, 'error', source)
}

export function markSessionActivity(tab: StatusTab) {
  void tab
}

export function clearLocalWorkingTurn(tab: StatusTab, isActive: boolean) {
  if (tab.turnState !== 'working') return
  setTurnState(tab, completedState(isActive), 'pty-input')
}

export function rememberPendingTurnState(
  agent: Agent,
  sessionPath: string,
  state: TerminalTurnEventState,
  source: TerminalTurnSignalSource,
) {
  if (!sessionPath) return
  pendingTurnStates.set(turnStateKey(agent, sessionPath), {
    state,
    source,
    updatedAt: Date.now(),
  })
  if (pendingTurnStates.size > 200) {
    const first = pendingTurnStates.keys().next().value
    if (first) pendingTurnStates.delete(first)
  }
}

export function applyPendingTurnState(tab: StatusTab, isActive: boolean) {
  if (!tab.sessionPath) return
  const key = turnStateKey(tab.agent, tab.sessionPath)
  const pending = pendingTurnStates.get(key)
  if (!pending) return
  applyTurnSignal(tab, pending.state, pending.source, isActive)
  tab.turnStateUpdatedAt = pending.updatedAt
  pendingTurnStates.delete(key)
}
