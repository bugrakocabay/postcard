// Open request tabs, drafts, sending, and session persistence.
import { api, errorMessage } from '../api'
import type { Ancestor, Item, Request, RequestItem, SendResult } from '../types'
import { uid } from '../util/format'
import { stableStringify } from '../util/stable'
import { ancestorsOf, findContainer, findRequest, insertItem } from '../util/tree'
import { ui, type Layout } from './ui.svelte'
import { ws } from './workspace.svelte'

export type RequestSubTab = 'params' | 'headers' | 'auth' | 'body' | 'scripts' | 'settings'
export type ResponseSubTab = 'body' | 'headers' | 'cookies'

export interface TabRef {
  collectionId: string
  requestId: string
}

export interface Tab {
  id: string
  ref: TabRef | null
  draft: Request
  /** JSON of the draft as last saved/opened; used for the dirty check. */
  saved: string
  sending: boolean
  sendId: string | null
  result: SendResult | null
  /** Error from the IPC call itself (not an HTTP error). */
  error: string | null
  /** Request before the last curl paste, for undo. */
  undoDraft: Request | null
  /** The draft right after the curl paste; undo only applies while the draft is unchanged. */
  undoGuard: string | null
  reqTab: RequestSubTab
  resTab: ResponseSubTab
}

interface SessionTab {
  id: string
  ref: TabRef | null
  draft: Request
  saved: string
  reqTab: RequestSubTab
  resTab: ResponseSubTab
}

interface Session {
  version: 1
  tabs: SessionTab[]
  activeId: string | null
  layout: Layout
  expanded: Record<string, boolean>
}

export function blankRequest(): Request {
  return {
    id: uid(),
    name: 'New request',
    method: 'GET',
    url: '',
    params: [],
    headers: [],
    auth: { type: 'inherit' },
    body: { type: 'none' },
    scripts: { pre: '', post: '' },
    settings: {
      timeoutMs: ws.settings.defaultTimeoutMs,
      followRedirects: true,
      verifySsl: ws.settings.defaultVerifySsl,
    },
  }
}

/** Plain deep copy of a request (drops the tree's `type` tag). */
export function toRequest(r: Request | RequestItem): Request {
  const { type: _t, ...rest } = $state.snapshot(r) as RequestItem
  return JSON.parse(JSON.stringify(rest)) as Request
}

class TabsState {
  tabs = $state<Tab[]>([])
  activeId = $state<string | null>(null)

  get active(): Tab | null {
    return this.tabs.find((t) => t.id === this.activeId) ?? null
  }

  #make(draft: Request, ref: TabRef | null): Tab {
    return {
      id: uid(),
      ref,
      draft,
      saved: stableStringify(draft),
      sending: false,
      sendId: null,
      result: null,
      error: null,
      undoDraft: null,
      undoGuard: null,
      reqTab: 'params',
      resTab: 'body',
    }
  }

  isDirty(tab: Tab): boolean {
    return stableStringify(tab.draft) !== tab.saved
  }

  activate(id: string): void {
    this.activeId = id
    this.persist()
  }

  newTab(draft: Request = blankRequest()): Tab {
    const tab = this.#make(draft, null)
    this.tabs.push(tab)
    this.activeId = tab.id
    this.persist()
    return this.tabs[this.tabs.length - 1]
  }

  openRequest(collectionId: string, requestId: string): void {
    const existing = this.tabs.find((t) => t.ref?.requestId === requestId)
    if (existing) {
      this.activate(existing.id)
      return
    }
    const coll = ws.collection(collectionId)
    const found = coll ? findRequest([coll], requestId) : null
    if (!found) return
    const tab = this.#make(toRequest(found.request), { collectionId, requestId })
    // Replace the active tab if it is an untouched blank tab.
    const act = this.active
    if (act && !act.ref && !this.isDirty(act) && act.draft.url === '' && !act.result) {
      const i = this.tabs.indexOf(act)
      this.tabs[i] = tab
    } else {
      this.tabs.push(tab)
    }
    this.activeId = tab.id
    this.persist()
  }

  /** Opens a copy of a request as a new unsaved tab (e.g. from history). */
  openCopy(request: Request): void {
    const draft = toRequest(request)
    draft.id = uid()
    this.newTab(draft)
  }

  async close(id: string): Promise<void> {
    const tab = this.tabs.find((t) => t.id === id)
    if (!tab) return
    if (this.isDirty(tab)) {
      const ok = await ui.confirm({
        title: 'Close tab?',
        message: `"${tab.draft.name}" has unsaved changes. Close it anyway?`,
        confirmLabel: 'Discard changes',
        danger: true,
      })
      if (!ok) return
    }
    if (tab.sending && tab.sendId) api.cancelRequest(tab.sendId).catch(() => {})
    const i = this.tabs.findIndex((t) => t.id === id)
    this.tabs.splice(i, 1)
    if (this.activeId === id) {
      const next = this.tabs[Math.min(i, this.tabs.length - 1)]
      this.activeId = next?.id ?? null
    }
    this.persist()
  }

  // ---- saving ----

  /** Saves into the collection. Returns false if the tab has no saved location yet (caller shows Save As). */
  save(tab: Tab): boolean {
    if (!tab.ref) return false
    const coll = ws.collection(tab.ref.collectionId)
    const found = coll ? findRequest([coll], tab.ref.requestId) : null
    if (!coll || !found) {
      tab.ref = null
      return false
    }
    const data = toRequest(tab.draft)
    data.id = found.request.id
    Object.assign(found.request, data)
    tab.saved = stableStringify(tab.draft)
    ws.saveCollection(coll)
    this.persist()
    return true
  }

  saveAs(tab: Tab, collectionId: string, parentId: string | null, name: string): void {
    const coll = ws.collection(collectionId)
    if (!coll) return
    const container = findContainer(coll, parentId) ?? coll
    const data = toRequest(tab.draft)
    data.id = uid()
    data.name = name
    const item: Item = { type: 'request', ...data }
    insertItem(container, item)
    tab.draft.id = data.id
    tab.draft.name = name
    tab.ref = { collectionId, requestId: data.id }
    tab.saved = stableStringify(tab.draft)
    ws.saveCollection(coll)
    ui.expanded[collectionId] = true
    if (parentId) ui.expanded[parentId] = true
    this.persist()
  }

  /** Keeps open tabs in step with a rename in the sidebar. */
  syncRename(requestId: string, name: string): void {
    for (const t of this.tabs) {
      if (t.ref?.requestId === requestId) {
        t.draft.name = name
        const saved = JSON.parse(t.saved) as Request
        saved.name = name
        t.saved = stableStringify(saved)
      }
    }
    this.persist()
  }

  /** Requests were deleted: their tabs become unsaved drafts. */
  detach(requestIds: Set<string>): void {
    for (const t of this.tabs) {
      if (t.ref && requestIds.has(t.ref.requestId)) {
        t.ref = null
        t.saved = ''
      }
    }
    this.persist()
  }

  /** A request moved to another collection. */
  retarget(requestIds: Set<string>, collectionId: string): void {
    for (const t of this.tabs) {
      if (t.ref && requestIds.has(t.ref.requestId)) t.ref.collectionId = collectionId
    }
    this.persist()
  }

  // ---- sending ----

  ancestorsFor(tab: Tab): Ancestor[] {
    if (!tab.ref) return []
    const coll = ws.collection(tab.ref.collectionId)
    if (!coll) return []
    return ancestorsOf(coll, tab.ref.requestId).map((c) => ({
      auth: $state.snapshot(c.auth),
      scripts: $state.snapshot(c.scripts),
    })) as Ancestor[]
  }

  async send(tab: Tab): Promise<void> {
    if (tab.sending) return
    if (!tab.draft.url.trim()) {
      ws.log('warn', 'Enter a URL first.')
      return
    }
    const sendId = uid()
    tab.sending = true
    tab.sendId = sendId
    tab.error = null
    const tabId = tab.id
    try {
      await ws.flushEnvs()
      const result = await api.sendRequest({
        sendId,
        request: toRequest(tab.draft),
        ancestors: this.ancestorsFor(tab),
      })
      const t = this.tabs.find((x) => x.id === tabId)
      for (const l of result.logs) ws.log(l.level, l.message)
      const he = result.historyEntry
      if (result.response) {
        ws.log('info', `${he.method} ${he.url} → ${result.response.status} ${result.response.statusText} (${result.response.timeMs} ms)`)
      } else if (result.error) {
        ws.log('error', `${he.method} ${he.url} → ${result.error}`)
      }
      if (result.environments) ws.applyEnvsFromBackend(result.environments)
      ws.addHistory(he)
      if (t && t.sendId === sendId) t.result = result
    } catch (e) {
      const t = this.tabs.find((x) => x.id === tabId)
      if (t) t.error = errorMessage(e)
      ws.log('error', errorMessage(e))
    } finally {
      const t = this.tabs.find((x) => x.id === tabId)
      if (t && t.sendId === sendId) {
        t.sending = false
        t.sendId = null
      }
    }
  }

  cancel(tab: Tab): void {
    if (tab.sendId) api.cancelRequest(tab.sendId).catch(() => {})
  }

  // ---- session ----

  #timer: ReturnType<typeof setTimeout> | undefined

  /** Saves the session now (used when the window closes). */
  async flushSession(): Promise<void> {
    if (this.#timer === undefined) return
    clearTimeout(this.#timer)
    this.#timer = undefined
    await this.#writeSession()
  }

  persist(): void {
    clearTimeout(this.#timer)
    this.#timer = setTimeout(() => {
      this.#timer = undefined
      this.#writeSession()
    }, 500)
  }

  #writeSession(): Promise<void> {
    {
      const session: Session = {
        version: 1,
        tabs: this.tabs.map((t) => ({
          id: t.id,
          ref: $state.snapshot(t.ref),
          draft: $state.snapshot(t.draft) as Request,
          saved: t.saved,
          reqTab: t.reqTab,
          resTab: t.resTab,
        })),
        activeId: this.activeId,
        layout: $state.snapshot(ui.layout),
        expanded: $state.snapshot(ui.expanded),
      }
      return api.saveSession(session).catch(() => {})
    }
  }

  restore(raw: unknown): void {
    const s = raw as Partial<Session> | null
    if (s && typeof s === 'object') {
      if (s.layout) ui.layout = { ...ui.layout, ...s.layout }
      if (s.expanded) ui.expanded = s.expanded
      for (const st of s.tabs ?? []) {
        if (!st?.draft) continue
        const tab = this.#make(st.draft, null)
        tab.id = st.id ?? tab.id
        tab.saved = st.saved ?? ''
        tab.reqTab = st.reqTab ?? 'params'
        tab.resTab = st.resTab ?? 'body'
        if (st.ref) {
          const coll = ws.collection(st.ref.collectionId)
          if (coll && findRequest([coll], st.ref.requestId)) tab.ref = st.ref
          else tab.saved = ''
        }
        this.tabs.push(tab)
      }
      if (s.activeId && this.tabs.some((t) => t.id === s.activeId)) this.activeId = s.activeId
    }
    if (this.tabs.length === 0) this.newTab()
    if (!this.activeId) this.activeId = this.tabs[0].id
  }
}

export const tabs = new TabsState()
