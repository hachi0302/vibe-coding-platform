// Mermaid 渲染辅助 —— 给 ChatView 在 v-html 注入完之后扫一遍 DOM 替换占位符。
//
// 渲染管线：
//   1. `renderText` 看到 ```mermaid``` 围栏，发 `<div class="md-mermaid" data-source="...">`
//      占位符，里面塞一份 escaped 源码（fallback / 渲染失败时露出来 + 主题切换时复用）。
//   2. ChatView 在 onMounted / messages 变化的 nextTick 调 `renderAllMermaid(root)`。
//   3. 这里 dynamic-import mermaid，给每个 .md-mermaid 调 mermaid.render() 替换 innerHTML。
//
// 为什么 dynamic-import：mermaid 压缩后约 600KB，没用到 mermaid 的会话不该把它拖进
// 主 bundle。第一次出现 mermaid 块时才 fetch / 解析。
//
// 主题：跟 settings.ts 里 `theme` 联动。light → 'default'，dark → 'dark'。
// 切换主题时需要重渲染所有 .md-mermaid（mermaid 不支持运行时改主题，要拿 source 重画）。

import { theme } from './settings'

let mermaidPromise: Promise<typeof import('mermaid').default> | null = null
let currentTheme: 'light' | 'dark' | null = null
let renderSeq = 0

function effectiveTheme(): 'light' | 'dark' {
  if (theme.value === 'dark') return 'dark'
  if (theme.value === 'light') return 'light'
  // system → 看 prefers-color-scheme
  return window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light'
}

async function loadMermaid() {
  if (!mermaidPromise) {
    mermaidPromise = import('mermaid').then((m) => m.default)
  }
  const mermaid = await mermaidPromise
  const themeNow = effectiveTheme()
  if (currentTheme !== themeNow) {
    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: themeNow === 'dark' ? 'dark' : 'default',
      // 字体跟全局 UI 一致，避免 mermaid 默认衬线字体在我们的 sans-serif UI 里突兀。
      fontFamily:
        '-apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif',
    })
    currentTheme = themeNow
  }
  return mermaid
}

/** 渲染 root 下所有未渲染过的 .md-mermaid 节点。幂等 —— 已渲染过的（带 data-rendered）会跳过。 */
export async function renderAllMermaid(root: HTMLElement | null): Promise<void> {
  if (!root) return
  const nodes = root.querySelectorAll<HTMLElement>('.md-mermaid:not([data-rendered])')
  if (!nodes.length) return
  let mermaid: Awaited<ReturnType<typeof loadMermaid>>
  try {
    mermaid = await loadMermaid()
  } catch (e) {
    // mermaid 拉不到（离线 / 包损坏）——保留占位符里的源码，不阻断其它消息渲染。
    console.warn('[mermaid] failed to load:', e)
    return
  }
  for (const el of Array.from(nodes)) {
    const src = decodeURIComponent(el.dataset.source ?? '')
    if (!src) {
      el.setAttribute('data-rendered', '1')
      continue
    }
    renderSeq += 1
    const id = `md-mermaid-${renderSeq}`
    try {
      const { svg } = await mermaid.render(id, src)
      el.innerHTML = svg
      // 甘特图等宽图表：mermaid 把 SVG width 设为容器宽度导致内容挤压看不清。
      // 读 viewBox 的固有宽度作为 min-width，容器 overflow:auto 提供滚动。
      const svgEl = el.querySelector('svg')
      if (svgEl) {
        const vb = svgEl.getAttribute('viewBox')
        if (vb) {
          const parts = vb.split(/[\s,]+/)
          const vbWidth = parseFloat(parts[2])
          if (vbWidth > 0) {
            svgEl.style.minWidth = `${vbWidth}px`
            svgEl.removeAttribute('width')
          }
        }
      }
      addExportButton(el)
      el.setAttribute('data-rendered', '1')
    } catch (e) {
      // 语法错误 / 渲染失败：把 .md-mermaid 改成 .md-mermaid-error，露出源码 + 一行错误。
      const msg = (e as Error)?.message ?? String(e)
      el.classList.add('md-mermaid-error')
      el.innerHTML =
        `<div class="md-mermaid-errmsg">mermaid: ${escapeHtml(msg)}</div>` +
        `<pre class="md-mermaid-source">${escapeHtml(src)}</pre>`
      el.setAttribute('data-rendered', '1')
    }
  }
}

/** 主题切换时把所有 .md-mermaid 标记成"未渲染"，下次 renderAllMermaid 会重画。
 *  调用方应在 nextTick 后调 renderAllMermaid。 */
export function resetMermaidForTheme(root: HTMLElement | null): void {
  currentTheme = null
  if (!root) return
  root.querySelectorAll<HTMLElement>('.md-mermaid[data-rendered]').forEach((el) => {
    const src = decodeURIComponent(el.dataset.source ?? '')
    el.removeAttribute('data-rendered')
    el.classList.remove('md-mermaid-error')
    el.innerHTML = `<pre class="md-mermaid-source">${escapeHtml(src)}</pre>`
  })
}

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

const DOWNLOAD_SVG =
  '<svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>'

function addExportButton(container: HTMLElement): void {
  if (container.querySelector('.mermaid-export')) return
  const btn = document.createElement('button')
  btn.type = 'button'
  btn.className = 'mermaid-export'
  btn.setAttribute('aria-label', 'Export as PNG')
  btn.innerHTML = DOWNLOAD_SVG
  btn.addEventListener('click', (e) => {
    e.preventDefault()
    e.stopPropagation()
    const svgEl = container.querySelector('svg')
    if (!svgEl) return
    void exportSvgAsPng(svgEl)
  })
  container.appendChild(btn)
}

async function exportSvgAsPng(svgEl: SVGSVGElement): Promise<void> {
  const { save: saveDialog } = await import('@tauri-apps/plugin-dialog')
  const { writeBinaryFile, revealInFinder } = await import('./api')

  const chosen = await saveDialog({
    defaultPath: `mermaid-${Date.now()}.png`,
    filters: [{ name: 'PNG Image', extensions: ['png'] }],
  })
  if (!chosen) return

  const clone = svgEl.cloneNode(true) as SVGSVGElement
  const vb = svgEl.getAttribute('viewBox')
  const rect = svgEl.getBoundingClientRect()
  const w = parseFloat(clone.getAttribute('width') || '') || (vb ? parseFloat(vb.split(/[\s,]+/)[2]) : rect.width)
  const h = parseFloat(clone.getAttribute('height') || '') || (vb ? parseFloat(vb.split(/[\s,]+/)[3]) : rect.height)
  clone.setAttribute('width', `${w}`)
  clone.setAttribute('height', `${h}`)
  if (!clone.getAttribute('xmlns')) clone.setAttribute('xmlns', 'http://www.w3.org/2000/svg')

  const styles = document.querySelectorAll('style')
  const styleEl = document.createElementNS('http://www.w3.org/2000/svg', 'style')
  let css = ''
  styles.forEach((s) => { css += s.textContent || '' })
  styleEl.textContent = css
  clone.insertBefore(styleEl, clone.firstChild)

  const scale = 2
  const serialized = new XMLSerializer().serializeToString(clone)
  const blob = new Blob([serialized], { type: 'image/svg+xml;charset=utf-8' })
  const url = URL.createObjectURL(blob)

  await new Promise<void>((resolve) => {
    const img = new Image()
    img.onload = async () => {
      const canvas = document.createElement('canvas')
      canvas.width = w * scale
      canvas.height = h * scale
      const ctx = canvas.getContext('2d')!
      ctx.fillStyle = '#ffffff'
      ctx.fillRect(0, 0, canvas.width, canvas.height)
      ctx.drawImage(img, 0, 0, canvas.width, canvas.height)
      URL.revokeObjectURL(url)
      const pngBlob = await new Promise<Blob | null>((r) => canvas.toBlob(r, 'image/png'))
      if (pngBlob) {
        const buf = await pngBlob.arrayBuffer()
        const base64 = btoa(String.fromCharCode(...new Uint8Array(buf)))
        await writeBinaryFile(chosen, base64)
        revealInFinder(chosen)
      }
      resolve()
    }
    img.onerror = () => { URL.revokeObjectURL(url); resolve() }
    img.src = url
  })
}
