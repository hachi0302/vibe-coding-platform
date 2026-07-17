import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import { fly } from '../src/fly'

// A DOMRect-shaped anchor; fly only reads left/top/width/height.
const rect = (left: number, top: number, width: number, height: number) =>
  ({ left, top, width, height }) as DOMRect

beforeEach(() => {
  document.body.innerHTML = ''
})
afterEach(() => {
  document.body.innerHTML = ''
})

describe('fly', () => {
  it('does nothing when an anchor is missing', () => {
    expect(() => fly({ from: rect(0, 0, 10, 10), to: null, variant: 'trash' })).not.toThrow()
    expect(() => fly({ from: undefined, to: rect(0, 0, 10, 10), variant: 'restore' })).not.toThrow()
    expect(document.querySelector('.fly-orb')).toBeNull()
  })

  it('spawns a chip positioned at the source rect centre', () => {
    fly({ from: rect(10, 20, 30, 40), to: rect(0, 0, 10, 10), variant: 'trash' })

    const outer = document.querySelector<HTMLElement>('.fly-orb')
    expect(outer).not.toBeNull()
    // centre (25,40) minus half the 34px chip → top-left at (8,23)
    expect(outer!.style.left).toBe('8px')
    expect(outer!.style.top).toBe('23px')
    expect(outer!.querySelector('.fly-chip svg')).not.toBeNull()
  })

  it('tags the chip with the variant class', () => {
    fly({ from: rect(0, 0, 10, 10), to: rect(0, 0, 10, 10), variant: 'restore' })
    expect(document.querySelector('.fly-chip-restore')).not.toBeNull()
    expect(document.querySelector('.fly-chip-trash')).toBeNull()
  })

  it('accepts a live element as the target and pulses it on landing', async () => {
    const target = document.createElement('div')
    target.className = 'topbar-trash-btn'
    document.body.appendChild(target)

    fly({ from: rect(0, 0, 20, 20), to: target, variant: 'trash' })
    expect(document.querySelector('.fly-orb')).not.toBeNull()

    // the stubbed animation resolves `finished` immediately
    await Promise.resolve()
    await Promise.resolve()

    expect(document.querySelector('.fly-orb')).toBeNull()
    expect(target.classList.contains('fly-receive')).toBe(true)

    // the receive pulse clears itself after ~320ms
    await new Promise((resolve) => setTimeout(resolve, 360))
    expect(target.classList.contains('fly-receive')).toBe(false)
  })

  it('removes the chip even when the target is only a rect (no pulse)', async () => {
    fly({ from: rect(0, 0, 20, 20), to: rect(100, 100, 20, 20), variant: 'restore' })
    expect(document.querySelector('.fly-orb')).not.toBeNull()

    await Promise.resolve()
    await Promise.resolve()

    expect(document.querySelector('.fly-orb')).toBeNull()
  })
})
