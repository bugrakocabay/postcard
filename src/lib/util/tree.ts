// Operations on the collection tree. Functions mutate in place (works with Svelte $state proxies).
import type { Collection, FolderItem, Item, RequestItem } from '../types'
import { uid } from './format'

export type Container = Collection | FolderItem

function walk(
  container: Container,
  visit: (item: Item, parent: Container, index: number) => boolean | void,
): boolean {
  for (let i = 0; i < container.items.length; i++) {
    const item = container.items[i]
    if (visit(item, container, i)) return true
    if (item.type === 'folder' && walk(item, visit)) return true
  }
  return false
}

export function findItem(coll: Collection, id: string): Item | null {
  let found: Item | null = null
  walk(coll, (item) => {
    if (item.id === id) {
      found = item
      return true
    }
  })
  return found
}

/** The collection itself (when id === coll.id) or a folder in it. */
export function findContainer(coll: Collection, id: string | null): Container | null {
  if (id === null || id === coll.id) return coll
  const item = findItem(coll, id)
  return item && item.type === 'folder' ? item : null
}

export function parentOf(coll: Collection, id: string): { parent: Container; index: number } | null {
  let res: { parent: Container; index: number } | null = null
  walk(coll, (item, parent, index) => {
    if (item.id === id) {
      res = { parent, index }
      return true
    }
  })
  return res
}

/** Containers from the collection down to the item's direct parent (outermost first). */
export function ancestorsOf(coll: Collection, id: string): Container[] {
  const path: Container[] = []
  const search = (container: Container): boolean => {
    path.push(container)
    for (const item of container.items) {
      if (item.id === id) return true
      if (item.type === 'folder' && search(item)) return true
    }
    path.pop()
    return false
  }
  return search(coll) ? path : []
}

export function removeItem(coll: Collection, id: string): Item | null {
  const loc = parentOf(coll, id)
  if (!loc) return null
  return loc.parent.items.splice(loc.index, 1)[0]
}

export function insertItem(container: Container, item: Item, index = container.items.length): void {
  const i = Math.max(0, Math.min(index, container.items.length))
  container.items.splice(i, 0, item)
}

/** True if `id` is `folderId` itself or inside it. */
export function isInside(coll: Collection, folderId: string, id: string): boolean {
  if (folderId === id) return true
  const folder = findItem(coll, folderId)
  if (!folder || folder.type !== 'folder') return false
  return findItem({ ...coll, items: folder.items }, id) !== null
}

/**
 * Moves an item (possibly between collections) into `target` at `index`.
 * Returns false (and changes nothing) for invalid moves, e.g. a folder into itself.
 */
export function moveItem(
  from: Collection,
  id: string,
  to: Collection,
  targetId: string | null,
  index: number,
): boolean {
  const target = findContainer(to, targetId)
  if (!target) return false
  if (from.id === to.id && targetId && targetId !== to.id && isInside(from, id, targetId)) return false
  const loc = parentOf(from, id)
  if (!loc) return false
  let i = index
  if (loc.parent === target && loc.index < index) i -= 1
  const [item] = loc.parent.items.splice(loc.index, 1)
  insertItem(target, item, i)
  return true
}

export interface RequestRef {
  collection: Collection
  request: RequestItem
  path: string[]
}

export function allRequests(collections: Collection[]): RequestRef[] {
  const out: RequestRef[] = []
  for (const collection of collections) {
    const rec = (container: Container, path: string[]) => {
      for (const item of container.items) {
        if (item.type === 'request') out.push({ collection, request: item, path })
        else rec(item, [...path, item.name])
      }
    }
    rec(collection, [collection.name])
  }
  return out
}

export function findRequest(
  collections: Collection[],
  requestId: string,
): { collection: Collection; request: RequestItem } | null {
  for (const collection of collections) {
    const item = findItem(collection, requestId)
    if (item && item.type === 'request') return { collection, request: item }
  }
  return null
}

/** Deep copy with fresh ids for the item and everything inside it. */
export function cloneWithNewIds<T extends Item>(item: T): T {
  const copy = JSON.parse(JSON.stringify(item)) as T
  const reid = (it: Item) => {
    it.id = uid()
    if (it.type === 'folder') it.items.forEach(reid)
  }
  reid(copy)
  return copy
}
