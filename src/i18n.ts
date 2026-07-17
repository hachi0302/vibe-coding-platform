import { lang } from './settings'
import en from './locales/en'
import zh from './locales/zh'
import zhTW from './locales/zh-TW'
import ja from './locales/ja'

const dicts = { en, zh, 'zh-TW': zhTW, ja } as const

/**
 * 翻译函数：根据当前语言取词，模板内调用会自动随 lang 变化重渲染
 * （Vue 模板渲染本身就是响应式 effect，会跟踪对 lang.value 的读取）。
 */
export function t(key: string, vars?: Record<string, string | number>): string {
  const d = (dicts[lang.value] ?? dicts.en) as Record<string, string>
  let s = d[key] ?? (dicts.en as Record<string, string>)[key] ?? key
  if (vars) {
    for (const k in vars) {
      s = s.replace(new RegExp(`\\{${k}\\}`, 'g'), String(vars[k]))
    }
  }
  return s
}
