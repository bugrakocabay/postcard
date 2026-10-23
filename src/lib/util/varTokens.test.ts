import { expect, it } from 'vitest'
import { findVarTokens } from './varTokens'

it('finds var tokens with whitespace', () => {
  expect(findVarTokens('{{a}}/x/{{ b c }}?{{$uuid}}')).toEqual([
    { from: 0, to: 5, name: 'a' },
    { from: 8, to: 17, name: 'b c' },
    { from: 18, to: 27, name: '$uuid' },
  ])
})
it('ignores unclosed and nested braces', () => {
  expect(findVarTokens('{{a} {b}} {{}} {{x')).toEqual([])
})
