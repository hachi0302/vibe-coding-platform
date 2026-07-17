<script setup lang="ts">
// 按 model 的 cost 横向条形图。每个模型一个分类色 —— 比从 brand 派生 HSL 旋转
// 更清晰，因为模型名一般不超过 8 个（Opus / Sonnet / Haiku / GPT-5.x ...）。

import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Chart } from '@antv/g2'
import { theme } from '../settings'
import { categoricalColors, readPalette } from './chartPalette'

interface ModelPoint {
  label: string
  cost: number
}

const props = defineProps<{ data: ModelPoint[] }>()
const el = ref<HTMLDivElement | null>(null)
let chart: Chart | null = null

function build() {
  if (!el.value) return
  if (chart) {
    chart.destroy()
    chart = null
  }
  if (!props.data || props.data.length === 0) return
  const p = readPalette()
  const dark = document.documentElement.classList.contains('theme-dark')
  const total = props.data.reduce((a, b) => a + b.cost, 0)
  const colors = categoricalColors(props.data.length)

  chart = new Chart({
    container: el.value,
    autoFit: true,
    theme: dark ? 'classicDark' : 'classic',
    marginTop: 8,
    marginRight: 28,
    marginBottom: 32,
    marginLeft: 8,
  })

  const data = props.data.map((m) => ({
    label: m.label,
    cost: Number(m.cost.toFixed(4)),
  }))

  chart.coordinate({ transform: [{ type: 'transpose' }] })

  chart
    .interval()
    .data(data)
    .encode('x', 'label')
    .encode('y', 'cost')
    .encode('color', 'label')
    .scale('x', { padding: 0.35 })
    .scale('color', { range: colors })
    .style('radiusTopLeft', 4)
    .style('radiusTopRight', 4)
    .axis('x', {
      title: null,
      labelFontSize: 11,
      labelFill: p.textMute,
      tick: null,
      line: false,
      grid: false,
    })
    .axis('y', {
      title: null,
      labelFontSize: 10,
      labelFill: p.textMute,
      labelFormatter: (v: number) => `$${Number(v).toFixed(2)}`,
      tick: null,
      line: false,
      grid: true,
      gridStroke: p.grid,
      gridStrokeOpacity: 1,
      gridLineDash: [3, 3],
    })
    .legend(false)
    .tooltip({
      title: (d: { label: string }) => d.label,
      items: [
        {
          field: 'cost',
          valueFormatter: (v: number) => {
            const pct = total > 0 ? (v / total) * 100 : 0
            return `$${v.toFixed(2)} (${pct.toFixed(1)}%)`
          },
        },
      ],
    })

  chart.render()
}

onMounted(build)
onBeforeUnmount(() => {
  chart?.destroy()
  chart = null
})

watch(() => props.data, build, { deep: true })
watch(theme, () => build())
</script>

<template>
  <div ref="el" class="g2-chart" />
</template>
