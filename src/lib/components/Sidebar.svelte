<script lang="ts">
  import TreeNode from './TreeNode.svelte'
  import RenameInput from './RenameInput.svelte'
  import HistoryList from './HistoryList.svelte'
  import { ws } from '../state/workspace.svelte'
  import { ui } from '../state/ui.svelte'
  import { dnd } from '../state/dnd.svelte'
  import {
    deleteCollection,
    duplicateCollection,
    newCollection,
    newFolderIn,
    newRequestIn,
    renameNode,
  } from '../actions.svelte'
  import type { Collection } from '../types'
  import { tabs } from '../state/tabs.svelte'
  import { allRequests } from '../util/tree'
  import { methodColor, shortMethod } from '../util/format'

  let section = $state<'collections' | 'history'>('collections')
  let filter = $state('')

  function collMenu(e: MouseEvent, c: Collection) {
    ui.showMenu(e, [
      { label: 'New request', action: () => newRequestIn(c.id, null) },
      { label: 'New folder', action: () => newFolderIn(c.id, null) },
      { separator: true },
      { label: 'Auth & scripts…', action: () => ui.open({ kind: 'containerSettings', collectionId: c.id, folderId: null }) },
      { separator: true },
      { label: 'Rename', action: () => (ui.renamingId = c.id) },
      { label: 'Duplicate', action: () => duplicateCollection(c.id) },
      { separator: true },
      { label: 'Delete', danger: true, action: () => deleteCollection(c.id) },
    ])
  }

  function collDragOver(e: DragEvent, c: Collection) {
    if (!dnd.drag) return
    e.preventDefault()
    dnd.over = { id: c.id, pos: 'inside' }
  }

  const q = $derived(filter.trim().toLowerCase())
  const matches = $derived(
    q
      ? allRequests(ws.collections).filter((m) =>
          `${m.request.name} ${m.request.url} ${m.path.join(' ')}`.toLowerCase().includes(q),
        )
      : [],
  )
</script>

<aside class="sidebar">
  <div class="switch">
    <button class:active={section === 'collections'} onclick={() => (section = 'collections')}>Collections</button>
    <button class:active={section === 'history'} onclick={() => (section = 'history')}>
      History
    </button>
  </div>

  {#if section === 'collections'}
    <div class="tools">
      <input class="input filter" placeholder="Filter requests" bind:value={filter} />
      <button class="icon-btn" title="New collection" onclick={newCollection}>+</button>
    </div>
    <div class="tree" role="tree">
      {#if q}
        {#each matches as m (m.request.id)}
          <button class="match" onclick={() => tabs.openRequest(m.collection.id, m.request.id)}>
            <span class="method" style:color={methodColor(m.request.method)}>{shortMethod(m.request.method)}</span>
            <span class="match-text">
              <span class="match-name">{m.request.name}</span>
              <span class="match-path">{m.path.join(' / ')}</span>
            </span>
          </button>
        {:else}
          <div class="empty-state">No matching requests.</div>
        {/each}
      {:else}
      {#each ws.collections as c (c.id)}
        {@const open = !!ui.expanded[c.id]}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="coll-row"
          class:drop-inside={dnd.over?.id === c.id}
          onclick={() => (ui.expanded[c.id] = !ui.expanded[c.id])}
          oncontextmenu={(e) => collMenu(e, c)}
          ondblclick={() => (ui.renamingId = c.id)}
          ondragover={(e) => collDragOver(e, c)}
          ondragleave={() => dnd.over?.id === c.id && (dnd.over = null)}
          ondrop={(e) => {
            e.preventDefault()
            ui.expanded[c.id] = true
            dnd.drop(c.id, null, c.items.length)
          }}
          onkeydown={(e) => e.key === 'F2' && (ui.renamingId = c.id)}
          tabindex="0"
          role="treeitem"
          aria-selected="false"
        >
          <span class="chev">{open ? '▾' : '▸'}</span>
          {#if ui.renamingId === c.id}
            <RenameInput
              value={c.name}
              oncommit={(v) => {
                ui.renamingId = null
                renameNode(c.id, c.id, v)
              }}
              oncancel={() => (ui.renamingId = null)}
            />
          {:else}
            <span class="coll-name" title={c.name}>{c.name}</span>
            <button class="icon-btn more" title="More" onclick={(e) => collMenu(e, c)}>⋯</button>
          {/if}
        </div>
        {#if open}
          {#each c.items as item (item.id)}
            <TreeNode collection={c} {item} depth={1} />
          {/each}
          {#if c.items.length === 0}
            <div class="hint">
              Empty. <button class="link" onclick={() => newRequestIn(c.id, null)}>Add a request</button>
            </div>
          {/if}
        {/if}
      {:else}
        <div class="empty-state">
          <p>No collections yet.</p>
          <button class="btn small" onclick={newCollection}>New collection</button>
        </div>
      {/each}
      {/if}
    </div>
  {:else}
    <HistoryList />
  {/if}
</aside>

<style>
  .sidebar { display: flex; flex-direction: column; height: 100%; background: var(--bg-2); min-width: 0; }
  .switch { display: flex; padding: 8px 8px 0; gap: 4px; border-bottom: 1px solid var(--border); }
  .switch button {
    flex: 1; background: none; border: none; border-bottom: 2px solid transparent;
    padding: 6px 0 7px; color: var(--muted); font-weight: 600; font-size: 12px;
  }
  .switch button.active { color: var(--text-strong); border-bottom-color: var(--accent); }
  .tools { display: flex; gap: 4px; padding: 8px; align-items: center; }
  .filter { flex: 1; height: 26px; font-size: 12px; }
  .tree { flex: 1; overflow: auto; padding-bottom: 20px; }
  .coll-row {
    display: flex; align-items: center; gap: 6px; height: 28px; margin: 0 4px; padding: 0 4px 0 8px;
    border-radius: 4px; cursor: pointer; font-weight: 600; color: var(--text-strong); outline: none;
  }
  .coll-row:hover { background: #ffffff0a; }
  .coll-row:focus-visible { box-shadow: inset 0 0 0 1px var(--accent); }
  .coll-row.drop-inside { background: #5b8cff33; box-shadow: inset 0 0 0 1px var(--accent); }
  .chev { width: 10px; color: var(--muted); font-size: 11px; }
  .coll-name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .more { visibility: hidden; width: 20px; height: 20px; }
  .coll-row:hover .more { visibility: visible; }
  .hint { padding: 4px 0 6px 36px; color: var(--faint); font-size: 12px; }
  .link { background: none; border: none; color: var(--accent); padding: 0; font-size: 12px; }
  .match {
    display: flex; align-items: center; gap: 6px; width: calc(100% - 8px); margin: 0 4px; padding: 4px 6px;
    background: none; border: none; border-radius: 4px; text-align: left;
  }
  .match:hover { background: #ffffff0a; }
  .method { font-family: var(--mono); font-size: 10px; font-weight: 700; width: 38px; flex-shrink: 0; text-align: right; }
  .match-text { display: flex; flex-direction: column; min-width: 0; }
  .match-name, .match-path { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .match-path { font-size: 11px; color: var(--faint); }
  .empty-state { padding: 30px 16px; text-align: center; color: var(--faint); }
</style>
