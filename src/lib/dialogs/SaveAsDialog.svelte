<script lang="ts">
  import Modal from '../components/Modal.svelte'
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { tabs } from '../state/tabs.svelte'
  import type { Item } from '../types'

  let { tabId }: { tabId: string } = $props()
  const tab = $derived(tabs.tabs.find((t) => t.id === tabId))

  let name = $state(tabs.tabs.find((t) => t.id === tabId)?.draft.name ?? 'New request')
  let target = $state(ws.collections[0] ? `${ws.collections[0].id}|` : '')
  let newCollName = $state('')

  // Flatten collections + folders into select options.
  const options = $derived.by(() => {
    const out: { value: string; label: string }[] = []
    for (const c of ws.collections) {
      out.push({ value: `${c.id}|`, label: c.name })
      const rec = (items: Item[], depth: number) => {
        for (const it of items) {
          if (it.type !== 'folder') continue
          out.push({ value: `${c.id}|${it.id}`, label: `${'   '.repeat(depth)}└ ${it.name}` })
          rec(it.items, depth + 1)
        }
      }
      rec(c.items, 1)
    }
    return out
  })

  function submit() {
    if (!tab || !name.trim()) return
    let [collId, folderId] = target.split('|')
    if (!collId) {
      const c = ws.addCollection(newCollName.trim() || 'My collection')
      collId = c.id
      folderId = ''
    }
    tabs.saveAs(tab, collId, folderId || null, name.trim())
    ui.close()
    ui.toast('Saved')
  }
</script>

<Modal title="Save request" width={460} onclose={() => ui.close()}>
  <form onsubmit={(e) => { e.preventDefault(); submit() }}>
    <label class="field">
      <span>Name</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input class="input" bind:value={name} autofocus />
    </label>
    <label class="field">
      <span>Save to</span>
      <select class="select" bind:value={target}>
        {#each options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
        <option value="">+ New collection…</option>
      </select>
    </label>
    {#if target === ''}
      <label class="field">
        <span>New collection name</span>
        <input class="input" bind:value={newCollName} placeholder="My collection" />
      </label>
    {/if}
    <button type="submit" hidden aria-hidden="true"></button>
  </form>
  {#snippet footer()}
    <button class="btn" onclick={() => ui.close()}>Cancel</button>
    <button class="btn primary" onclick={submit} disabled={!name.trim()}>Save</button>
  {/snippet}
</Modal>
