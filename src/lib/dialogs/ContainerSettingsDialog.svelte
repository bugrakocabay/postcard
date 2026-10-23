<script lang="ts">
  // Auth and scripts for a collection or folder (inherited by requests inside).
  import Modal from '../components/Modal.svelte'
  import AuthEditor from '../components/AuthEditor.svelte'
  import ScriptsEditor from '../components/ScriptsEditor.svelte'
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { findContainer } from '../util/tree'

  let { collectionId, folderId }: { collectionId: string; folderId: string | null } = $props()
  const coll = $derived(ws.collection(collectionId))
  const container = $derived(coll ? findContainer(coll, folderId) : null)
  let section = $state<'auth' | 'scripts'>('auth')

  // Save only on changes, not when the dialog opens.
  let mounted = false
  $effect(() => {
    if (!container || !coll) return
    JSON.stringify(container.auth)
    JSON.stringify(container.scripts)
    if (mounted) ws.saveCollection(coll)
    mounted = true
  })
</script>

{#if container}
  <Modal title="{folderId ? 'Folder' : 'Collection'}: {container.name}" width={760} onclose={() => ui.close()}>
    <div class="wrap">
      <div class="subtabs">
        <button class:active={section === 'auth'} onclick={() => (section = 'auth')}>Auth</button>
        <button class:active={section === 'scripts'} onclick={() => (section = 'scripts')}>Scripts</button>
      </div>
      {#if section === 'auth'}
        <p class="muted note">Requests set to "Inherit from parent" use this auth.</p>
        <AuthEditor auth={container.auth} allowInherit={folderId !== null} onchange={(a) => (container.auth = a)} />
      {:else}
        <p class="muted note">These scripts run for every request inside, before the request's own scripts (outermost first).</p>
        <div class="scripts"><ScriptsEditor scripts={container.scripts} /></div>
      {/if}
    </div>
    {#snippet footer()}
      <span class="faint saved">Changes save automatically</span>
      <button class="btn primary" onclick={() => ui.close()}>Done</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .wrap { margin: -14px; }
  .note { margin: 10px 14px 0; font-size: 12px; }
  .scripts { height: 380px; display: flex; flex-direction: column; border-top: 1px solid var(--border); margin-top: 10px; }
  .saved { margin-right: auto; font-size: 12px; align-self: center; }
</style>
