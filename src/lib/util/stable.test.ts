import { expect, it } from 'vitest'
import { stableStringify } from './stable'

it('ignores key order, keeps array order', () => {
  expect(stableStringify({ b: 1, a: { d: [2, 1], c: null } })).toBe(stableStringify({ a: { c: null, d: [2, 1] }, b: 1 }))
  expect(stableStringify([1, 2])).not.toBe(stableStringify([2, 1]))
})
