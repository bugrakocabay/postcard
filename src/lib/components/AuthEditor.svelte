<script lang="ts">
  import type { Auth } from '../types'

  interface Props {
    auth: Auth
    /** Collections can't inherit. */
    allowInherit?: boolean
    onchange: (auth: Auth) => void
  }
  let { auth, allowInherit = true, onchange }: Props = $props()
  let reveal = $state(false)

  function setType(type: Auth['type']) {
    if (type === auth.type) return
    switch (type) {
      case 'none':
      case 'inherit':
        onchange({ type })
        break
      case 'bearer':
        onchange({ type, token: '' })
        break
      case 'basic':
        onchange({ type, username: '', password: '' })
        break
      case 'apiKey':
        onchange({ type, key: 'X-API-Key', value: '', location: 'header' })
        break
    }
  }
</script>

<div class="auth">
  <label class="field row">
    <span>Type</span>
    <select class="select" value={auth.type} onchange={(e) => setType(e.currentTarget.value as Auth['type'])}>
      {#if allowInherit}<option value="inherit">Inherit from parent</option>{/if}
      <option value="none">No auth</option>
      <option value="bearer">Bearer token</option>
      <option value="basic">Basic auth</option>
      <option value="apiKey">API key</option>
    </select>
  </label>

  {#if auth.type === 'inherit'}
    <p class="muted">Uses the auth of the nearest folder or collection that sets one.</p>
  {:else if auth.type === 'none'}
    <p class="muted">No Authorization header is added.</p>
  {:else if auth.type === 'bearer'}
    <label class="field row">
      <span>Token</span>
      <input class="input mono" bind:value={auth.token} placeholder={'{{token}}'} spellcheck="false" />
    </label>
  {:else if auth.type === 'basic'}
    <label class="field row">
      <span>Username</span>
      <input class="input mono" bind:value={auth.username} spellcheck="false" />
    </label>
    <label class="field row">
      <span>Password</span>
      <span class="pw">
        <input class="input mono" type={reveal ? 'text' : 'password'} bind:value={auth.password} spellcheck="false" />
        <button class="btn small" onclick={() => (reveal = !reveal)}>{reveal ? 'Hide' : 'Show'}</button>
      </span>
    </label>
  {:else if auth.type === 'apiKey'}
    <label class="field row">
      <span>Key</span>
      <input class="input mono" bind:value={auth.key} spellcheck="false" />
    </label>
    <label class="field row">
      <span>Value</span>
      <input class="input mono" bind:value={auth.value} placeholder={'{{apiKey}}'} spellcheck="false" />
    </label>
    <label class="field row">
      <span>Add to</span>
      <select class="select" bind:value={auth.location}>
        <option value="header">Header</option>
        <option value="query">Query params</option>
      </select>
    </label>
  {/if}
  <p class="faint tip">Values can use {'{{variables}}'}.</p>
</div>

<style>
  .auth { padding: 12px 14px; max-width: 620px; }
  .row { display: grid; grid-template-columns: 100px 1fr; align-items: center; gap: 10px; flex-direction: row; }
  .row > span:first-child { color: var(--muted); font-size: 12px; }
  .pw { display: flex; gap: 6px; }
  .pw .input { flex: 1; }
  p { margin: 4px 0 12px 110px; font-size: 12px; }
  .tip { margin-top: 16px; }
</style>
