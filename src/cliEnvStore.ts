import { ref, computed } from 'vue'
import type { CliVersionInfo, CliDiagnosisResult } from './types'
import * as api from './api'

export const cliVersions = ref<CliVersionInfo[]>([])
export const loading = ref(false)
export const installing = ref<Record<string, boolean>>({})
export const installMsg = ref<Record<string, { ok: boolean; text: string } | undefined>>({})
export const upgrading = ref<Record<string, boolean>>({})
export const diagnosing = ref<Record<string, boolean>>({})
export const diagnosisResults = ref<Record<string, CliDiagnosisResult | null>>({})
export const upgradeMsg = ref<Record<string, { ok: boolean; text: string }>>({})

export const upgradableCount = computed(() =>
  cliVersions.value.filter((v) => v.upgradable).length,
)

export const anyUpgrading = computed(() =>
  Object.values(upgrading.value).some(Boolean),
)

async function fetchVersions() {
  loading.value = true
  try {
    cliVersions.value = await api.checkCliVersions()
  } catch {
    cliVersions.value = []
  } finally {
    loading.value = false
  }
}

export async function refresh() {
  upgradeMsg.value = {}
  installMsg.value = {}
  await fetchVersions()
}

export async function install(cli: string) {
  installing.value = { ...installing.value, [cli]: true }
  installMsg.value = { ...installMsg.value, [cli]: undefined }
  try {
    const r = await api.installCli(cli)
    if (r.success) {
      installMsg.value = { ...installMsg.value, [cli]: { ok: true, text: r.newVersion || '' } }
    } else {
      installMsg.value = { ...installMsg.value, [cli]: { ok: false, text: r.error || 'unknown' } }
    }
    await fetchVersions()
  } catch (e) {
    installMsg.value = { ...installMsg.value, [cli]: { ok: false, text: String(e) } }
  } finally {
    installing.value = { ...installing.value, [cli]: false }
  }
}

export async function upgrade(cli: string) {
  upgrading.value = { ...upgrading.value, [cli]: true }
  upgradeMsg.value = { ...upgradeMsg.value, [cli]: undefined as never }
  try {
    const r = await api.upgradeCli(cli)
    if (r.success) {
      upgradeMsg.value = { ...upgradeMsg.value, [cli]: { ok: true, text: r.newVersion || '' } }
    } else if (r.error === 'version_unchanged') {
      upgradeMsg.value = { ...upgradeMsg.value, [cli]: { ok: false, text: 'version_unchanged' } }
    } else {
      upgradeMsg.value = { ...upgradeMsg.value, [cli]: { ok: false, text: r.error || 'unknown' } }
    }
    await fetchVersions()
  } catch (e) {
    upgradeMsg.value = { ...upgradeMsg.value, [cli]: { ok: false, text: String(e) } }
  } finally {
    upgrading.value = { ...upgrading.value, [cli]: false }
  }
}

export async function upgradeAll() {
  const targets = cliVersions.value.filter((v) => v.upgradable)
  const next: Record<string, boolean> = {}
  for (const v of targets) next[v.cli] = true
  upgrading.value = { ...upgrading.value, ...next }
  try {
    const results = await api.upgradeAllClis()
    for (const r of results) {
      if (r.success) {
        upgradeMsg.value = { ...upgradeMsg.value, [r.cli]: { ok: true, text: r.newVersion || '' } }
      } else if (r.error === 'version_unchanged') {
        upgradeMsg.value = { ...upgradeMsg.value, [r.cli]: { ok: false, text: 'version_unchanged' } }
      } else {
        upgradeMsg.value = { ...upgradeMsg.value, [r.cli]: { ok: false, text: r.error || 'unknown' } }
      }
    }
    await refresh()
  } catch (e) {
    for (const v of targets)
      upgradeMsg.value = { ...upgradeMsg.value, [v.cli]: { ok: false, text: String(e) } }
  } finally {
    const done: Record<string, boolean> = {}
    for (const v of targets) done[v.cli] = false
    upgrading.value = { ...upgrading.value, ...done }
  }
}

export const anyDiagnosing = computed(() =>
  Object.values(diagnosing.value).some(Boolean),
)

export const hasDiagnosisResults = computed(() =>
  Object.values(diagnosisResults.value).some(Boolean),
)

export async function diagnoseAll() {
  if (hasDiagnosisResults.value) {
    diagnosisResults.value = {}
    return
  }
  const installed = cliVersions.value.filter((v) => v.installed)
  if (!installed.length) return
  const next: Record<string, boolean> = {}
  for (const v of installed) next[v.cli] = true
  diagnosing.value = { ...diagnosing.value, ...next }
  try {
    const results = await Promise.all(
      installed.map((v) => api.diagnoseCli(v.cli).catch(() => null)),
    )
    const updated: Record<string, CliDiagnosisResult | null> = {}
    for (let i = 0; i < installed.length; i++) {
      updated[installed[i].cli] = results[i]
    }
    diagnosisResults.value = { ...diagnosisResults.value, ...updated }
  } finally {
    const done: Record<string, boolean> = {}
    for (const v of installed) done[v.cli] = false
    diagnosing.value = { ...diagnosing.value, ...done }
  }
}
