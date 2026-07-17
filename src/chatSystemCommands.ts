// 客户端「系统指令」—— 注入到 GUI chat `/` 浮层「System」分组的内置命令。
//
// 它们**不来自**磁盘扫描（headless 下 CLI 的内置斜杠命令不在 agentChatSlashCommands 里），
// 而是前端硬编码一份，让用户能在浮层里发现 / 补全。提交行为分两类：
//   · 客户端拦截（不发给 agent）：/model /export /rename /clear /fork /btw —— 见 chatSlashActions.ts；
//   · 透传给 agent（照常发送）：/compact /context /reload-skills。
//
// 描述走 i18n（随语言切换刷新），故对外是个函数而非常量。

import type { Agent, SlashCommand } from './types'
import { t } from './i18n'

interface SystemCmdDef {
  name: string
  /** 仅这些 agent 显示；缺省 = 全部 agent。 */
  agents?: Agent[]
}

// 顺序即浮层内的展示顺序（System 组排在扫描出的 Commands / Skills 之前）。
const SYSTEM_COMMANDS: SystemCmdDef[] = [
  { name: 'model' },
  { name: 'export' },
  { name: 'rename' },
  { name: 'clear' },
  { name: 'fork', agents: ['claude'] },
  // { name: 'fork', agents: ['codex'] }, // TODO: Codex fork 暂未对接
  { name: 'compact' },
  { name: 'goal' },
  { name: 'plan' },
  { name: 'review', agents: ['codex'] },
  { name: 'archive', agents: ['codex'] },
  { name: 'btw', agents: ['claude'] },
  { name: 'context', agents: ['claude'] },
  { name: 'reload-skills', agents: ['claude'] },
]

/** 当前 agent 可用的系统指令（已套 i18n 描述），形态与扫描出的 SlashCommand 一致。 */
export function systemSlashCommands(agent: Agent): SlashCommand[] {
  return SYSTEM_COMMANDS.filter((c) => !c.agents || c.agents.includes(agent)).map((c) => ({
    name: c.name,
    title: `/${c.name}`,
    description: t(`chat.composer.system.${c.name}`),
    kind: 'system',
    origin: 'system',
  }))
}
