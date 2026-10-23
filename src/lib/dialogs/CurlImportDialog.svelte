<script lang="ts">
  import Modal from '../components/Modal.svelte'
  import { ui } from '../state/ui.svelte'
  import { importCurlToNewTab } from '../actions.svelte'

  let text = $state('')
  let busy = $state(false)

  async function submit() {
    if (!text.trim()) return
    busy = true
    const ok = await importCurlToNewTab(text)
    busy = false
    if (ok) ui.close()
  }
</script>

<Modal title="Import curl" width={640} onclose={() => ui.close()}>
  <!-- svelte-ignore a11y_autofocus -->
  <textarea
    class="input"
    rows="12"
    bind:value={text}
    autofocus
    spellcheck="false"
    placeholder={"curl 'https://api.example.com/users' \\\n  -H 'Authorization: Bearer …' \\\n  --data-raw '{\"name\":\"Ada\"}'"}
    onkeydown={(e) => (e.ctrlKey || e.metaKey) && e.key === 'Enter' && submit()}
  ></textarea>
  <p class="faint small">Tip: you can also paste a curl command straight into the URL bar.</p>
  {#snippet footer()}
    <button class="btn" onclick={() => ui.close()}>Cancel</button>
    <button class="btn primary" onclick={submit} disabled={busy || !text.trim()}>Import</button>
  {/snippet}
</Modal>

<style>
  textarea { width: 100%; font-size: 12px; }
  .small { font-size: 12px; margin: 8px 0 0; }
</style>
