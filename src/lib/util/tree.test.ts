import { describe, expect, it } from 'vitest'
import type { Collection, Item } from '../types'
import {
  allRequests,
  ancestorsOf,
  cloneWithNewIds,
  findContainer,
  findItem,
  moveItem,
  removeItem,
} from './tree'

const req = (id: string): Item => ({
  type: 'request',
  id,
  name: id,
  method: 'GET',
  url: '',
  params: [],
  headers: [],
  auth: { type: 'inherit' },
  body: { type: 'none' },
  scripts: { pre: '', post: '' },
  settings: { timeoutMs: 30000, followRedirects: true, verifySsl: true },
})
const folder = (id: string, items: Item[]): Item => ({
  type: 'folder',
  id,
  name: id,
  items,
  auth: { type: 'inherit' },
  scripts: { pre: '', post: '' },
})
const coll = (id: string, items: Item[]): Collection => ({
  version: 1,
  id,
  name: id,
  items,
  auth: { type: 'none' },
  scripts: { pre: '', post: '' },
})

const sample = () => coll('c', [req('r1'), folder('f1', [req('r2'), folder('f2', [req('r3')])])])

describe('tree', () => {
  it('finds nested items and containers', () => {
    const c = sample()
    expect(findItem(c, 'r3')?.id).toBe('r3')
    expect(findItem(c, 'nope')).toBeNull()
    expect(findContainer(c, null)).toBe(c)
    expect(findContainer(c, 'f2')?.id).toBe('f2')
    expect(findContainer(c, 'r1')).toBeNull()
  })
  it('ancestors outermost first', () => {
    expect(ancestorsOf(sample(), 'r3').map((a) => a.id)).toEqual(['c', 'f1', 'f2'])
    expect(ancestorsOf(sample(), 'r1').map((a) => a.id)).toEqual(['c'])
    expect(ancestorsOf(sample(), 'zz')).toEqual([])
  })
  it('removes items', () => {
    const c = sample()
    expect(removeItem(c, 'r2')?.id).toBe('r2')
    expect(findItem(c, 'r2')).toBeNull()
  })
  it('moves within and across collections', () => {
    const c = sample()
    expect(moveItem(c, 'r3', c, null, 0)).toBe(true)
    expect(c.items[0].id).toBe('r3')
    const d = coll('d', [])
    expect(moveItem(c, 'f1', d, null, 0)).toBe(true)
    expect(findItem(d, 'r2')?.id).toBe('r2')
    expect(findItem(c, 'f1')).toBeNull()
  })
  it('reorders within same parent accounting for removal', () => {
    const c = coll('c', [req('a'), req('b'), req('c')])
    moveItem(c, 'a', c, null, 3)
    expect(c.items.map((i) => i.id)).toEqual(['b', 'c', 'a'])
  })
  it('refuses moving a folder into itself or a descendant', () => {
    const c = sample()
    expect(moveItem(c, 'f1', c, 'f2', 0)).toBe(false)
    expect(moveItem(c, 'f1', c, 'f1', 0)).toBe(false)
    expect(findItem(c, 'f1')).not.toBeNull()
  })
  it('lists all requests with paths', () => {
    const refs = allRequests([sample()])
    expect(refs.map((r) => [r.request.id, r.path.join('/')])).toEqual([
      ['r1', 'c'],
      ['r2', 'c/f1'],
      ['r3', 'c/f1/f2'],
    ])
  })
  it('clones with new ids', () => {
    const f = folder('f', [req('x')])
    const copy = cloneWithNewIds(f)
    expect(copy.id).not.toBe('f')
    expect((copy as any).items[0].id).not.toBe('x')
    expect((copy as any).items[0].name).toBe('x')
  })
})
