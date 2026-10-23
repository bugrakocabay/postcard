import { describe, expect, it } from 'vitest'
import { applyParams, mergeParams, parseQuery } from './urlParams'

describe('urlParams', () => {
  it('parses query with vars and encoded values raw', () => {
    expect(parseQuery('{{baseUrl}}/users?id={{id}}&q=a%20b&flag#top')).toEqual([
      { key: 'id', value: '{{id}}', enabled: true },
      { key: 'q', value: 'a%20b', enabled: true },
      { key: 'flag', value: '', enabled: true },
    ])
  })
  it('returns empty for url without query', () => {
    expect(parseQuery('http://x/y')).toEqual([])
    expect(parseQuery('http://x/y?')).toEqual([])
  })
  it('keeps duplicate keys', () => {
    expect(parseQuery('/a?t=1&t=2').map((p) => p.value)).toEqual(['1', '2'])
  })
  it('applies enabled params and keeps hash', () => {
    const url = applyParams('http://x/a?old=1#h', [
      { key: 'a', value: '{{v}}', enabled: true },
      { key: 'b', value: '2', enabled: false },
      { key: 'c', value: '', enabled: true },
    ])
    expect(url).toBe('http://x/a?a={{v}}&c#h')
  })
  it('removes ? when no enabled params', () => {
    expect(applyParams('http://x/a?old=1', [])).toBe('http://x/a')
  })
  it('merge keeps disabled rows', () => {
    const merged = mergeParams('/a?x=1', [
      { key: 'x', value: '0', enabled: true },
      { key: 'off', value: '1', enabled: false },
    ])
    expect(merged).toEqual([
      { key: 'x', value: '1', enabled: true },
      { key: 'off', value: '1', enabled: false },
    ])
  })
})
