// 一次性「飞线」动画：一个小球从 from 沿弧线飞向 to。
// 删除时飞向顶栏回收站、恢复时飞回侧边栏项目列表，都共用这一个函数。
//
// 弧线用嵌套元素分轴合成：外层动 X（强 ease-in，先慢后快），中层动 Y
// （ease-out，先快后慢）。早期 Y 先动 → 先升起；后期 X 才动 → 再扫向目标，
// 还原「先升起、再扫入目标」的轨迹。chip 自身负责缩放 + 淡出。

const ICON = {
  trash: `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M3 6h18"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>`,
  restore: `<svg viewBox="0 0 24 24" width="16" height="16" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><rect width="20" height="5" x="2" y="3" rx="1"/><path d="M4 8v11a2 2 0 0 0 2 2h2"/><path d="M20 8v11a2 2 0 0 1-2 2h-2"/><path d="m9 15 3-3 3 3"/><path d="M12 12v9"/></svg>`,
}

export type FlyVariant = keyof typeof ICON

/** 起点 / 落点：可传 DOM 元素，也可传已捕获的 DOMRect（元素在动画开始前
 *  就会被移除时用 rect）。Vue 模板 ref 直接传 `someRef.value` 即可。 */
export type FlyAnchor = HTMLElement | DOMRect | null | undefined

const CHIP = 34
const DUR = 540

function rectOf(a: FlyAnchor): DOMRect | null {
  if (!a) return null
  return a instanceof HTMLElement ? a.getBoundingClientRect() : a
}

/** 触发一次飞线动画。起点或落点缺失则静默跳过 —— 动画失败不应影响主操作本身。
 *  落点是元素时，动画结束会给它加一次性 `fly-receive` 类做「接收」反馈。 */
export function fly(opts: { from: FlyAnchor; to: FlyAnchor; variant: FlyVariant }) {
  const src = rectOf(opts.from)
  const dst = rectOf(opts.to)
  if (!src || !dst) return

  const x0 = src.left + src.width / 2
  const y0 = src.top + src.height / 2
  const x1 = dst.left + dst.width / 2
  const y1 = dst.top + dst.height / 2
  const dx = x1 - x0
  const dy = y1 - y0

  const outer = document.createElement('div')
  outer.className = 'fly-orb'
  // outer 左上角 = chip 中心对齐到起点
  outer.style.left = `${x0 - CHIP / 2}px`
  outer.style.top = `${y0 - CHIP / 2}px`

  const mid = document.createElement('div')
  const chip = document.createElement('div')
  chip.className = `fly-chip fly-chip-${opts.variant}`
  chip.innerHTML = ICON[opts.variant]
  mid.appendChild(chip)
  outer.appendChild(mid)
  document.body.appendChild(outer)

  outer.animate(
    [{ transform: 'translateX(0)' }, { transform: `translateX(${dx}px)` }],
    { duration: DUR, easing: 'cubic-bezier(0.75, 0, 0.85, 1)', fill: 'forwards' },
  )
  mid.animate(
    [{ transform: 'translateY(0)' }, { transform: `translateY(${dy}px)` }],
    { duration: DUR, easing: 'cubic-bezier(0.2, 0.7, 0.35, 1)', fill: 'forwards' },
  )
  const anim = chip.animate(
    [
      { transform: 'scale(1)', opacity: 1 },
      { transform: 'scale(0.82)', opacity: 1, offset: 0.55 },
      { transform: 'scale(0.14)', opacity: 0 },
    ],
    { duration: DUR, easing: 'ease-in', fill: 'forwards' },
  )

  const landing = opts.to instanceof HTMLElement ? opts.to : null
  const done = () => {
    outer.remove()
    if (landing) {
      landing.classList.add('fly-receive')
      setTimeout(() => landing.classList.remove('fly-receive'), 320)
    }
  }
  anim.finished.then(done).catch(done)
}
