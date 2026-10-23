<script lang="ts">
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { tabs } from '../state/tabs.svelte'
  import { allRequests, type RequestRef } from '../util/tree'
  import { methodColor, shortMethod } from '../util/format'

  let query = $state('')
  let index = $state(0)
  let input: HTMLInputElement

  $effect(() => input.focus())

  /** Subsequence match score; lower is better, -1 = no match. */
  function score(text: string, q: string): number {
    let ti = 0
    let gaps = 0
    for (const ch of q) {
      const found = text.indexOf(ch, ti)
      if (found < 0) return -1
      gaps += found - ti
      ti = found + 1
    }
    return gaps
  }

  const results = $derived.by(() => {
    const all = allRequests(ws.collections)
    const q = query.trim().toLowerCase()
    if (!q) return all.slice(0, 50)
    const scored: [number, RequestRef][] = []
    for (const r of all) {
      const name = r.request.name.toLowerCase()
      const hay = `${name} ${r.request.url.toLowerCase()} ${r.path.join(' ').toLowerCase()}`
      const s = name.includes(q) ? 0 : hay.includes(q) ? 1 : score(hay, q.replace(/\s+/g, ''))
      if (s >= 0) scored.push([s, r])
    }
    return scored.sort((a, b) => a[0] - b[0]).slice(0, 50).map(([, r]) => r)
  })

  $effect(() => {
    query
    index = 0
  })

  function choose(r: RequestRef | undefined) {
    if (!r) return
    ui.close()
    tabs.openRequest(r.collection.id, r.request.id)
  }

  function key(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault()
      index = Math.min(index + 1, results.length - 1)
    } else if (e.key === 'ArrowUp') {
      e.preventDefault()
      index = Math.max(index - 1, 0)
    } else if (e.key === 'Enter') {
      e.preventDefault()
      choose(results[index])
    } else if (e.key === 'Escape') {
      ui.close()
    }
  }

  $effect(() => {
    document.querySelector('.qs-item.sel')?.scrollIntoView({ block: 'nearest' })
  })
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="modal-backdrop" onmousedown={(e) => e.target === e.currentTarget && ui.close()}>
  <div class="modal qs">
    <input class="q" bind:this={input} bind:value={query} onkeydown={key} placeholder="Jump to a saved request…" spellcheck="false" />
    <div class="results">
      {#each results as r, i (r.request.id)}
        <button class="qs-item" class:sel={i === index} onmousemove={() => (index = i)} onclick={() => choose(r)}>
          <span class="method" style:color={methodColor(r.request.method)}>{shortMethod(r.request.method)}</span>
          <span class="text">
            <span class="name">{r.request.name}</span>
            <span class="path">{r.path.join(' / ')} · {r.request.url}</span>
          </span>
        </button>
      {:else}
        <div class="empty">{ws.collections.length ? 'No matching requests' : 'No saved requests yet'}</div>
      {/each}
    </div>
  </div>
</div>

<style>
  .qs { width: 620px; overflow: hidden; }
  .q { height: 44px; padding: 0 16px; font-size: 15px; background: transparent; border: none; border-bottom: 1px solid var(--border); outline: none; color: var(--text-strong); }
  .results { overflow: auto; max-height: 50vh; padding: 4px; }
  .qs-item { display: flex; align-items: center; gap: 10px; width: 100%; padding: 6px 10px; background: none; border: none; border-radius: 5px; text-align: left; }
  .qs-item.sel { background: #5b8cff2e; }
  .method { font-family: var(--mono); font-size: 10.5px; font-weight: 700; width: 42px; text-align: right; flex-shrink: 0; }
  .text { display: flex; flex-direction: column; min-width: 0; }
  .name { color: var(--text-strong); }
  .path { font-size: 11.5px; color: var(--faint); overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
