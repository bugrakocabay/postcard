// User actions shared by the sidebar, menus and keyboard shortcuts.
import { writeText } from '@tauri-apps/plugin-clipboard-manager'
import { api, errorMessage } from './api'
import { tabs, toRequest, blankRequest, type Tab } from './state/tabs.svelte'
import { ui } from './state/ui.svelte'
import { ws } from './state/workspace.svelte'
import type { Ancestor, Collection, Item, Request } from './types'
import { uid } from './util/format'
import { stableStringify } from './util/stable'
import { cloneWithNewIds, findContainer, findItem, insertItem, parentOf, removeItem } from './util/tree'

function requestIdsIn(item: Item): Set<string> {
  const ids = new Set<string>()
  const rec = (it: Item) => {
    if (it.type === 'request') ids.add(it.id)
    else it.items.forEach(rec)
  }
  rec(item)
  return ids
}

export function newCollection(): void {
  const c = ws.addCollection('New collection')
  ui.renamingId = c.id
}

export function newRequestIn(collectionId: string, parentId: string | null): void {
  const coll = ws.collection(collectionId)
  if (!coll) return
  const container = findContainer(coll, parentId) ?? coll
  const req = blankRequest()
  insertItem(container, { type: 'request', ...req })
  ui.expanded[collectionId] = true
  if (parentId) ui.expanded[parentId] = true
  ws.saveCollection(coll)
  tabs.openRequest(collectionId, req.id)
  ui.renamingId = req.id
}

export function newFolderIn(collectionId: string, parentId: string | null): void {
  const coll = ws.collection(collectionId)
  if (!coll) return
  const container = findContainer(coll, parentId) ?? coll
  const id = uid()
  insertItem(container, {
    type: 'folder',
    id,
    name: 'New folder',
    items: [],
    auth: { type: 'inherit' },
    scripts: { pre: '', post: '' },
  })
  ui.expanded[collectionId] = true
  if (parentId) ui.expanded[parentId] = true
  ui.expanded[id] = true
  ws.saveCollection(coll)
  ui.renamingId = id
}

export function renameNode(collectionId: string, id: string, name: string): void {
  const coll = ws.collection(collectionId)
  const trimmed = name.trim()
  if (!coll || !trimmed) return
  if (id === collectionId) {
    coll.name = trimmed
    ws.collections.sort((a, b) => a.name.localeCompare(b.name))
  } else {
    const item = findItem(coll, id)
    if (!item) return
    item.name = trimmed
    if (item.type === 'request') tabs.syncRename(id, trimmed)
  }
  ws.saveCollection(coll)
}

export function duplicateNode(collectionId: string, id: string): void {
  const coll = ws.collection(collectionId)
  if (!coll) return
  const item = findItem(coll, id)
  const loc = parentOf(coll, id)
  if (!item || !loc) return
  const copy = cloneWithNewIds(item)
  copy.name = `${item.name} copy`
  insertItem(loc.parent, copy, loc.index + 1)
  ws.saveCollection(coll)
}

export async function duplicateCollection(collectionId: string): Promise<void> {
  const coll = ws.collection(collectionId)
  if (!coll) return
  const copy: Collection = JSON.parse(JSON.stringify($state.snapshot(coll)))
  copy.id = uid()
  copy.name = `${coll.name} copy`
  copy.items = copy.items.map((i) => cloneWithNewIds(i))
  ws.collections.push(copy)
  ws.collections.sort((a, b) => a.name.localeCompare(b.name))
  ws.saveCollection(copy)
}

export async function deleteNode(collectionId: string, id: string): Promise<void> {
  const coll = ws.collection(collectionId)
  if (!coll) return
  const item = findItem(coll, id)
  if (!item) return
  const what = item.type === 'folder' ? `folder "${item.name}" and everything in it` : `"${item.name}"`
  const ok = await ui.confirm({ title: 'Delete?', message: `Delete ${what}?`, confirmLabel: 'Delete', danger: true })
  if (!ok) return
  const ids = requestIdsIn(item)
  removeItem(coll, id)
  tabs.detach(ids)
  ws.saveCollection(coll)
}

export async function deleteCollection(collectionId: string): Promise<void> {
  const coll = ws.collection(collectionId)
  if (!coll) return
  const ok = await ui.confirm({
    title: 'Delete collection?',
    message: `Delete collection "${coll.name}" and all its requests? This cannot be undone.`,
    confirmLabel: 'Delete',
    danger: true,
  })
  if (!ok) return
  const ids = new Set<string>()
  coll.items.forEach((i) => requestIdsIn(i).forEach((x) => ids.add(x)))
  tabs.detach(ids)
  await ws.deleteCollection(collectionId)
}

export function moveRequestIdsTo(ids: Set<string>, collectionId: string): void {
  tabs.retarget(ids, collectionId)
}

export { requestIdsIn }

// ---- curl ----

export async function copyAsCurl(request: Request, ancestors: Ancestor[], resolved: boolean): Promise<void> {
  try {
    const text = await api.toCurl(toRequest(request), ancestors, resolved)
    await writeText(text)
    ui.toast(resolved ? 'Copied curl (variables resolved)' : 'Copied curl (with {{placeholders}})')
  } catch (e) {
    ws.log('error', `Copy as curl failed: ${errorMessage(e)}`)
  }
}

export function looksLikeCurl(text: string): boolean {
  return /^\s*curl(\.exe)?\s/i.test(text)
}

/** Replaces the tab's request with the parsed curl command. Keeps the old one for undo. */
export async function applyCurl(tab: Tab, text: string): Promise<boolean> {
  try {
    const { request, warnings } = await api.parseCurl(text.trim())
    tab.undoDraft = toRequest(tab.draft)
    const keepName = tab.ref !== null
    const next: Request = { ...request, id: tab.draft.id, name: keepName ? tab.draft.name : request.name }
    tab.draft = next
    tab.undoGuard = stableStringify(next)
    tab.reqTab = next.body.type !== 'none' ? 'body' : next.headers.length ? 'headers' : 'params'
    for (const w of warnings) ws.log('warn', `curl import: ${w}`)
    const files =
      next.body.type === 'binary'
        ? [next.body.path]
        : next.body.type === 'multipart'
          ? next.body.fields.filter((f) => f.kind === 'file').map((f) => f.value)
          : []
    if (files.length) ws.log('warn', `This curl command uploads local files when sent: ${files.join(', ')}`)
    ui.toast('Imported curl command — Ctrl+Z to undo')
    tabs.persist()
    return true
  } catch (e) {
    ws.log('error', `Could not parse curl: ${errorMessage(e)}`)
    return false
  }
}

export function undoCurl(tab: Tab): boolean {
  if (!tab.undoDraft) return false
  // Only right after the paste; once the request was edited, Ctrl+Z is a normal undo.
  if (tab.undoGuard !== stableStringify(tab.draft)) {
    tab.undoDraft = null
    tab.undoGuard = null
    return false
  }
  tab.draft = tab.undoDraft
  tab.undoDraft = null
  tab.undoGuard = null
  tabs.persist()
  return true
}

export async function importCurlToNewTab(text: string): Promise<boolean> {
  const tab = tabs.newTab()
  const ok = await applyCurl(tab, text)
  if (ok) tab.undoDraft = null
  return ok
}

export function saveActive(): void {
  const tab = tabs.active
  if (!tab) return
  if (tabs.save(tab)) {
    ui.toast('Saved')
  } else {
    ui.open({ kind: 'saveAs', tabId: tab.id })
  }
}

export function ancestorsForCollectionItem(collectionId: string, itemId: string): Ancestor[] {
  const coll = ws.collection(collectionId)
  if (!coll) return []
  const fake: Tab = { ref: { collectionId, requestId: itemId } } as Tab
  return tabs.ancestorsFor(fake)
}
