<script setup lang="ts">
// 统计页扫描中占位：用一组上下脉动的小柱子代替静态的 bar-chart 图标。
// 4 根柱子分别给不同 animation-delay，看起来像"正在拉数据"。
</script>

<template>
  <div class="stats-loading-icon" aria-hidden="true">
    <span class="bar bar-1" />
    <span class="bar bar-2" />
    <span class="bar bar-3" />
    <span class="bar bar-4" />
    <span class="baseline" />
  </div>
</template>
