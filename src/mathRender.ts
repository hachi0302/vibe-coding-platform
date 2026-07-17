import 'katex/dist/katex.min.css'

let katexPromise: Promise<typeof import('katex').default> | null = null

async function loadKatex() {
  if (!katexPromise) {
    katexPromise = import('katex').then((m) => m.default)
  }
  return katexPromise
}

export function renderAllMath(root: HTMLElement | null): void {
  if (!root) return
  const els = root.querySelectorAll<HTMLElement>(
    '.md-math-inline:not([data-rendered]), .md-math-block:not([data-rendered])',
  )
  if (!els.length) return
  void loadKatex().then((katex) => {
    els.forEach((el) => {
      const expr = el.dataset.math
      if (!expr) return
      const isBlock = el.classList.contains('md-math-block')
      try {
        el.innerHTML = katex.renderToString(expr, {
          displayMode: isBlock,
          throwOnError: false,
          output: 'html',
        })
        el.setAttribute('data-rendered', '1')
      } catch {
        // fallback: keep the raw source visible
      }
    })
  })
}
