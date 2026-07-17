// 聊天输入框的「历史回填」：把当前会话里**用户发出的消息**抽成一个可上下翻的列表，
// 供 ChatComposer 的 ↑/↓ 回填（参考 Claude 客户端）。每条保真还原 —— 文本 + 图片 + 文件附件，
// 这样翻出来的就是当时那条 prompt 的完整内容，可直接改了再发。
//
// 纯函数 + 无副作用，便于单测；翻页/光标等有状态逻辑留在组件里。
import type { Block, Msg, ChatImageAttachment, ChatFileAttachment } from './types'
import { commandInputFromMarkup, isCaveatOnlyMsg, parseSystemEvent } from './format'

/** 一条可回填的历史输入。 */
export interface ChatHistoryEntry {
  text: string
  images: ChatImageAttachment[]
  files: ChatFileAttachment[]
}

function baseName(p: string): string {
  return p.replace(/[/\\]+$/, '').split(/[/\\]/).pop() || p
}

/** 从 image 块的 src 还原成可再发送的图片附件；只有 `data:` 内联图能还原（远程 URL 拿不到 base64）。 */
function imageFromSrc(src: string | undefined): ChatImageAttachment | null {
  if (!src || !src.startsWith('data:')) return null
  const comma = src.indexOf(',')
  if (comma < 0) return null
  const mediaType = src.slice(5, comma).split(';')[0] || 'image/png'
  return { dataUrl: src, mediaType, data: src.slice(comma + 1), name: 'image' }
}

function entryFromBlocks(blocks: Block[]): ChatHistoryEntry | null {
  const texts: string[] = []
  const images: ChatImageAttachment[] = []
  const files: ChatFileAttachment[] = []
  for (const b of blocks) {
    if (b.kind === 'text' && b.text) texts.push(b.text)
    else if (b.kind === 'image') {
      const img = imageFromSrc(b.imageSrc)
      if (img) images.push(img)
    } else if (b.kind === 'file' && b.filePath) {
      files.push({ path: b.filePath, name: baseName(b.filePath), isDir: !!b.isDir })
    }
  }
  const joined = texts.join('\n').trim()
  // slash 命令在转录里是一坨 <command-name>/effort</…> 伪 XML —— 收回成用户敲的「/effort」。
  const text = commandInputFromMarkup(joined) ?? joined
  if (!text && !images.length && !files.length) return null
  return { text, images, files }
}

/**
 * 把会话消息抽成历史输入列表（旧 → 新）。只取真正的「Me」气泡内容：
 * role==='user'、非 sidechain、非系统注入记录（metaKind）、非 local-command 提示、
 * 非 rename/中断这类系统事件 —— 与聊天里渲染成用户气泡的那批消息保持一致。
 */
export function buildChatHistory(msgs: Msg[]): ChatHistoryEntry[] {
  const out: ChatHistoryEntry[] = []
  for (const m of msgs) {
    if (m.role !== 'user' || m.sidechain || m.metaKind) continue
    if (isCaveatOnlyMsg(m) || parseSystemEvent(m)) continue
    const entry = entryFromBlocks(m.blocks)
    if (entry) out.push(entry)
  }
  return out
}
