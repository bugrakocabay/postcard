// Keeps the Params table and the URL's query string in sync.
// The URL is the source of truth for enabled params; values are kept raw (not decoded).
import type { KeyValue } from '../types'

function splitUrl(url: string): { base: string; query: string | null; hash: string } {
  let hash = ''
  let rest = url
  const h = rest.indexOf('#')
  if (h >= 0) {
    hash = rest.slice(h)
    rest = rest.slice(0, h)
  }
  const q = rest.indexOf('?')
  if (q < 0) return { base: rest, query: null, hash }
  return { base: rest.slice(0, q), query: rest.slice(q + 1), hash }
}

/** Parses the query string of `url` into enabled params. */
export function parseQuery(url: string): KeyValue[] {
  const { query } = splitUrl(url)
  if (!query) return []
  return query
    .split('&')
    .filter((part) => part.length > 0)
    .map((part) => {
      const eq = part.indexOf('=')
      return eq < 0
        ? { key: part, value: '', enabled: true }
        : { key: part.slice(0, eq), value: part.slice(eq + 1), enabled: true }
    })
}

/** Rebuilds `url`'s query string from the enabled params. */
export function applyParams(url: string, params: KeyValue[]): string {
  const { base, hash } = splitUrl(url)
  const parts = params
    .filter((p) => p.enabled && (p.key !== '' || p.value !== ''))
    .map((p) => (p.value === '' ? p.key : `${p.key}=${p.value}`))
  return parts.length ? `${base}?${parts.join('&')}${hash}` : `${base}${hash}`
}

/** New params list after the URL changed: query params from the URL, then previously disabled rows. */
export function mergeParams(url: string, previous: KeyValue[]): KeyValue[] {
  return [...parseQuery(url), ...previous.filter((p) => !p.enabled)]
}
