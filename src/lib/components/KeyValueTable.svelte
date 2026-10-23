<script lang="ts">
  // Editable key/value rows with enable checkboxes. A blank row at the end adds new rows.
  import type { KeyValue } from '../types'

  interface Props {
    rows: KeyValue[]
    keyPlaceholder?: string
    valuePlaceholder?: string
    keySuggestions?: string[]
    onchange?: () => void
  }
  let { rows, keyPlaceholder = 'Key', valuePlaceholder = 'Value', keySuggestions = [], onchange }: Props = $props()
  const listId = `kv-${Math.random().toString(36).slice(2)}`

  function addFrom(field: 'key' | 'value', value: string) {
    rows.push({ key: field === 'key' ? value : '', value: field === 'value' ? value : '', enabled: true })
    onchange?.()
  }

  function remove(i: number) {
    rows.splice(i, 1)
    onchange?.()
  }
</script>

<div class="kv">
  <div class="head">
    <span></span><span>{keyPlaceholder}</span><span>{valuePlaceholder}</span><span></span>
  </div>
  {#each rows as row, i (i)}
    <div class="row" class:off={!row.enabled}>
      <input type="checkbox" bind:checked={row.enabled} onchange={() => onchange?.()} title="Enabled" />
      <input class="cell mono" list={keySuggestions.length ? listId : undefined} placeholder={keyPlaceholder} bind:value={row.key} oninput={() => onchange?.()} spellcheck="false" />
      <input class="cell mono" placeholder={valuePlaceholder} bind:value={row.value} oninput={() => onchange?.()} spellcheck="false" />
      <button class="icon-btn" title="Remove" onclick={() => remove(i)}>✕</button>
    </div>
  {/each}
  <div class="row ghost">
    <span></span>
    <input class="cell mono" list={keySuggestions.length ? listId : undefined} placeholder="Add {keyPlaceholder.toLowerCase()}" value="" oninput={(e) => { const v = e.currentTarget.value; e.currentTarget.value = ''; addFrom('key', v); focusLast(e, 1) }} spellcheck="false" />
    <input class="cell mono" placeholder={valuePlaceholder} value="" oninput={(e) => { const v = e.currentTarget.value; e.currentTarget.value = ''; addFrom('value', v); focusLast(e, 2) }} spellcheck="false" />
    <span></span>
  </div>
  {#if keySuggestions.length}
    <datalist id={listId}>
      {#each keySuggestions as s (s)}<option value={s}></option>{/each}
    </datalist>
  {/if}
</div>

<script lang="ts" module>
  // After typing in the ghost row, move focus to the same column of the new row.
  export function focusLast(e: Event, col: number) {
    const ghost = (e.currentTarget as HTMLElement).closest('.ghost')
    queueMicrotask(() => {
      const row = ghost?.previousElementSibling
      const input = row?.children[col] as HTMLInputElement | undefined
      if (input) {
        input.focus()
        input.setSelectionRange(input.value.length, input.value.length)
      }
    })
  }
</script>

<style>
  .kv { display: flex; flex-direction: column; padding: 6px 10px; }
  .head, .row { display: grid; grid-template-columns: 22px minmax(0, 1fr) minmax(0, 1.6fr) 26px; gap: 4px; align-items: center; }
  .head { font-size: 11px; color: var(--faint); padding: 2px 0 4px; }
  .head span:nth-child(2), .head span:nth-child(3) { padding-left: 8px; }
  .row { min-height: 30px; }
  .row input[type='checkbox'] { justify-self: center; }
  .cell { height: 26px; padding: 0 8px; background: transparent; border: 1px solid transparent; border-radius: 4px; outline: none; color: var(--text); }
  .cell:hover { border-color: var(--border); }
  .cell:focus { border-color: var(--accent); background: var(--input); }
  .row.off .cell { color: var(--faint); text-decoration: line-through; }
  .row .icon-btn { visibility: hidden; }
  .row:hover .icon-btn { visibility: visible; }
  .ghost .cell::placeholder { color: var(--faint); }
</style>
