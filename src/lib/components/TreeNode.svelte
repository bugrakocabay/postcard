<script lang="ts">
  import TreeNode from './TreeNode.svelte'
  import RenameInput from './RenameInput.svelte'
  import type { Collection, Item } from '../types'
  import { ui, type MenuItem } from '../state/ui.svelte'
  import { tabs } from '../state/tabs.svelte'
  import { dnd, type DropPos } from '../state/dnd.svelte'
  import { methodColor, shortMethod } from '../util/format'
  import {
    ancestorsForCollectionItem,
    copyAsCurl,
    deleteNode,
    duplicateNode,
    newFolderIn,
    newRequestIn,
    renameNode,
  } from '../actions.svelte'
  import { parentOf } from '../util/tree'

  interface Props {
    collection: Collection
    item: Item
    depth: number
  }
  let { collection, item, depth }: Props = $props()

  const isFolder = $derived(item.type === 'folder')
  const open = $derived(!!ui.expanded[item.id])
  const active = $derived(item.type === 'request' && tabs.active?.ref?.requestId === item.id)
  const dirty = $derived(
    item.type === 'request' && tabs.tabs.some((t) => t.ref?.requestId === item.id && tabs.isDirty(t)),
  )
  const dropPos = $derived(dnd.over?.id === item.id ? dnd.over.pos : null)

  function click() {
    if (item.type === 'folder') ui.expanded[item.id] = !open
    else tabs.openRequest(collection.id, item.id)
  }

  function menu(e: MouseEvent) {
    const items: MenuItem[] = []
    if (item.type === 'folder') {
      items.push(
        { label: 'New request', action: () => newRequestIn(collection.id, item.id) },
        { label: 'New folder', action: () => newFolderIn(collection.id, item.id) },
        { separator: true },
        {
          label: 'Auth & scripts…',
          action: () => ui.open({ kind: 'containerSettings', collectionId: collection.id, folderId: item.id }),
        },
        { separator: true },
      )
    } else {
      const req = item
      items.push(
        { label: 'Open', action: () => tabs.openRequest(collection.id, item.id) },
        {
          label: 'Copy as curl',
          action: () => copyAsCurl(req, ancestorsForCollectionItem(collection.id, req.id), true),
        },
        {
          label: 'Copy as curl (with {{vars}})',
          action: () => copyAsCurl(req, ancestorsForCollectionItem(collection.id, req.id), false),
        },
        { separator: true },
      )
    }
    items.push(
      { label: 'Rename', hint: 'F2', action: () => (ui.renamingId = item.id) },
      { label: 'Duplicate', action: () => duplicateNode(collection.id, item.id) },
      { separator: true },
      { label: 'Delete', danger: true, action: () => deleteNode(collection.id, item.id) },
    )
    ui.showMenu(e, items)
  }

  function dragOver(e: DragEvent) {
    if (!dnd.drag || dnd.drag.itemId === item.id) return
    e.preventDefault()
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect()
    const y = (e.clientY - r.top) / r.height
    let pos: DropPos
    if (isFolder) pos = y < 0.25 ? 'before' : y > 0.75 && !open ? 'after' : 'inside'
    else pos = y < 0.5 ? 'before' : 'after'
    dnd.over = { id: item.id, pos }
  }

  function drop(e: DragEvent) {
    e.preventDefault()
    const pos = dropPos
    if (!pos) return dnd.end()
    if (pos === 'inside') {
      ui.expanded[item.id] = true
      dnd.drop(collection.id, item.id, item.type === 'folder' ? item.items.length : 0)
      return
    }
    const loc = parentOf(collection, item.id)
    if (!loc) return dnd.end()
    const parentId = loc.parent.id === collection.id ? null : loc.parent.id
    dnd.drop(collection.id, parentId, pos === 'before' ? loc.index : loc.index + 1)
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="row"
  class:active
  class:drop-before={dropPos === 'before'}
  class:drop-after={dropPos === 'after'}
  class:drop-inside={dropPos === 'inside'}
  style:padding-left="{8 + depth * 14}px"
  draggable={ui.renamingId !== item.id}
  ondragstart={(e) => dnd.start(collection.id, item.id, e)}
  ondragend={() => dnd.end()}
  ondragover={dragOver}
  ondragleave={() => dnd.over?.id === item.id && (dnd.over = null)}
  ondrop={drop}
  onclick={click}
  ondblclick={() => (ui.renamingId = item.id)}
  oncontextmenu={menu}
  onkeydown={(e) => e.key === 'F2' && (ui.renamingId = item.id)}
  tabindex="0"
  role="treeitem"
  aria-selected={active}
>
  {#if item.type === 'folder'}
    <span class="chev">{open ? '▾' : '▸'}</span>
    <span class="folder-icon">▤</span>
  {:else}
    <span class="method" style:color={methodColor(item.method)}>{shortMethod(item.method)}</span>
  {/if}
  {#if ui.renamingId === item.id}
    <RenameInput
      value={item.name}
      oncommit={(v) => {
        ui.renamingId = null
        renameNode(collection.id, item.id, v)
      }}
      oncancel={() => (ui.renamingId = null)}
    />
  {:else}
    <span class="name" title={item.name}>{item.name}</span>
    {#if dirty}<span class="dot" title="Unsaved changes">●</span>{/if}
    <button class="icon-btn more" onclick={menu} title="More">⋯</button>
  {/if}
</div>

{#if item.type === 'folder' && open}
  {#each item.items as child (child.id)}
    <TreeNode {collection} item={child} depth={depth + 1} />
  {/each}
  {#if item.items.length === 0}
    <div class="hint" style:padding-left="{30 + depth * 14}px">Empty folder</div>
  {/if}
{/if}

<style>
  .row {
    display: flex; align-items: center; gap: 6px; height: 26px; padding-right: 4px;
    cursor: pointer; border-radius: 4px; margin: 0 4px; position: relative; outline: none;
  }
  .row:hover { background: #ffffff0a; }
  .row:focus-visible { box-shadow: inset 0 0 0 1px var(--accent); }
  .row.active { background: #5b8cff22; }
  .row.drop-before::before, .row.drop-after::after {
    content: ''; position: absolute; left: 0; right: 0; height: 2px; background: var(--accent);
  }
  .row.drop-before::before { top: -1px; }
  .row.drop-after::after { bottom: -1px; }
  .row.drop-inside { background: #5b8cff33; box-shadow: inset 0 0 0 1px var(--accent); }
  .chev { width: 10px; color: var(--muted); font-size: 11px; flex-shrink: 0; }
  .folder-icon { color: var(--muted); font-size: 12px; }
  .method { font-family: var(--mono); font-size: 10px; font-weight: 700; width: 38px; flex-shrink: 0; text-align: right; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .dot { color: var(--accent); font-size: 8px; }
  .more { visibility: hidden; width: 20px; height: 20px; }
  .row:hover .more { visibility: visible; }
  .hint { color: var(--faint); font-size: 12px; height: 22px; line-height: 22px; font-style: italic; }
</style>
