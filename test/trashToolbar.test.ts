import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  exitSelectMode,
  filterTrash,
  resetTrashToolbar,
  selectMode,
  selectedTrash,
  toggleTrashSelected,
  trashProject,
  trashProjects,
  trashSearch,
  trashSort,
} from '../src/trashToolbar'
import type { TrashItem } from '../src/types'

const item = (over: Partial<TrashItem> & { trashFile: string }): TrashItem => ({
  agent: 'claude',
  projectLabel: 'proj',
  originalPath: '/orig',
  trashPath: `/trash/${over.trashFile}`,
  deletedAt: 0,
  title: 'A session',
  size: 100,
  ...over,
})

// trashToolbar holds module-level state; reset it around every test.
beforeEach(() => resetTrashToolbar())
afterEach(() => resetTrashToolbar())

describe('filterTrash', () => {
  const items = [
    item({ trashFile: 'a', title: 'Refactor parser', projectLabel: 'viewer', deletedAt: 300 }),
    item({ trashFile: 'b', title: 'Fix login bug', projectLabel: 'web', deletedAt: 100 }),
    item({ trashFile: 'c', title: 'Add tests', projectLabel: 'viewer', deletedAt: 200 }),
  ]

  it('returns every item, newest first, with no filters', () => {
    expect(filterTrash(items).map((i) => i.trashFile)).toEqual(['a', 'c', 'b'])
  })

  it('sorts oldest first when trashSort is "oldest"', () => {
    trashSort.value = 'oldest'
    expect(filterTrash(items).map((i) => i.trashFile)).toEqual(['b', 'c', 'a'])
  })

  it('filters by a title substring', () => {
    trashSearch.value = 'login'
    expect(filterTrash(items).map((i) => i.trashFile)).toEqual(['b'])
  })

  it('filters by a project-label substring', () => {
    trashSearch.value = 'web'
    expect(filterTrash(items).map((i) => i.trashFile)).toEqual(['b'])
  })

  it('matches the search case-insensitively', () => {
    trashSearch.value = 'REFACTOR'
    expect(filterTrash(items).map((i) => i.trashFile)).toEqual(['a'])
  })

  it('filters by project', () => {
    trashProject.value = 'viewer'
    expect(filterTrash(items).map((i) => i.trashFile)).toEqual(['a', 'c'])
  })

  it('combines search, project filter and sort', () => {
    trashProject.value = 'viewer'
    trashSearch.value = 'a'
    trashSort.value = 'oldest'
    // 'viewer' items whose "title + label" contains 'a': both — sorted oldest→newest
    expect(filterTrash(items).map((i) => i.trashFile)).toEqual(['c', 'a'])
  })

  it('does not mutate the input array', () => {
    const input = [...items]
    filterTrash(input)
    expect(input.map((i) => i.trashFile)).toEqual(['a', 'b', 'c'])
  })
})

describe('trashProjects', () => {
  it('returns distinct project labels, sorted', () => {
    const labels = trashProjects([
      item({ trashFile: 'a', projectLabel: 'web' }),
      item({ trashFile: 'b', projectLabel: 'api' }),
      item({ trashFile: 'c', projectLabel: 'web' }),
    ])
    expect(labels).toEqual(['api', 'web'])
  })

  it('skips blank labels', () => {
    const labels = trashProjects([
      item({ trashFile: 'a', projectLabel: '' }),
      item({ trashFile: 'b', projectLabel: '   ' }),
      item({ trashFile: 'c', projectLabel: 'real' }),
    ])
    expect(labels).toEqual(['real'])
  })
})

describe('toggleTrashSelected', () => {
  it('adds then removes a key', () => {
    toggleTrashSelected('x')
    expect(selectedTrash.value.has('x')).toBe(true)
    toggleTrashSelected('x')
    expect(selectedTrash.value.has('x')).toBe(false)
  })

  it('replaces the Set instance so refs stay reactive', () => {
    const before = selectedTrash.value
    toggleTrashSelected('y')
    expect(selectedTrash.value).not.toBe(before)
  })
})

describe('exitSelectMode', () => {
  it('turns off select mode and clears the selection', () => {
    selectMode.value = true
    toggleTrashSelected('z')
    exitSelectMode()
    expect(selectMode.value).toBe(false)
    expect(selectedTrash.value.size).toBe(0)
  })
})

describe('resetTrashToolbar', () => {
  it('restores every field to its default', () => {
    trashSearch.value = 'q'
    trashSort.value = 'oldest'
    trashProject.value = 'web'
    selectMode.value = true
    toggleTrashSelected('k')

    resetTrashToolbar()

    expect(trashSearch.value).toBe('')
    expect(trashSort.value).toBe('recent')
    expect(trashProject.value).toBe('all')
    expect(selectMode.value).toBe(false)
    expect(selectedTrash.value.size).toBe(0)
  })
})
