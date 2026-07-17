<script setup lang="ts">
// 按 activity 类型的 cost 横向条形图。activity 是分类（Coding / Refactoring /
// Conversation / ...），每条 bar 一个独立分类色比清一色 brand 更易读。

import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Chart } from '@antv/g2'
import { theme } from '../settings'
import { categoricalColors, readPalette } from './chartPalette'

interface ActivityPoint {
  name: string
  cost: number
}

const props = defineProps<{ data: ActivityPoint[] }>()
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

  const data = props.data.map((d) => ({
    name: d.name,
    cost: Number(d.cost.toFixed(4)),
  }))

  chart.coordinate({ transform: [{ type: 'transpose' }] })

  chart
    .interval()
    .data(data)
    .encode('x', 'name')
    .encode('y', 'cost')
    .encode('color', 'name')
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
      title: (d: { name: string }) => d.name,
      items: [
        {
          field: 'cost',
          valueFormatter: (v: number) => `$${Number(v).toFixed(2)}`,
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
