// 统计页图表共享配色板。
//
// 设计思路：
//   - 8 色循环，全部为 Tailwind 500 级（亮）/ 400 级（暗）饱和度，刻意避开 red
//     —— 因为页面 brand（--brand-claude = #c2410c）本身就是橙红，brand 当强调色
//     时不会跟柱体撞色。
//   - 横向 bar 的"By Model" / "By Activity" 直接按这个顺序取色；折线图保留
//     brand 做高亮线，bar 用 neutral 软色当背景。
//   - 暗色模式整体提亮一档，避免在深色 surface 上发闷。

export interface ChartPalette {
  /** brand 强调色（折线 / 高亮线）。 */
  brand: string
  /** 文字弱化色（轴 label）。 */
  textMute: string
  /** 边框软色（grid / 柱体填充软底）。 */
  border: string
  /** 网格线颜色（半透明）。 */
  grid: string
  /** 柱体软色（柱状图非强调色 —— 譬如 daily chart 的 call 柱）。 */
  softBar: string
  /** 描边色（圆环 / 圆点的轮廓）。 */
  stroke: string
}

/** 8 色分类调色板（按视觉重要性排序，第一条最醒目）。 */
const CATEGORICAL_LIGHT = [
  '#3b82f6', // blue-500
  '#8b5cf6', // violet-500
  '#10b981', // emerald-500
  '#f59e0b', // amber-500
  '#ec4899', // pink-500
  '#14b8a6', // teal-500
  '#6366f1', // indigo-500
  '#f97316', // orange-500
]

const CATEGORICAL_DARK = [
  '#60a5fa',
  '#a78bfa',
  '#34d399',
  '#fbbf24',
  '#f472b6',
  '#2dd4bf',
  '#818cf8',
  '#fb923c',
]

export function isDark(): boolean {
  return document.documentElement.classList.contains('theme-dark')
}

export function readPalette(): ChartPalette {
  const root = getComputedStyle(document.documentElement)
  const dark = isDark()
  return {
    brand: root.getPropertyValue('--brand').trim() || '#c2410c',
    textMute: root.getPropertyValue('--text-mute').trim() || '#888',
    border: root.getPropertyValue('--border').trim() || '#e5e5e5',
    grid: dark ? 'rgba(255,255,255,0.05)' : 'rgba(0,0,0,0.05)',
    softBar: dark ? 'rgba(148,163,184,0.18)' : 'rgba(148,163,184,0.22)',
    stroke: dark ? root.getPropertyValue('--surface').trim() || '#0a0a0a' : '#ffffff',
  }
}

/** 取分类色，按 i 循环；暗色模式自动切到 light 调色。 */
export function categoricalColor(i: number): string {
  const arr = isDark() ? CATEGORICAL_DARK : CATEGORICAL_LIGHT
  return arr[i % arr.length]
}

/** 取一组分类色（前 n 个，循环），用于一次性给整个数据集染色。 */
export function categoricalColors(n: number): string[] {
  const arr = isDark() ? CATEGORICAL_DARK : CATEGORICAL_LIGHT
  const out: string[] = []
  for (let i = 0; i < n; i++) out.push(arr[i % arr.length])
  return out
}
