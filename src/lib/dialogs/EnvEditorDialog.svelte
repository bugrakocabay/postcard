<script lang="ts">
  import Modal from '../components/Modal.svelte'
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { uid } from '../util/format'
  import type { Variable } from '../types'

  let { envId }: { envId?: string | null } = $props()
  // 'globals' or an environment id
  // svelte-ignore state_referenced_locally
  let selected = $state<string>(envId ?? ws.envs.environments[0]?.id ?? 'globals')
  let revealed = $state<Record<number, boolean>>({})

  const env = $derived(ws.envs.environments.find((e) => e.id === selected) ?? null)
  const vars = $derived<Variable[]>(selected === 'globals' ? ws.envs.globals : (env?.variables ?? []))

  // Save only on changes, not when the dialog opens.
  let mounted = false
  $effect(() => {
    JSON.stringify(ws.envs)
    if (mounted) ws.saveEnvs()
    mounted = true
  })

  function select(id: string) {
    selected = id
    revealed = {}
  }

  function addEnv() {
    const id = uid()
    ws.envs.environments.push({ id, name: `Environment ${ws.envs.environments.length + 1}`, variables: [] })
    select(id)
  }

  function duplicateEnv() {
    if (!env) return
    const id = uid()
    ws.envs.environments.push({ id, name: `${env.name} copy`, variables: JSON.parse(JSON.stringify(env.variables)) })
    select(id)
  }

  async function deleteEnv() {
    if (!env) return
    const ok = await ui.confirm({ title: 'Delete environment?', message: `Delete "${env.name}" and its variables?`, confirmLabel: 'Delete', danger: true })
    if (!ok) return
    const id = env.id
    ws.envs.environments = ws.envs.environments.filter((e) => e.id !== id)
    if (ws.envs.activeId === id) ws.envs.activeId = null
    select(ws.envs.environments[0]?.id ?? 'globals')
  }

  function addVar(field: 'key' | 'value', v: string) {
    vars.push({ key: field === 'key' ? v : '', value: field === 'value' ? v : '', enabled: true, secret: false })
  }
</script>

<Modal title="Environments" width={880} onclose={() => ui.close()}>
  <div class="layout">
    <div class="list">
      <button class="item" class:sel={selected === 'globals'} onclick={() => select('globals')}>
        <span>Globals</span><span class="faint small">all envs</span>
      </button>
      <div class="label sect">Environments</div>
      {#each ws.envs.environments as e (e.id)}
        <button class="item" class:sel={selected === e.id} onclick={() => select(e.id)}>
          <span class="nm">{e.name}</span>
          {#if ws.envs.activeId === e.id}<span class="active-badge">active</span>{/if}
        </button>
      {/each}
      <button class="btn small add" onclick={addEnv}>+ New environment</button>
    </div>

    <div class="detail">
      {#if selected === 'globals'}
        <div class="top">
          <h3>Globals</h3>
          <span class="muted small">Used in every environment. Environment variables with the same name win.</span>
        </div>
      {:else if env}
        <div class="top">
          <input class="input name" bind:value={env.name} />
          {#if ws.envs.activeId === env.id}
            <button class="btn small" onclick={() => ws.setActiveEnv(null)}>Deactivate</button>
          {:else}
            <button class="btn small primary" onclick={() => ws.setActiveEnv(env.id)}>Set active</button>
          {/if}
          <button class="btn small" onclick={duplicateEnv}>Duplicate</button>
          <button class="btn small danger" onclick={deleteEnv}>Delete</button>
        </div>
      {:else}
        <div class="empty">Create an environment such as local, staging or prod.</div>
      {/if}

      {#if selected === 'globals' || env}
        <div class="table">
          <div class="row head">
            <span></span><span>Variable</span><span>Value</span><span title="Secret">Secret</span><span></span>
          </div>
          {#each vars as v, i (i)}
            <div class="row" class:off={!v.enabled}>
              <input type="checkbox" bind:checked={v.enabled} title="Enabled" />
              <input class="cell mono" bind:value={v.key} placeholder="name" spellcheck="false" />
              <span class="valcell">
                <input class="cell mono" type={v.secret && !revealed[i] ? 'password' : 'text'} bind:value={v.value} spellcheck="false" />
                {#if v.secret}
                  <button class="icon-btn" title={revealed[i] ? 'Hide' : 'Show'} onclick={() => (revealed[i] = !revealed[i])}>{revealed[i] ? '◉' : '◎'}</button>
                {/if}
              </span>
              <input type="checkbox" bind:checked={v.secret} title="Secret (masked on screen)" />
              <button class="icon-btn rm" title="Remove" onclick={() => vars.splice(i, 1)}>✕</button>
            </div>
          {/each}
          <div class="row ghost">
            <span></span>
            <input class="cell mono" placeholder="Add variable" value="" oninput={(e) => { const x = e.currentTarget.value; e.currentTarget.value = ''; addVar('key', x); refocus(e, 1) }} />
            <span class="valcell"><input class="cell mono" placeholder="value" value="" oninput={(e) => { const x = e.currentTarget.value; e.currentTarget.value = ''; addVar('value', x); refocus(e, 2) }} /></span>
            <span></span><span></span>
          </div>
        </div>
        <p class="faint small foot">Use as <span class="mono">{'{{name}}'}</span> in URLs, headers, auth and bodies. Secret values are hidden on screen but stored as plain text on this computer.</p>
      {/if}
    </div>
  </div>
  {#snippet footer()}
    <span class="faint small saved">Changes save automatically</span>
    <button class="btn primary" onclick={() => ui.close()}>Done</button>
  {/snippet}
</Modal>

<script lang="ts" module>
  function refocus(e: Event, col: number) {
    const ghost = (e.currentTarget as HTMLElement).closest('.ghost')
    queueMicrotask(() => {
      const row = ghost?.previousElementSibling
      const el = row?.children[col]
      const input = (el?.tagName === 'INPUT' ? el : el?.querySelector('input')) as HTMLInputElement | null
      input?.focus()
    })
  }
</script>

<style>
  .layout { display: grid; grid-template-columns: 210px 1fr; gap: 0; margin: -14px; min-height: 420px; }
  .list { border-right: 1px solid var(--border); padding: 8px; display: flex; flex-direction: column; gap: 2px; }
  .sect { padding: 12px 8px 4px; }
  .item { display: flex; justify-content: space-between; align-items: center; gap: 6px; padding: 6px 8px; background: none; border: none; border-radius: 4px; text-align: left; }
  .item:hover { background: #ffffff0a; }
  .item.sel { background: #5b8cff26; color: var(--text-strong); }
  .nm { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .active-badge { font-size: 10px; color: var(--ok); border: 1px solid #5fcf8660; border-radius: 3px; padding: 0 4px; }
  .add { margin-top: 8px; align-self: flex-start; }
  .detail { padding: 12px 14px; min-width: 0; display: flex; flex-direction: column; }
  .top { display: flex; align-items: center; gap: 8px; margin-bottom: 10px; flex-wrap: wrap; }
  .top h3 { margin: 0; font-size: 14px; }
  .name { flex: 1; font-weight: 600; min-width: 160px; }
  .small { font-size: 12px; }
  .table { display: flex; flex-direction: column; }
  .row { display: grid; grid-template-columns: 22px minmax(0, 1fr) minmax(0, 1.5fr) 46px 26px; gap: 4px; align-items: center; min-height: 30px; }
  .row.head { font-size: 11px; color: var(--faint); min-height: 22px; }
  .row.head span:nth-child(2), .row.head span:nth-child(3) { padding-left: 8px; }
  .row input[type='checkbox'] { justify-self: center; }
  .valcell { display: flex; align-items: center; min-width: 0; }
  .valcell .cell { flex: 1; min-width: 0; }
  .cell { height: 26px; padding: 0 8px; background: transparent; border: 1px solid transparent; border-radius: 4px; outline: none; color: var(--text); }
  .cell:hover { border-color: var(--border); }
  .cell:focus { border-color: var(--accent); background: var(--input); }
  .row.off .cell { color: var(--faint); text-decoration: line-through; }
  .rm { visibility: hidden; }
  .row:hover .rm { visibility: visible; }
  .foot { margin-top: 14px; }
  .saved { margin-right: auto; align-self: center; }
</style>
