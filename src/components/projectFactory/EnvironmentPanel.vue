<script setup lang="ts">
import type { EnvCheckItem, TechnologyDecision } from '../../projectFactory/types'

defineProps<{
  items: EnvCheckItem[]
  services?: TechnologyDecision[]
  loading?: boolean
  installingToolId?: string
  error?: string
}>()
const emit = defineEmits<{
  (e: 'refresh'): void
  (e: 'install', toolId: string): void
  (e: 'continue'): void
  (e: 'back'): void
}>()

function environmentStatus(item: EnvCheckItem) {
  if (item.installed && item.compatible) return '已就绪'
  if (item.installed) return '需要升级'
  return '未安装'
}
</script>

<template>
  <section class="pf-panel pf-environment-panel">
    <div class="pf-panel-head">
      <div>
        <h2>本机开发环境</h2>
        <p>仅检查当前方案需要的工具。macOS 使用 Homebrew，Windows 使用 winget。</p>
      </div>
      <span class="pf-step">04 / 环境</span>
    </div>
    <p v-if="error" class="pf-error">{{ error }}</p>
    <div class="pf-env-list">
      <div v-for="item in items" :key="item.toolId" class="pf-env-item">
        <div>
          <strong>{{ item.label }}</strong>
          <span>{{ item.detail ?? (item.installed ? item.version ?? '已安装' : '未安装或未加入 PATH') }}</span>
        </div>
        <button
          v-if="!item.installed || !item.compatible"
          type="button"
          class="pf-button secondary compact"
          :disabled="installingToolId === item.toolId"
          @click="emit('install', item.toolId)"
        >{{ installingToolId === item.toolId ? '安装中...' : item.installed ? '升级' : '安装' }}</button>
        <span v-else class="pf-ok">{{ environmentStatus(item) }}</span>
      </div>
      <div v-if="!items.length && loading" class="pf-empty">正在检查环境...</div>
      <div v-else-if="!items.length" class="pf-empty">暂未获得环境检查结果。</div>
    </div>
    <section v-if="services?.length" class="pf-external-services">
      <div>
        <h3>外部服务</h3>
        <p>按生成项目的配置连接开发环境、Docker 或云服务；不要求在本机安装服务端。</p>
      </div>
      <span v-for="service in services" :key="service.category">{{ service.title }}：{{ service.choices.join('、') || '复用既有能力' }}</span>
    </section>
    <div class="pf-actions">
      <button type="button" class="pf-button secondary" @click="emit('back')">返回方案</button>
      <button type="button" class="pf-button text" :disabled="loading" @click="emit('refresh')">重新检查</button>
      <button type="button" class="pf-button primary" :disabled="loading" @click="emit('continue')">查看项目预览</button>
    </div>
  </section>
</template>
