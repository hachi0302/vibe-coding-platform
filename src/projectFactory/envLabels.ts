import type { StackRecommendation } from './types'

export interface RequiredTool {
  toolId: 'node' | 'pnpm' | 'jdk' | 'maven' | 'rust' | 'tauri' | 'python' | 'go' | 'dotnet'
  label: string
  command: string
}

const tools: Record<RequiredTool['toolId'], RequiredTool> = {
  node: { toolId: 'node', label: 'Node.js', command: 'node --version' },
  pnpm: { toolId: 'pnpm', label: 'pnpm', command: 'pnpm --version' },
  jdk: { toolId: 'jdk', label: 'JDK 17+', command: 'java -version' },
  maven: { toolId: 'maven', label: 'Maven', command: 'mvn --version' },
  rust: { toolId: 'rust', label: 'Rust', command: 'rustc --version' },
  tauri: { toolId: 'tauri', label: 'Tauri CLI', command: 'tauri --version' },
  python: { toolId: 'python', label: 'Python 3.9+', command: 'python3 --version' },
  go: { toolId: 'go', label: 'Go 1.22+', command: 'go version' },
  dotnet: { toolId: 'dotnet', label: '.NET SDK 8+', command: 'dotnet --version' },
}

export function toolsForRecommendation(recommendation: StackRecommendation): RequiredTool[] {
  const ids = new Set<RequiredTool['toolId']>()
  if (recommendation.frontend.length) {
    ids.add('node')
    if (recommendation.packageManager === 'pnpm') ids.add('pnpm')
  }
  if (recommendation.backend.some(item => item.includes('Java'))) {
    ids.add('jdk')
    ids.add('maven')
  }
  if (recommendation.backend.some(item => item === 'Rust')) {
    ids.add('rust')
    if (recommendation.id === 'tauri-vue') ids.add('tauri')
  }
  if (recommendation.backend.some(item => item.includes('Python'))) ids.add('python')
  if (recommendation.backend.some(item => item === 'Go')) ids.add('go')
  if (recommendation.backend.some(item => item.includes('.NET'))) ids.add('dotnet')
  return [...ids].map(id => tools[id])
}
