<script lang="ts">
  import VarInput from '../editor/VarInput.svelte'
  import { tabs, type Tab } from '../state/tabs.svelte'
  import { applyCurl, looksLikeCurl, undoCurl } from '../actions.svelte'
  import { mergeParams } from '../util/urlParams'
  import { methodColor } from '../util/format'

  let { tab }: { tab: Tab } = $props()
  const METHODS = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS']
  let customMethod = $state(false)

  function setUrl(v: string) {
    tab.draft.url = v
    tab.draft.params = mergeParams(v, tab.draft.params)
  }

  function onpaste(text: string): boolean {
    if (!looksLikeCurl(text)) return false
    applyCurl(tab, text)
    return true
  }

  function onMethod(e: Event) {
    const v = (e.currentTarget as HTMLSelectElement).value
    if (v === '__custom') {
      customMethod = true
      return
    }
    tab.draft.method = v
  }
</script>

<div class="urlbar">
  {#if customMethod || !METHODS.includes(tab.draft.method)}
    <input
      class="method custom mono"
      style:color={methodColor(tab.draft.method)}
      value={tab.draft.method}
      oninput={(e) => (tab.draft.method = e.currentTarget.value.toUpperCase().replace(/\s/g, ''))}
      onblur={() => {
        customMethod = false
        if (!tab.draft.method) tab.draft.method = 'GET'
      }}
      placeholder="METHOD"
      spellcheck="false"
    />
  {:else}
    <select class="method mono" style:color={methodColor(tab.draft.method)} value={tab.draft.method} onchange={onMethod}>
      {#each METHODS as m (m)}<option value={m}>{m}</option>{/each}
      <option value="__custom">Custom…</option>
    </select>
  {/if}
  <div class="url">
    <VarInput
      value={tab.draft.url}
      placeholder="Enter URL or paste a curl command"
      onchange={setUrl}
      {onpaste}
      onenter={() => tabs.send(tab)}
      onundo={() => undoCurl(tab)}
    />
  </div>
  {#if tab.sending}
    <button class="btn send cancel" onclick={() => tabs.cancel(tab)}>Cancel</button>
  {:else}
    <button class="btn primary send" onclick={() => tabs.send(tab)} title="Send (Ctrl+Enter)">Send</button>
  {/if}
</div>

<style>
  .urlbar { display: flex; gap: 6px; padding: 10px 12px; align-items: stretch; }
  .method {
    width: 104px; height: 32px; padding: 0 8px; font-weight: 700; font-size: 12.5px;
    background: var(--input); border: 1px solid var(--border); border-radius: var(--radius); outline: none;
  }
  .method:focus { border-color: var(--accent); }
  .method option { color: var(--text); }
  .url {
    flex: 1; min-width: 0; display: flex; align-items: center; height: 32px;
    background: var(--input); border: 1px solid var(--border); border-radius: var(--radius); overflow: hidden;
  }
  .url:focus-within { border-color: var(--accent); }
  .send { height: 32px; min-width: 84px; justify-content: center; }
  .cancel { background: #3a2a2a; border-color: #6b3131; }
</style>
