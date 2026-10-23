export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`
  return `${(n / 1024 / 1024).toFixed(1)} MB`
}

export function formatMs(ms: number): string {
  return ms < 1000 ? `${ms} ms` : `${(ms / 1000).toFixed(2)} s`
}

export function statusClass(status: number): string {
  if (status >= 500) return 's-5xx'
  if (status >= 400) return 's-4xx'
  if (status >= 300) return 's-3xx'
  return 's-2xx'
}

export function methodColor(method: string): string {
  const m = method.toUpperCase()
  if (m === 'GET') return 'var(--m-get)'
  if (m === 'POST') return 'var(--m-post)'
  if (m === 'PUT') return 'var(--m-put)'
  if (m === 'PATCH') return 'var(--m-patch)'
  if (m === 'DELETE') return 'var(--m-delete)'
  return 'var(--m-other)'
}

export function shortMethod(method: string): string {
  const m = method.toUpperCase()
  if (m === 'DELETE') return 'DEL'
  if (m === 'OPTIONS') return 'OPT'
  if (m === 'PATCH') return 'PATCH'
  return m.length > 5 ? m.slice(0, 5) : m
}

export function dayLabel(ts: number, now = Date.now()): string {
  const d = new Date(ts)
  const today = new Date(now)
  const startOf = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime()
  const diff = Math.round((startOf(today) - startOf(d)) / 86400000)
  if (diff === 0) return 'Today'
  if (diff === 1) return 'Yesterday'
  return d.toLocaleDateString(undefined, { weekday: 'short', month: 'short', day: 'numeric' })
}

export function uid(): string {
  return crypto.randomUUID()
}

/** Non-negative whole number from an input's text; `fallback` when empty or invalid. */
export function toInt(text: string, fallback: number): number {
  const n = Number(text)
  return text.trim() === '' || !Number.isFinite(n) ? fallback : Math.max(0, Math.round(n))
}
