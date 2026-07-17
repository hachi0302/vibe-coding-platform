import type { CreateProjectRequest, ProjectPreview } from './types'

function joinPath(parentPath: string, projectName: string) {
  return `${parentPath.replace(/[\\/]$/, '')}/${projectName}`
}

export function buildPreview(request: CreateProjectRequest): ProjectPreview {
  const directories = ['docs/', 'docs/product/', 'docs/detail-design/', 'docs/progress/']
  const files = ['README.md', '.gitignore']
  const basePath = request.parentPath.replace(/[\\/]$/, '')
  const frontendProjectName = request.frontendProjectName?.trim() || `${request.projectName}-frontend`
  const backendProjectName = request.backendProjectName?.trim() || `${request.projectName}-backend`
  const targetPaths = request.recommendation.structure === 'frontend-backend'
    ? [
        { label: '前端项目', path: joinPath(basePath, frontendProjectName) },
        { label: '后端项目', path: joinPath(basePath, backendProjectName) },
      ]
    : [{ label: '项目路径', path: joinPath(basePath, request.projectName) }]
  if (request.recommendation.structure === 'frontend-backend') {
    directories.unshift('src/')
  } else if (request.recommendation.id === 'tauri-vue') {
    directories.unshift('src/', 'src-tauri/')
  } else {
    directories.unshift('src/')
  }

  const agentFiles = request.agentChoice === 'claude'
    ? ['CLAUDE.md', '.claude/rules/', '.claude/skills/', '.claude/settings.json']
    : request.agentChoice === 'codex'
      ? ['AGENTS.md', '.agents/CODEX.md', '.agents/rules/', '.agents/skills/']
      : [
          'CLAUDE.md', 'AGENTS.md', '.claude/rules/', '.claude/skills/',
          '.agents/rules/ → .claude/rules/', '.agents/skills/ → .claude/skills/',
        ]

  return {
    projectName: request.projectName,
    parentPath: request.parentPath,
    targetPaths,
    directories,
    files,
    agentFiles,
    agentMode: request.agentChoice === 'both' ? 'symlink' : request.agentChoice,
  }
}
