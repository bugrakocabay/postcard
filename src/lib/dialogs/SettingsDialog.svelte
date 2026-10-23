<script lang="ts">
  import Modal from '../components/Modal.svelte'
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { toInt } from '../util/format'

  const isWindows = navigator.userAgent.includes('Windows')
  const s = ws.settings

  // Save only on changes, not when the dialog opens.
  let mounted = false
  $effect(() => {
    JSON.stringify(ws.settings)
    if (mounted) ws.saveSettings()
    mounted = true
  })
</script>

<Modal title="Settings" width={520} onclose={() => ui.close()}>
  <div class="label">Requests</div>
  <label class="field">
    <span>Default timeout for new requests (ms)</span>
    <input class="input" type="number" min="0" step="1000" value={s.defaultTimeoutMs} oninput={(e) => (s.defaultTimeoutMs = toInt(e.currentTarget.value, 30000))} />
  </label>
  <label class="check row"><input type="checkbox" bind:checked={s.defaultVerifySsl} /> Verify SSL certificates by default</label>
  <label class="check row"><input type="checkbox" bind:checked={s.cookieJarEnabled} /> Store cookies and send them on later requests</label>

  <div class="label">Scripts — pm.shell()</div>
  {#if isWindows}
    <label class="field">
      <span>Run commands with</span>
      <select class="select" bind:value={s.shell.windows}>
        <option value="wsl">WSL (bash in your default distro)</option>
        <option value="cmd">Command Prompt (cmd)</option>
        <option value="powerShell">PowerShell</option>
      </select>
    </label>
    {#if s.shell.windows === 'wsl'}
      <label class="field">
        <span>WSL distro (empty = default)</span>
        <input class="input" value={s.shell.wslDistro ?? ''} oninput={(e) => (s.shell.wslDistro = e.currentTarget.value.trim() || null)} placeholder="Ubuntu" />
      </label>
    {/if}
  {:else}
    <label class="field">
      <span>Shell (empty = your shell, $SHELL -ilc)</span>
      <input class="input mono" value={s.shell.unixCustom ?? ''} oninput={(e) => (s.shell.unixCustom = e.currentTarget.value.trim() || null)} placeholder="/bin/zsh" />
    </label>
  {/if}

  <div class="label">Editor</div>
  <label class="field">
    <span>Font size</span>
    <input class="input" type="number" min="9" max="24" value={s.editorFontSize} oninput={(e) => (s.editorFontSize = Math.min(24, Math.max(9, toInt(e.currentTarget.value, 13))))} />
  </label>

  {#snippet footer()}
    <span class="faint saved">Changes save automatically</span>
    <button class="btn primary" onclick={() => ui.close()}>Done</button>
  {/snippet}
</Modal>

<style>
  .label { margin: 4px 0 10px; }
  .label:not(:first-child) { margin-top: 18px; }
  .field .input, .field .select { max-width: 320px; }
  .row { margin-bottom: 10px; }
  .saved { margin-right: auto; font-size: 12px; align-self: center; }
</style>
