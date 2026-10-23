<script lang="ts">
  import UrlBar from './UrlBar.svelte'
  import KeyValueTable from './KeyValueTable.svelte'
  import AuthEditor from './AuthEditor.svelte'
  import BodyEditor from './BodyEditor.svelte'
  import ScriptsEditor from './ScriptsEditor.svelte'
  import RequestSettings from './RequestSettings.svelte'
  import { tabs, type RequestSubTab, type Tab } from '../state/tabs.svelte'
  import { applyParams } from '../util/urlParams'
  import { HEADER_NAMES } from '../util/headers'

  let { tab }: { tab: Tab } = $props()

  const enabledCount = (rows: { enabled: boolean; key: string }[]) => rows.filter((r) => r.enabled && r.key).length
  const subtabs = $derived<[RequestSubTab, string, string][]>([
    ['params', 'Params', String(enabledCount(tab.draft.params) || '')],
    ['headers', 'Headers', String(enabledCount(tab.draft.headers) || '')],
    ['auth', 'Auth', tab.draft.auth.type === 'inherit' || tab.draft.auth.type === 'none' ? '' : '•'],
    ['body', 'Body', tab.draft.body.type === 'none' ? '' : '•'],
    ['scripts', 'Scripts', tab.draft.scripts.pre.trim() || tab.draft.scripts.post.trim() ? '•' : ''],
    ['settings', 'Settings', ''],
  ])

  // Persist the session whenever the draft changes.
  $effect(() => {
    JSON.stringify(tab.draft)
    tabs.persist()
  })
</script>

<div class="request">
  <UrlBar {tab} />
  <div class="subtabs">
    {#each subtabs as [id, label, badge] (id)}
      <button class:active={tab.reqTab === id} onclick={() => (tab.reqTab = id)}>
        {label}{#if badge}<span class="count">{badge}</span>{/if}
      </button>
    {/each}
  </div>
  <div class="content">
    {#if tab.reqTab === 'params'}
      <div class="scroll">
        <KeyValueTable rows={tab.draft.params} onchange={() => (tab.draft.url = applyParams(tab.draft.url, tab.draft.params))} />
      </div>
    {:else if tab.reqTab === 'headers'}
      <div class="scroll">
        <KeyValueTable rows={tab.draft.headers} keyPlaceholder="Header" keySuggestions={HEADER_NAMES} />
      </div>
    {:else if tab.reqTab === 'auth'}
      <div class="scroll">
        <AuthEditor auth={tab.draft.auth} onchange={(a) => (tab.draft.auth = a)} />
      </div>
    {:else if tab.reqTab === 'body'}
      <BodyEditor request={tab.draft} onsend={() => tabs.send(tab)} />
    {:else if tab.reqTab === 'scripts'}
      <ScriptsEditor scripts={tab.draft.scripts} onsend={() => tabs.send(tab)} />
    {:else if tab.reqTab === 'settings'}
      <div class="scroll"><RequestSettings settings={tab.draft.settings} /></div>
    {/if}
  </div>
</div>

<style>
  .request { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--bg); }
  .content { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .scroll { flex: 1; overflow: auto; }
</style>
