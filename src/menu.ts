// 原生应用菜单 ↔ 前端 的桥。
//
// Rust 侧 build_menu 把所有菜单项的点击都收敛成一个 Tauri event：
//   `menu://action` payload = { id: "open-global-search" | "toggle-sidebar" | ... }
// 这里给一份 id → 函数 的路由表，并暴露 emitSync(group, value) 让前端在
// theme / lang 变化时反推菜单 CheckMenuItem 的勾选态。

import { emit, listen, type UnlistenFn } from '@tauri-apps/api/event'

export type MenuHandler = () => void
export type MenuHandlers = Record<string, MenuHandler>

interface MenuActionPayload {
  id: string
}

/** 注册菜单事件 listener；返回 unlisten 句柄供 app onUnmounted 用。
 *  未在 handlers 里出现的 id 会被静默忽略 —— 譬如 theme:* / lang:* 是用 emitSync 反向同步的，
 *  这里也接到它们当作 setter（让从菜单切换主题 / 语言时能改前端 state）。 */
export async function installMenuRouter(handlers: MenuHandlers): Promise<UnlistenFn> {
  return listen<MenuActionPayload>('menu://action', (e) => {
    const id = e.payload?.id
    if (!id) return
    const fn = handlers[id]
    if (fn) {
      fn()
      return
    }
    // 未匹配的 id 直接吃掉；保留 console.warn 方便排错（菜单 id 改了忘改这里）。
    console.warn('[menu] unknown id:', id)
  })
}

/** 单选 group（theme / lang）的当前值变了 —— 告诉 Rust 菜单更新勾选态。
 *  不强求 await：菜单同步轻量、失败就下次再 sync 也没事。 */
export function emitMenuSync(group: 'theme' | 'lang', value: string): void {
  void emit('menu:sync', { group, value })
}
