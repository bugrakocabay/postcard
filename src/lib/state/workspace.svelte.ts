// Persisted data (collections, environments, history, settings) and the console.
import { api, errorMessage } from '../api'
import type {
  Collection,
  Environment,
  EnvironmentsFile,
  HistoryEntry,
  LogLevel,
  Settings,
} from '../types'
import { uid } from '../util/format'

export interface ConsoleLine {
  id: number
  ts: number
  level: LogLevel
  message: string
}

export interface VarInfo {
  value: string
  secret: boolean
  source: 'environment' | 'global'
}

const MAX_CONSOLE = 1000
export const MAX_HISTORY = 500

function defaultSettings(): Settings {
  return {
    version: 1,
    defaultTimeoutMs: 30000,
    defaultVerifySsl: true,
    cookieJarEnabled: true,
    shell: { windows: 'wsl', wslDistro: null, unixCustom: null },
    editorFontSize: 13,
  }
}

/** Debounced jobs per key that can also be run right away (before sending, or when the window closes). */
export class Debouncer {
  #pending = new Map<string, { timer: ReturnType<typeof setTimeout>; fn: () => Promise<void> }>()

  later(key: string, ms: number, fn: () => Promise<void>): void {
    const old = this.#pending.get(key)
    if (old) clearTimeout(old.timer)
    const timer = setTimeout(() => {
      this.#pending.delete(key)
      fn()
    }, ms)
    this.#pending.set(key, { timer, fn })
  }

  has(key: string): boolean {
    return this.#pending.has(key)
  }

  /** Runs pending jobs now (all of them, or those whose key starts with `prefix`). */
  async flush(prefix = ''): Promise<void> {
    const jobs: Promise<void>[] = []
    for (const [key, job] of [...this.#pending]) {
      if (!key.startsWith(prefix)) continue
      clearTimeout(job.timer)
      this.#pending.delete(key)
      jobs.push(job.fn())
    }
    await Promise.all(jobs)
  }
}

class WorkspaceState {
  loaded = $state(false)
  collections = $state<Collection[]>([])
  envs = $state<EnvironmentsFile>({ version: 1, environments: [], globals: [], activeId: null })
  history = $state<HistoryEntry[]>([])
  settings = $state<Settings>(defaultSettings())
  consoleLines = $state<ConsoleLine[]>([])
  consoleOpen = $state(false)
  unreadConsole = $state(0)
  session: unknown = null

  #saves = new Debouncer()
  #consoleId = 0

  get activeEnv(): Environment | null {
    return this.envs.environments.find((e) => e.id === this.envs.activeId) ?? null
  }

  /** Variables visible to `{{name}}` lookups (environment overrides globals). */
  get vars(): Map<string, VarInfo> {
    const map = new Map<string, VarInfo>()
    for (const v of this.envs.globals) {
      if (v.enabled && v.key) map.set(v.key, { value: v.value, secret: v.secret, source: 'global' })
    }
    for (const v of this.activeEnv?.variables ?? []) {
      if (v.enabled && v.key) map.set(v.key, { value: v.value, secret: v.secret, source: 'environment' })
    }
    return map
  }

  async load(): Promise<void> {
    try {
      const { workspace, warnings } = await api.loadWorkspace()
      this.collections = workspace.collections.sort((a, b) => a.name.localeCompare(b.name))
      this.envs = workspace.environments
      this.history = workspace.history
      this.settings = { ...defaultSettings(), ...workspace.settings }
      this.session = workspace.session
      for (const w of warnings) this.log('warn', w)
    } catch (e) {
      this.log('error', `Could not load data: ${errorMessage(e)}`)
    }
    this.loaded = true
  }

  log(level: LogLevel, message: string): void {
    this.consoleLines.push({ id: ++this.#consoleId, ts: Date.now(), level, message })
    if (this.consoleLines.length > MAX_CONSOLE) this.consoleLines.splice(0, this.consoleLines.length - MAX_CONSOLE)
    if (!this.consoleOpen) this.unreadConsole++
    if (level === 'error' || level === 'warn') this.consoleOpen = true
  }

  clearConsole(): void {
    this.consoleLines = []
    this.unreadConsole = 0
  }

  // ---- persistence (debounced per file) ----

  saveCollection(c: Collection): void {
    const id = c.id
    this.#saves.later(`c:${id}`, 300, async () => {
      const current = this.collections.find((x) => x.id === id)
      if (!current) return
      await api.saveCollection($state.snapshot(current) as Collection).catch((e) =>
        this.log('error', `Could not save collection "${current.name}": ${errorMessage(e)}`),
      )
    })
  }

  saveEnvs(): void {
    this.#saves.later('envs', 300, async () => {
      await api.saveEnvironments($state.snapshot(this.envs) as EnvironmentsFile).catch((e) =>
        this.log('error', `Could not save environments: ${errorMessage(e)}`),
      )
    })
  }

  saveSettings(): void {
    this.#saves.later('settings', 300, async () => {
      await api.saveSettings($state.snapshot(this.settings) as Settings).catch((e) =>
        this.log('error', `Could not save settings: ${errorMessage(e)}`),
      )
    })
  }

  /** Writes environment changes now, so a send uses what is on screen. */
  flushEnvs(): Promise<void> {
    return this.#saves.flush('envs')
  }

  get envSavePending(): boolean {
    return this.#saves.has('envs')
  }

  flushAll(): Promise<void> {
    return this.#saves.flush()
  }

  // ---- collections ----

  addCollection(name: string): Collection {
    const c: Collection = {
      version: 1,
      id: uid(),
      name,
      items: [],
      auth: { type: 'none' },
      scripts: { pre: '', post: '' },
    }
    this.collections.push(c)
    this.collections.sort((a, b) => a.name.localeCompare(b.name))
    this.saveCollection(c)
    return this.collections.find((x) => x.id === c.id)!
  }

  async deleteCollection(id: string): Promise<void> {
    this.collections = this.collections.filter((c) => c.id !== id)
    try {
      await api.deleteCollection(id)
    } catch (e) {
      this.log('error', `Could not delete collection: ${errorMessage(e)}`)
    }
  }

  collection(id: string): Collection | undefined {
    return this.collections.find((c) => c.id === id)
  }

  // ---- history ----

  addHistory(entry: HistoryEntry): void {
    this.history.unshift(entry)
    if (this.history.length > MAX_HISTORY) this.history.length = MAX_HISTORY
  }

  async clearHistory(): Promise<void> {
    this.history = []
    try {
      await api.clearHistory()
    } catch (e) {
      this.log('error', `Could not clear history: ${errorMessage(e)}`)
    }
  }

  // ---- environments ----

  setActiveEnv(id: string | null): void {
    this.envs.activeId = id
    this.saveEnvs()
  }

  /** Replace environments with the copy returned after scripts changed them (already saved by Rust). */
  applyEnvsFromBackend(file: EnvironmentsFile): void {
    // If the user edited environments while the request was running, their pending save wins.
    if (this.envSavePending) {
      this.log('warn', 'Environments were edited during the request; script variable changes from it were not applied.')
      return
    }
    this.envs = file
  }
}

export const ws = new WorkspaceState()
