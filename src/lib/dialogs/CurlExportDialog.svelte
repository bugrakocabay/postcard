<script lang="ts">
  import { writeText } from '@tauri-apps/plugin-clipboard-manager'
  import Modal from '../components/Modal.svelte'
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { tabs, toRequest } from '../state/tabs.svelte'
  import { api, errorMessage } from '../api'

  let { tabId }: { tabId: string } = $props()
  let resolved = $state(true)
  let text = $state('')

  $effect(() => {
    const tab = tabs.tabs.find((t) => t.id === tabId)
    const r = resolved
    if (!tab) return
    JSON.stringify(ws.envs)
    api
      .toCurl(toRequest(tab.draft), tabs.ancestorsFor(tab), r)
      .then((t) => (text = t))
      .catch((e) => (text = `# error: ${errorMessage(e)}`))
  })

  async function copy() {
    await writeText(text)
    ui.toast('Copied')
    ui.close()
  }
</script>

<Modal title="Copy as curl" width={680} onclose={() => ui.close()}>
  <div class="opts">
    <label class="check"><input type="radio" name="res" checked={resolved} onchange={() => (resolved = true)} /> Variables filled in</label>
    <label class="check"><input type="radio" name="res" checked={!resolved} onchange={() => (resolved = false)} /> Keep <span class="mono">{'{{placeholders}}'}</span></label>
  </div>
  <pre class="out">{text}</pre>
  <p class="faint small">Scripts are not included in the export.</p>
  {#snippet footer()}
    <button class="btn" onclick={() => ui.close()}>Close</button>
    <button class="btn primary" onclick={copy}>Copy</button>
  {/snippet}
</Modal>

<style>
  .opts { display: flex; gap: 18px; margin-bottom: 10px; }
  .out { margin: 0; padding: 10px 12px; background: var(--input); border: 1px solid var(--border); border-radius: 5px; font-family: var(--mono); font-size: 12px; white-space: pre-wrap; word-break: break-all; max-height: 50vh; overflow: auto; user-select: text; -webkit-user-select: text; }
  .small { font-size: 12px; margin: 8px 0 0; }
</style>
