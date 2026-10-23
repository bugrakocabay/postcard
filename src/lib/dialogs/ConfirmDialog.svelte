<script lang="ts">
  import Modal from '../components/Modal.svelte'
  import { ui } from '../state/ui.svelte'
  let btn: HTMLButtonElement | undefined = $state()
  $effect(() => btn?.focus())
</script>

{#if ui.confirmReq}
  {@const c = ui.confirmReq}
  <Modal title={c.title} width={420} onclose={() => c.resolve(false)}>
    <p class="msg">{c.message}</p>
    {#snippet footer()}
      <button class="btn" onclick={() => c.resolve(false)}>Cancel</button>
      <button class="btn {c.danger ? 'danger' : 'primary'}" bind:this={btn} onclick={() => c.resolve(true)}>{c.confirmLabel}</button>
    {/snippet}
  </Modal>
{/if}

<style>
  .msg { margin: 0; line-height: 1.5; }
</style>
