import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ProjectContextMenu from '../../src/modals/ProjectContextMenu.vue'
import { setLang } from '../../src/settings'
import type { ProjectInfo } from '../../src/types'

beforeEach(() => setLang('en'))
afterEach(() => { document.body.innerHTML = '' })

const project = (over: Partial<ProjectInfo> & { dirName: string }): ProjectInfo => ({
  displayPath: `/projects/${over.dirName}`,
  sessionCount: 1,
  lastModified: 0,
  exists: true,
  ...over,
})

type Props = InstanceType<typeof ProjectContextMenu>['$props']
const factory = (props: Partial<Props> & { project: ProjectInfo }) =>
  mount(ProjectContextMenu, {
    props: { x: 0, y: 0, projState: undefined, isGitRepo: false, ...props } as Props,
    attachTo: document.body,
  })

// 菜单 Teleport 到 body，按钮文本从 document.body 读。
const itemTexts = () =>
  Array.from(document.body.querySelectorAll('.ctx-item')).map((el) => el.textContent?.trim() ?? '')

describe('ProjectContextMenu — worktree items', () => {
  it('offers existing-project initialization once and disables it after completion', async () => {
    const wrapper = factory({ project: project({ dirName: 'a' }) })
    const init = document.body.querySelector<HTMLElement>('[data-menu-action="initialize-project"]')
    expect(init?.textContent).toContain('Initialize project')
    init?.click()
    expect(wrapper.emitted('initialize-project')).toHaveLength(1)

    document.body.innerHTML = ''
    factory({ project: project({ dirName: 'a' }), initialized: true } as Partial<Props> & { project: ProjectInfo })
    const completed = document.body.querySelector<HTMLButtonElement>('[data-menu-action="initialize-project"]')
    expect(completed?.disabled).toBe(true)
    expect(completed?.textContent).toContain('already initialized')
  })

  it('shows "Create worktree" only for a git-repo, non-worktree project', () => {
    factory({ project: project({ dirName: 'a' }), isGitRepo: true })
    expect(itemTexts().some((t) => t.includes('Create worktree'))).toBe(true)
  })

  it('hides "Create worktree" when the folder is not a git repo', () => {
    factory({ project: project({ dirName: 'a' }), isGitRepo: false })
    expect(itemTexts().some((t) => t.includes('Create worktree'))).toBe(false)
  })

  it('hides "Create worktree" on a worktree entry, showing "Delete worktree" instead', () => {
    factory({
      project: project({ dirName: 'a', worktreeName: 'feat', parentDirName: 'root' }),
      isGitRepo: true,
    })
    const texts = itemTexts()
    expect(texts.some((t) => t.includes('Create worktree'))).toBe(false)
    expect(texts.some((t) => t.includes('Delete worktree'))).toBe(true)
    expect(texts.some((t) => t === 'Delete project')).toBe(false)
  })

  it('emits create-worktree / delete-worktree from the respective items', async () => {
    // 普通 git 项目 → create-worktree
    const w1 = factory({ project: project({ dirName: 'a' }), isGitRepo: true })
    const createBtn = Array.from(document.body.querySelectorAll('.ctx-item')).find((el) =>
      el.textContent?.includes('Create worktree'),
    ) as HTMLElement
    createBtn.click()
    expect(w1.emitted('create-worktree')).toHaveLength(1)

    document.body.innerHTML = ''

    // worktree 条目 → delete-worktree
    const w2 = factory({
      project: project({ dirName: 'a', worktreeName: 'feat' }),
      isGitRepo: true,
    })
    const delBtn = Array.from(document.body.querySelectorAll('.ctx-item')).find((el) =>
      el.textContent?.includes('Delete worktree'),
    ) as HTMLElement
    delBtn.click()
    expect(w2.emitted('delete-worktree')).toHaveLength(1)
    expect(w2.emitted('delete')).toBeUndefined()
  })
})
