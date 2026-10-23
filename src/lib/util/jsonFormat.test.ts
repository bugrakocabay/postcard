import { describe, expect, it } from 'vitest'
import { formatJson } from './jsonFormat'

describe('formatJson', () => {
  it('matches JSON.stringify for ordinary input', () => {
    const v = { a: 1, b: [1, 2, { c: 'x' }], d: {}, e: [], f: null, g: 'q"uo\\te', h: true }
    expect(formatJson(JSON.stringify(v))).toBe(JSON.stringify(v, null, 2))
  })
  it('keeps number lexemes', () => {
    expect(formatJson('{"id":12345678901234567890,"v":1.0,"x":1e3}')).toBe(
      '{\n  "id": 12345678901234567890,\n  "v": 1.0,\n  "x": 1e3\n}',
    )
  })
  it('keeps whitespace and punctuation inside strings', () => {
    expect(formatJson('{"a" : "x, {y}: [z]  "}')).toBe('{\n  "a": "x, {y}: [z]  "\n}')
  })
  it('handles top-level scalars, empty containers with spaces, and invalid input', () => {
    expect(formatJson(' 42 ')).toBe('42')
    expect(formatJson('{ "a": [ ], "b": { } }')).toBe('{\n  "a": [],\n  "b": {}\n}')
    expect(formatJson('{bad')).toBeNull()
  })
})
