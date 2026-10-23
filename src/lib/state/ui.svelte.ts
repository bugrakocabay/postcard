// Transient UI state: layout sizes, open dialogs, confirm prompts, toasts.

export interface Layout {
  sidebarWidth: number
  sidebarCollapsed: boolean
  /** Fraction of the main column height used by the request editor (0..1). */
  requestRatio: number
  consoleHeight: number
}

export type Dialog =
  | { kind: 'envEditor'; envId?: string | null }
  | { kind: 'quickSearch' }
  | { kind: 'cookies' }
  | { kind: 'settings' }
  | { kind: 'curlImport' }
  | { kind: 'curlExport'; tabId: string }
  | { kind: 'saveAs'; tabId: string }
  | { kind: 'containerSettings'; collectionId: string; folderId: string | null }

export interface ConfirmRequest {
  title: string
  message: string
  confirmLabel: string
  danger: boolean
  resolve: (ok: boolean) => void
}

export interface MenuItem {
  label?: string
  action?: () => void
  danger?: boolean
  separator?: boolean
  hint?: string
}

export interface ContextMenuState {
  x: number
  y: number
  items: MenuItem[]
}

export interface Toast {
  id: number
  message: string
}

class UiState {
  layout = $state<Layout>({
    sidebarWidth: 260,
    sidebarCollapsed: false,
    requestRatio: 0.45,
    consoleHeight: 180,
  })
  dialog = $state<Dialog | null>(null)
  confirmReq = $state<ConfirmRequest | null>(null)
  toasts = $state<Toast[]>([])
  menu = $state<ContextMenuState | null>(null)
  /** Item id currently being renamed inline in the sidebar. */
  renamingId = $state<string | null>(null)
  expanded = $state<Record<string, boolean>>({})
  #toastId = 0

  open(d: Dialog): void {
    this.dialog = d
  }

  close(): void {
    this.dialog = null
  }

  confirm(opts: { title: string; message: string; confirmLabel?: string; danger?: boolean }): Promise<boolean> {
    this.confirmReq?.resolve(false)
    return new Promise((resolve) => {
      this.confirmReq = {
        title: opts.title,
        message: opts.message,
        confirmLabel: opts.confirmLabel ?? 'OK',
        danger: opts.danger ?? false,
        resolve: (ok) => {
          this.confirmReq = null
          resolve(ok)
        },
      }
    })
  }

  showMenu(e: MouseEvent, items: MenuItem[]): void {
    e.preventDefault()
    e.stopPropagation()
    this.menu = { x: e.clientX, y: e.clientY, items }
  }

  toast(message: string): void {
    const id = ++this.#toastId
    this.toasts.push({ id, message })
    setTimeout(() => {
      this.toasts = this.toasts.filter((t) => t.id !== id)
    }, 2200)
  }
}

export const ui = new UiState()
