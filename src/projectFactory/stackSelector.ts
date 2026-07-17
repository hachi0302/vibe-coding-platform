import type {
  FeatureSet,
  ProjectProfile,
  RequirementContext,
  StackRecommendation,
  StackRecommendationResult,
  SystemType,
  TechPreference,
  TechnologyDecision,
} from './types'

interface Candidate extends Omit<StackRecommendation, 'status'> {
  systemTypes: SystemType[]
  preferences: TechPreference[]
  score: number
}

const emptyFeatures = (): FeatureSet => ({
  seo: false,
  mobileFirst: false,
  auth: false,
  fileUpload: false,
  realtime: false,
  paymentOrOrder: false,
  adminConsole: false,
  crossPlatform: false,
  offline: false,
})

function contains(text: string, words: string[]) {
  return words.some(word => text.includes(word))
}

function baseDecisions(runtime: string[], runtimeReason: string): TechnologyDecision[] {
  return [
    { category: 'runtime', title: '后端运行时', status: 'adopt', choices: runtime, reason: runtimeReason, provision: 'project' },
    { category: 'persistence', title: '数据存储', status: 'defer', choices: [], reason: '当前兜底分析没有足够的数据模型与已有基础设施信息。', provision: 'not-applicable', trigger: '确认核心数据、事务和团队既有数据库后决定。' },
    { category: 'data-access', title: '数据访问方式', status: 'defer', choices: [], reason: '需要先确定数据模型、查询复杂度和团队已有代码规范。', provision: 'not-applicable', trigger: '选定持久化方案后决定 ORM 或 SQL 访问方式。' },
    { category: 'cache', title: '缓存与短期状态', status: 'not-needed', choices: [], reason: '尚未发现热点读取、限流、会话或分布式锁等明确需求。', provision: 'not-applicable' },
    { category: 'messaging', title: '消息与异步任务', status: 'not-needed', choices: [], reason: '尚未发现可靠异步、削峰、事件回放或跨服务解耦需求。', provision: 'not-applicable' },
    { category: 'configuration', title: '配置与服务治理', status: 'not-needed', choices: [], reason: '默认从单体或少量服务开始，优先使用部署环境配置。', provision: 'not-applicable' },
    { category: 'observability', title: '上线基线', status: 'adopt', choices: ['健康检查', '结构化日志', '环境变量配置'], reason: '即使是最小骨架也需要可诊断和可配置的运行基础。', provision: 'project' },
  ]
}

export function extractProjectProfile(context: RequirementContext): ProjectProfile {
  const text = context.text.trim().toLowerCase()
  const features = emptyFeatures()
  features.seo = contains(text, ['seo', '官网', '落地页'])
  features.mobileFirst = contains(text, ['h5', '移动端', '手机端', '响应式'])
  features.auth = contains(text, ['登录', '权限', '账号', '认证'])
  features.fileUpload = contains(text, ['上传', '附件', '文件', '图片'])
  features.realtime = contains(text, ['实时', '消息', '聊天', 'websocket'])
  features.paymentOrOrder = contains(text, ['订单', '支付', '退款', '交易', '结算'])
  features.adminConsole = contains(text, ['后台', '管理', '运营', '内部员工'])
  features.crossPlatform = contains(text, ['跨平台', 'windows', 'macos', '桌面'])
  features.offline = contains(text, ['离线', '本机', '本地文件'])

  let systemType: SystemType = 'fullstack'
  if (contains(text, ['桌面', '客户端', '本机 ai', '本地 ai'])) systemType = 'desktop'
  else if (contains(text, ['小程序', '微信小程序'])) systemType = 'mini-program'
  else if (contains(text, ['app', '安卓', 'ios'])) systemType = 'app'
  else if (contains(text, ['cli', '命令行', '脚本工具'])) systemType = 'cli'
  else if (contains(text, ['接口', 'api 服务', '后端服务'])) systemType = 'backend-api'
  else if (features.seo || contains(text, ['h5', '官网'])) systemType = 'web-h5'
  else if (features.adminConsole || context.audience === 'internal-staff') systemType = 'admin'

  return {
    summary: context.text.trim(),
    systemType,
    audience: context.audience,
    scale: context.scale,
    preference: context.preference,
    features,
  }
}

function candidateStacks(profile: ProjectProfile): Candidate[] {
  const candidates: Candidate[] = [
    {
      id: 'vue-spring-boot',
      title: 'Vue 3 + Spring Boot 3',
      frontend: ['Vue 3', 'TypeScript', 'Vite'],
      backend: ['Spring Boot', 'Java'],
      database: [],
      cache: [],
      messaging: [],
      decisions: baseDecisions(['Java', 'Spring Boot'], '适合权限、订单和长期维护的中后台业务；具体版本和数据层交由智能体结合项目环境确认。'),
      structure: 'frontend-backend',
      packageManager: 'maven',
      reasons: ['适合权限、订单和长期维护的中后台业务'],
      tradeoffs: ['需要维护前后端两个工程与 Java 运行环境'],
      preferenceMatched: profile.preference === 'java' || profile.preference === 'vue',
      systemTypes: ['admin', 'fullstack', 'backend-api'],
      preferences: ['java', 'vue'],
      score: 0,
    },
    {
      id: 'vue-vite',
      title: 'Vue 3 + Vite',
      frontend: ['Vue 3', 'TypeScript', 'Vite'],
      backend: [],
      database: [],
      cache: [],
      messaging: [],
      decisions: baseDecisions([], '当前需求可能只需要前端单应用，后端运行时待真实需求确认。').map(item => item.category === 'runtime' ? { ...item, status: 'not-needed', choices: [], reason: '当前轻量展示场景不需要独立后端。', provision: 'not-applicable' } : item),
      structure: 'single-app',
      packageManager: 'npm',
      reasons: ['适合轻量展示、内容站和移动端网页'],
      tradeoffs: ['复杂业务能力需要后续接入后端服务'],
      preferenceMatched: profile.preference === 'vue' || profile.preference === 'node',
      systemTypes: ['web-h5', 'admin', 'fullstack'],
      preferences: ['vue', 'node'],
      score: 0,
    },
    {
      id: 'nextjs',
      title: 'Next.js + TypeScript',
      frontend: ['Next.js', 'TypeScript'],
      backend: [],
      database: [],
      cache: [],
      messaging: [],
      decisions: baseDecisions([], '服务端渲染由 Next.js 应用承担，独立后端待业务数据和集成需求确认。').map(item => item.category === 'runtime' ? { ...item, status: 'not-needed', choices: [], reason: '当前对外站点可先由 Next.js 单应用提供服务端能力。', provision: 'not-applicable' } : item),
      structure: 'single-app',
      packageManager: 'npm',
      reasons: ['服务端渲染适合重视 SEO 的对外网站'],
      tradeoffs: ['团队需要接受 React 与 Next.js 生态'],
      preferenceMatched: profile.preference === 'react' || profile.preference === 'node',
      systemTypes: ['web-h5', 'fullstack'],
      preferences: ['react', 'node'],
      score: 0,
    },
    {
      id: 'tauri-vue',
      title: 'Tauri + Vue 3',
      frontend: ['Vue 3', 'TypeScript', 'Vite'],
      backend: ['Rust', 'Tauri 2'],
      database: [],
      cache: [],
      messaging: [],
      decisions: baseDecisions(['Rust', 'Tauri'], '本机文件、离线和跨平台桌面能力优先于通用 Web 服务运行时。'),
      structure: 'single-app',
      packageManager: 'npm',
      reasons: ['适合访问本机文件、AI 会话和跨平台桌面能力'],
      tradeoffs: ['需要 Node.js 与 Rust 双运行环境'],
      preferenceMatched: profile.preference === 'vue' || profile.preference === 'node',
      systemTypes: ['desktop'],
      preferences: ['vue', 'node'],
      score: 0,
    },
    {
      id: 'node-nestjs',
      title: 'Vue 3 + NestJS',
      frontend: ['Vue 3', 'TypeScript', 'Vite'],
      backend: ['NestJS', 'Node.js'],
      database: [],
      cache: [],
      messaging: [],
      decisions: baseDecisions(['TypeScript', 'NestJS'], 'TypeScript 全栈协作效率高，适合快速交付；数据层按实际业务和团队基础设施确认。'),
      structure: 'frontend-backend',
      packageManager: 'npm',
      reasons: ['TypeScript 全栈协作效率高，适合快速交付'],
      tradeoffs: ['大型复杂业务的团队规范需要额外沉淀'],
      preferenceMatched: profile.preference === 'node' || profile.preference === 'vue',
      systemTypes: ['admin', 'fullstack', 'backend-api'],
      preferences: ['node', 'vue'],
      score: 0,
    },
  ]
  return candidates.map(candidate => ({ ...candidate, score: scoreCandidate(candidate, profile) }))
}

function scoreCandidate(candidate: Candidate, profile: ProjectProfile): number {
  let score = candidate.systemTypes.includes(profile.systemType) ? 45 : -20
  if (profile.preference && profile.preference !== 'none') {
    score += candidate.preferences.includes(profile.preference) ? 40 : -8
  }
  if (profile.scale === 'large-maintained' && candidate.id === 'vue-spring-boot') score += 18
  if (profile.features.paymentOrOrder && candidate.id === 'vue-spring-boot') score += 14
  if (profile.features.auth && candidate.id === 'vue-spring-boot') score += 8
  if (profile.features.seo && candidate.id === 'nextjs') score += 20
  if (profile.features.mobileFirst && candidate.id === 'vue-vite') score += 12
  if (profile.systemType === 'web-h5' && candidate.backend.length === 0) score += 10
  if (profile.systemType === 'desktop' && candidate.id === 'tauri-vue') score += 30
  return score
}

function visibleRecommendation(candidate: Candidate, status: StackRecommendation['status']): StackRecommendation {
  const { systemTypes: _systemTypes, preferences: _preferences, score: _score, ...recommendation } = candidate
  return { ...recommendation, status }
}

export function recommendStacks(context: RequirementContext): StackRecommendationResult {
  const profile = extractProjectProfile(context)
  const candidates = candidateStacks(profile).sort((a, b) => b.score - a.score)
  const [first, ...rest] = candidates
  return {
    profile,
    recommended: visibleRecommendation(first, 'recommended'),
    alternatives: rest.slice(0, 2).map(candidate => visibleRecommendation(candidate, 'alternative')),
    notRecommended: rest.slice(2).filter(candidate => candidate.score < 0).map(candidate => visibleRecommendation(candidate, 'not-recommended')),
    projectName: context.projectName?.trim() || 'new-project',
    projectNameReason: context.projectName?.trim() ? '沿用已有项目名。' : '本地兜底仅用于需求摘要，最终名称由智能体分析生成。',
  }
}
