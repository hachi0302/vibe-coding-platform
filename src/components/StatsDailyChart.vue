<script setup lang="ts">
// 每日 cost / calls 双 Y 轴图。
//
// 视觉设计：
//   - calls（右轴）：软中性灰柱体，不抢戏；只是日活背景
//   - cost（左轴）：brand 平滑折线 + 渐变填充面 + 醒目数据点
// 颜色单独读 CSS 变量；theme 切换会重建图表。
//
// 单日回退：会话只跨一天时，二维图退化成一个孤零零的点，视觉极差 ——
// 改为渲染居中的摘要卡片（template 里的 .single-day-summary），跳过 G2。

import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { Chart } from '@antv/g2'
import { theme } from '../settings'
import { t } from '../i18n'
import { isDark, readPalette } from './chartPalette'

interface DailyPoint {
  date: string
  cost: number
  calls: number
}

const props = defineProps<{ data: DailyPoint[] }>()
const el = ref<HTMLDivElement | null>(null)
let chart: Chart | null = null

const isSingleDay = computed(() => (props.data?.length ?? 0) === 1)
const singleDay = computed(() => (isSingleDay.value ? props.data[0] : null))

function fmtCost(n: number): string {
  if (!Number.isFinite(n) || n === 0) return '$0.00'
  if (n < 0.01) return '<$0.01'
  if (n < 10) return `$${n.toFixed(2)}`
  if (n < 1000) return `$${n.toFixed(0)}`
  return `$${(n / 1000).toFixed(1)}K`
}

function build() {
  if (!el.value) return
  if (chart) {
    chart.destroy()
    chart = null
  }
  if (!props.data || props.data.length === 0) return
  // 单日数据走模板里的摘要卡片，不渲染 G2 —— 否则只会留一个孤点。
  if (isSingleDay.value) return
  const p = readPalette()
  const dark = isDark()

  chart = new Chart({
    container: el.value,
    autoFit: true,
    theme: dark ? 'classicDark' : 'classic',
    marginTop: 16,
    marginRight: 52,
    marginBottom: 28,
    marginLeft: 56,
  })

  const labelCalls = t('stats.daily.col.calls')
  const labelCost = t('stats.daily.col.cost')

  const data = props.data.map((d) => ({
    date: d.date.slice(5),
    cost: Number(d.cost.toFixed(4)),
    calls: d.calls,
  }))

  chart.data(data)

  // 软底柱：calls（右轴）
  chart
    .interval()
    .encode('x', 'date')
    .encode('y', 'calls')
    .scale('y', { nice: true })
    .style('fill', p.softBar)
    .style('radiusTopLeft', 3)
    .style('radiusTopRight', 3)
    .axis('x', {
      title: null,
      labelFontSize: 10,
      labelFill: p.textMute,
      tick: null,
      line: false,
      labelAutoHide: true,
    })
    .axis('y', {
      position: 'right',
      title: null,
      labelFontSize: 10,
      labelFill: p.textMute,
      grid: true,
      gridStroke: p.grid,
      gridStrokeOpacity: 1,
      gridLineDash: [3, 3],
      tick: null,
      line: false,
    })
    .tooltip({ name: labelCalls, channel: 'y' })

  // 渐变填充面：让折线下方有视觉重量
  chart
    .area()
    .encode('x', 'date')
    .encode('y', 'cost')
    .scale('y', { independent: true, nice: true })
    .style('shape', 'smooth')
    .style('fill', `linear-gradient(90deg, ${p.brand}, ${p.brand})`)
    .style('fillOpacity', dark ? 0.22 : 0.16)
    .axis(false)
    .tooltip(false)

  // brand 平滑折线：cost（左轴）
  chart
    .line()
    .encode('x', 'date')
    .encode('y', 'cost')
    .scale('y', { independent: true, nice: true })
    .style('stroke', p.brand)
    .style('lineWidth', 2.2)
    .style('shape', 'smooth')
    .axis('y', {
      position: 'left',
      title: null,
      labelFontSize: 10,
      labelFill: p.textMute,
      labelFormatter: (v: number) => `$${Number(v).toFixed(2)}`,
      tick: null,
      line: false,
      grid: false,
    })
    .tooltip({
      name: labelCost,
      channel: 'y',
      valueFormatter: (v: number) => `$${Number(v).toFixed(2)}`,
    })

  // 数据点：填色 + 白圈描边，强化"每天一个点"
  chart
    .point()
    .encode('x', 'date')
    .encode('y', 'cost')
    .scale('y', { independent: true })
    .style('fill', p.brand)
    .style('stroke', p.stroke)
    .style('lineWidth', 1.5)
    .style('r', 3)
    .tooltip(false)
    .axis(false)

  chart.legend(false)
  chart.interaction('tooltip', { shared: true })

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
  <div v-if="isSingleDay && singleDay" class="single-day-summary">
    <div class="single-day-date">{{ singleDay.date }}</div>
    <div class="single-day-metrics">
      <div class="single-day-metric">
        <div class="single-day-num">{{ fmtCost(singleDay.cost) }}</div>
        <div class="single-day-label">{{ t('stats.header.cost') }}</div>
      </div>
      <div class="single-day-sep" />
      <div class="single-day-metric">
        <div class="single-day-num">{{ singleDay.calls.toLocaleString() }}</div>
        <div class="single-day-label">{{ t('stats.header.calls') }}</div>
      </div>
    </div>
  </div>
  <div v-else ref="el" class="g2-chart" />
</template>
