<script lang="ts">
  // Multipart form rows: each value is text or a file picked from disk.
  import { open } from '@tauri-apps/plugin-dialog'
  import type { FormField } from '../types'

  let { rows, onchange }: { rows: FormField[]; onchange?: () => void } = $props()

  async function pick(row: FormField) {
    const path = await open({ multiple: false, directory: false })
    if (typeof path === 'string') {
      row.value = path
      onchange?.()
    }
  }

  function add() {
    rows.push({ key: '', value: '', enabled: true, kind: 'text' })
    onchange?.()
  }

  function fileName(p: string) {
    return p.split(/[\\/]/).pop() || p
  }
</script>

<div class="form">
  {#each rows as row, i (i)}
    <div class="row" class:off={!row.enabled}>
      <input type="checkbox" bind:checked={row.enabled} onchange={() => onchange?.()} />
      <input class="cell mono" placeholder="Key" bind:value={row.key} oninput={() => onchange?.()} spellcheck="false" />
      <select class="kind" bind:value={row.kind} onchange={() => { row.value = ''; onchange?.() }}>
        <option value="text">Text</option>
        <option value="file">File</option>
      </select>
      {#if row.kind === 'file'}
        <button class="cell file" onclick={() => pick(row)} title={row.value}>
          {row.value ? fileName(row.value) : 'Choose file…'}
        </button>
      {:else}
        <input class="cell mono" placeholder="Value" bind:value={row.value} oninput={() => onchange?.()} spellcheck="false" />
      {/if}
      <button class="icon-btn" title="Remove" onclick={() => { rows.splice(i, 1); onchange?.() }}>✕</button>
    </div>
  {/each}
  <button class="btn small add" onclick={add}>+ Add field</button>
</div>

<style>
  .form { padding: 6px 10px; display: flex; flex-direction: column; gap: 2px; }
  .row { display: grid; grid-template-columns: 22px minmax(0, 1fr) 70px minmax(0, 1.6fr) 26px; gap: 4px; align-items: center; min-height: 30px; }
  .row input[type='checkbox'] { justify-self: center; }
  .cell { height: 26px; padding: 0 8px; background: transparent; border: 1px solid transparent; border-radius: 4px; outline: none; color: var(--text); text-align: left; }
  .cell:hover { border-color: var(--border); }
  .cell:focus { border-color: var(--accent); background: var(--input); }
  .file { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--accent); }
  .kind { height: 26px; background: var(--input); border: 1px solid var(--border); border-radius: 4px; color: var(--text); font-size: 12px; }
  .row.off .cell { color: var(--faint); }
  .add { align-self: flex-start; margin: 6px 0 0 26px; }
</style>
