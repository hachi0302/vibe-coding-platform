import { afterEach, describe, expect, it } from 'vitest'
import { t } from '../src/i18n'
import { lang, setLang } from '../src/settings'

afterEach(() => setLang('en'))

describe('t', () => {
  it('returns the English string by default', () => {
    expect(t('time.today')).toBe('Today')
  })

  it('switches dictionary when the language changes', () => {
    setLang('zh')
    expect(t('time.today')).toBe('今天')
    setLang('zh-TW')
    expect(t('list.messages', { n: 3 })).toBe('3 則訊息')
    setLang('ja')
    expect(t('time.today')).toBe('今日')
  })

  it('returns the key itself when it is unknown', () => {
    expect(t('nonexistent.key')).toBe('nonexistent.key')
  })

  it('interpolates numeric variables', () => {
    expect(t('list.messages', { n: 42 })).toBe('42 messages')
  })

  it('interpolates string variables', () => {
    expect(t('sidebar.noSessions', { agent: 'Claude' })).toBe('No Claude sessions')
  })

  it('leaves the template untouched when a variable does not match a slot', () => {
    expect(t('time.today', { unused: 'x' })).toBe('Today')
  })

  it('falls back to the English dictionary for an unrecognized language', () => {
    lang.value = 'xx' as unknown as typeof lang.value
    expect(t('time.today')).toBe('Today')
  })
})
