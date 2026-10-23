<script lang="ts">
  import Modal from '../components/Modal.svelte'
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { api, errorMessage } from '../api'
  import type { CookieInfo } from '../types'

  let cookies = $state<CookieInfo[]>([])
  let loading = $state(true)

  async function load() {
    try {
      cookies = await api.listCookies()
    } catch (e) {
      ws.log('error', `Could not load cookies: ${errorMessage(e)}`)
    }
    loading = false
  }
  load()

  async function remove(c: CookieInfo) {
    await api.deleteCookie(c.domain, c.path, c.name)
    load()
  }

  async function clearAll() {
    const ok = await ui.confirm({ title: 'Clear cookies?', message: 'Delete all stored cookies?', confirmLabel: 'Clear', danger: true })
    if (!ok) return
    await api.clearCookies()
    load()
  }

  const byDomain = $derived.by(() => {
    const m = new Map<string, CookieInfo[]>()
    for (const c of cookies) m.set(c.domain, [...(m.get(c.domain) ?? []), c])
    return [...m.entries()].sort((a, b) => a[0].localeCompare(b[0]))
  })
</script>

<Modal title="Cookies" width={760} onclose={() => ui.close()}>
  {#if !ws.settings.cookieJarEnabled}
    <p class="warn">The cookie jar is turned off in Settings, so new cookies are not stored or sent.</p>
  {/if}
  {#if loading}
    <div class="empty">Loading…</div>
  {:else if cookies.length === 0}
    <div class="empty">No cookies stored. Cookies set by responses are kept here and sent on later requests to the same domain.</div>
  {:else}
    {#each byDomain as [domain, list] (domain)}
      <div class="domain">{domain}</div>
      <table>
        <tbody>
          {#each list as c (c.domain + c.path + c.name)}
            <tr>
              <td class="k">{c.name}</td>
              <td class="v">{c.value}</td>
              <td class="muted">{c.path}</td>
              <td class="muted">{c.expires ? new Date(c.expires).toLocaleString() : 'session'}</td>
              <td class="muted">{[c.secure && 'Secure', c.httpOnly && 'HttpOnly'].filter(Boolean).join(' ')}</td>
              <td><button class="icon-btn" title="Delete" onclick={() => remove(c)}>✕</button></td>
            </tr>
          {/each}
        </tbody>
      </table>
    {/each}
  {/if}
  {#snippet footer()}
    <button class="btn danger" onclick={clearAll} disabled={cookies.length === 0}>Clear all</button>
    <span class="grow"></span>
    <button class="btn primary" onclick={() => ui.close()}>Done</button>
  {/snippet}
</Modal>

<style>
  .warn { color: var(--warn); margin: 0 0 10px; font-size: 12px; }
  .domain { font-weight: 600; margin: 12px 0 4px; color: var(--text-strong); }
  .domain:first-child { margin-top: 0; }
  table { width: 100%; border-collapse: collapse; font-size: 12px; font-family: var(--mono); }
  td { padding: 4px 8px; border-bottom: 1px solid #ffffff08; word-break: break-all; }
  td.k { color: #8fc1ff; white-space: nowrap; }
  td.v { max-width: 260px; }
  .grow { flex: 1; }
</style>
