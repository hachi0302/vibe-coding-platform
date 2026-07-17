<script setup lang="ts">
import type { CreateProjectResult } from '../../projectFactory/types'

defineProps<{ result?: CreateProjectResult; error?: string }>()
const emit = defineEmits<{ (e: 'open', path: string): void; (e: 'back'): void }>()
</script>

<template>
  <section class="pf-panel pf-result-panel">
    <template v-if="result">
      <div class="pf-result-mark">完成</div>
      <h2>项目骨架已创建</h2>
      <p>{{ result.message }}</p>
      <div class="pf-verification" :class="result.verification.status">
        <button v-if="result.verification.status === 'passed'" type="button" class="pf-button verified" disabled>已验证可启动</button>
        <strong v-else-if="result.verification.status === 'failed'">启动自检未通过</strong>
        <strong v-else>尚未完成启动自检</strong>
        <p>{{ result.verification.detail }}</p>
        <ul v-if="result.verification.checks.length">
          <li v-for="check in result.verification.checks" :key="check">{{ check }}</li>
        </ul>
      </div>
      <div class="pf-result-path-list">
        <div v-for="path in result.projectPaths" :key="path" class="pf-result-path-row">
          <code class="pf-result-path">{{ path }}</code>
          <button type="button" class="pf-button secondary compact" @click="emit('open', path)">打开目录</button>
        </div>
      </div>
    </template>
    <template v-else>
      <div class="pf-result-mark danger">失败</div>
      <h2>项目未创建</h2>
      <p class="pf-error">{{ error ?? '创建过程出现未知错误。' }}</p>
      <div class="pf-actions"><button type="button" class="pf-button primary" @click="emit('back')">返回预览</button></div>
    </template>
  </section>
</template>
