<script setup lang="ts">
import type { AgentChoice, ProjectPreview, StackRecommendation } from '../../projectFactory/types'

defineProps<{
  projectName: string
  parentPath: string
  frontendProjectName: string
  backendProjectName: string
  agentChoice: AgentChoice
  preview: ProjectPreview
  recommendation: StackRecommendation
  creating?: boolean
  error?: string
}>()
const emit = defineEmits<{
  (e: 'update:projectName', value: string): void
  (e: 'update:parentPath', value: string): void
  (e: 'update:frontendProjectName', value: string): void
  (e: 'update:backendProjectName', value: string): void
  (e: 'update:agentChoice', value: AgentChoice): void
  (e: 'choose-path'): void
  (e: 'create'): void
  (e: 'back'): void
}>()
</script>

<template>
  <section class="pf-panel pf-preview-panel">
    <div class="pf-panel-head">
      <div>
        <h2>创建前预览</h2>
        <p>确认目标路径、项目结构和开发智能体规则后再创建。</p>
      </div>
      <span class="pf-step">05 / 预览</span>
    </div>
    <p v-if="error" class="pf-error">{{ error }}</p>
    <label class="pf-field">
      <span>项目名称</span>
      <input class="pf-project-name" :value="projectName" @input="emit('update:projectName', ($event.target as HTMLInputElement).value)">
      <small>由智能体根据需求建议；修改后会同步更新生成路径与文档名称。</small>
    </label>
    <label class="pf-field">
      <span>项目父路径</span>
      <div class="pf-file-row">
        <input :value="parentPath" placeholder="选择项目保存位置" @input="emit('update:parentPath', ($event.target as HTMLInputElement).value)">
        <button type="button" class="pf-button secondary" @click="emit('choose-path')">选择路径</button>
      </div>
    </label>
    <div v-if="recommendation.structure === 'frontend-backend'" class="pf-project-name-grid">
      <label class="pf-field">
        <span>前端项目名</span>
        <input :value="frontendProjectName" :placeholder="`${preview.projectName}-frontend`" @input="emit('update:frontendProjectName', ($event.target as HTMLInputElement).value)">
      </label>
      <label class="pf-field">
        <span>后端项目名</span>
        <input :value="backendProjectName" :placeholder="`${preview.projectName}-backend`" @input="emit('update:backendProjectName', ($event.target as HTMLInputElement).value)">
      </label>
    </div>
    <div class="pf-field">
      <span>开发智能体</span>
      <div class="pf-segmented">
        <button type="button" :class="{ active: agentChoice === 'claude' }" @click="emit('update:agentChoice', 'claude')">Claude Code</button>
        <button type="button" :class="{ active: agentChoice === 'codex' }" @click="emit('update:agentChoice', 'codex')">Codex</button>
        <button type="button" :class="{ active: agentChoice === 'both' }" @click="emit('update:agentChoice', 'both')">两个都用</button>
      </div>
    </div>
    <div class="pf-preview-grid">
      <div><span>项目路径</span><code v-for="item in preview.targetPaths" :key="item.path">{{ item.label }}：{{ item.path }}</code></div>
      <div><span>技术栈</span><strong>{{ recommendation.title }}</strong></div>
      <div><span>目录结构</span><code v-for="item in preview.directories" :key="item">{{ item }}</code></div>
      <div><span>AI 规则文件</span><code v-for="item in preview.agentFiles" :key="item">{{ item }}</code></div>
    </div>
    <div class="pf-actions">
      <button type="button" class="pf-button secondary" :disabled="creating" @click="emit('back')">返回环境</button>
      <button type="button" class="pf-button primary" :disabled="creating || !parentPath.trim()" @click="emit('create')">{{ creating ? '创建并自检中...' : '创建并自检项目' }}</button>
    </div>
  </section>
</template>
