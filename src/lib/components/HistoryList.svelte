<script lang="ts">
  import { ws } from '../state/workspace.svelte'
  import { tabs } from '../state/tabs.svelte'
  import { ui } from '../state/ui.svelte'
  import { dayLabel, methodColor, shortMethod, statusClass } from '../util/format'
  import type { HistoryEntry } from '../types'

  const groups = $derived.by(() => {
    const out: { label: string; entries: HistoryEntry[] }[] = []
    for (const h of ws.history) {
      const label = dayLabel(h.timestamp)
      const last = out[out.length - 1]
      if (last && last.label === label) last.entries.push(h)
      else out.push({ label, entries: [h] })
    }
    return out
  })

  async function clear() {
    const ok = await ui.confirm({
      title: 'Clear history?',
      message: `Remove all ${ws.history.length} history entries?`,
      confirmLabel: 'Clear',
      danger: true,
    })
    if (ok) ws.clearHistory()
  }

  function time(ts: number) {
    return new Date(ts).toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' })
  }
</script>

<div class="history">
  <div class="tools">
    <span class="muted">{ws.history.length} requests</span>
    <button class="btn small" onclick={clear} disabled={ws.history.length === 0}>Clear</button>
  </div>
  <div class="list">
    {#each groups as g (g.label)}
      <div class="day label">{g.label}</div>
      {#each g.entries as h (h.id)}
        <button
          class="entry"
          title={h.url}
          onclick={() => tabs.openCopy(h.request)}
          oncontextmenu={(e) =>
            ui.showMenu(e, [
              { label: 'Open in new tab', action: () => tabs.openCopy(h.request) },
              {
                label: 'Save to collection…',
                action: () => {
                  tabs.openCopy(h.request)
                  if (tabs.active) ui.open({ kind: 'saveAs', tabId: tabs.active.id })
                },
              },
            ])}
        >
          <span class="method" style:color={methodColor(h.method)}>{shortMethod(h.method)}</span>
          <span class="url">{h.url.replace(/^https?:\/\//, '')}</span>
          <span class="meta">
            {#if h.status}<span class={statusClass(h.status)}>{h.status}</span>{:else}<span class="s-5xx">ERR</span>{/if}
            <span class="faint">{time(h.timestamp)}</span>
          </span>
        </button>
      {/each}
    {:else}
      <div class="empty">Requests you send show up here.</div>
    {/each}
  </div>
</div>

<style>
  .history { display: flex; flex-direction: column; flex: 1; min-height: 0; }
  .tools { display: flex; justify-content: space-between; align-items: center; padding: 8px 10px; font-size: 12px; }
  .list { flex: 1; overflow: auto; padding-bottom: 20px; }
  .day { padding: 10px 12px 4px; }
  .entry {
    display: flex; align-items: center; gap: 6px; width: calc(100% - 8px); margin: 0 4px; height: 26px;
    padding: 0 6px; background: none; border: none; border-radius: 4px; text-align: left;
  }
  .entry:hover { background: #ffffff0a; }
  .method { font-family: var(--mono); font-size: 10px; font-weight: 700; width: 38px; flex-shrink: 0; text-align: right; }
  .url { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 12px; }
  .meta { display: flex; gap: 6px; font-size: 11px; font-family: var(--mono); flex-shrink: 0; }
</style>
