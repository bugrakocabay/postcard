<script lang="ts">
  import { ui } from '../state/ui.svelte'

  let el: HTMLDivElement | undefined = $state()
  let pos = $state({ x: 0, y: 0 })

  $effect(() => {
    const m = ui.menu
    if (!m || !el) return
    const r = el.getBoundingClientRect()
    pos = {
      x: Math.min(m.x, window.innerWidth - r.width - 4),
      y: Math.min(m.y, window.innerHeight - r.height - 4),
    }
  })

  function close() {
    ui.menu = null
  }
</script>

<svelte:window
  onmousedown={(e) => {
    if (ui.menu && el && !el.contains(e.target as Node)) close()
  }}
  onkeydown={(e) => e.key === 'Escape' && close()}
  onblur={close}
/>

{#if ui.menu}
  <div class="menu" bind:this={el} style:left="{pos.x || ui.menu.x}px" style:top="{pos.y || ui.menu.y}px" role="menu">
    {#each ui.menu.items as item, i (i)}
      {#if item.separator}
        <div class="sep"></div>
      {:else}
        <button
          class:danger={item.danger}
          role="menuitem"
          onclick={() => {
            close()
            item.action?.()
          }}
        >
          <span>{item.label}</span>
          {#if item.hint}<span class="hint">{item.hint}</span>{/if}
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .menu {
    position: fixed; z-index: 200; min-width: 180px; padding: 4px;
    background: var(--panel-2); border: 1px solid var(--border-strong); border-radius: 6px;
    box-shadow: 0 8px 30px #0008;
  }
  button {
    display: flex; justify-content: space-between; gap: 20px; width: 100%;
    padding: 5px 10px; background: none; border: none; border-radius: 4px; text-align: left;
  }
  button:hover { background: var(--accent); color: #fff; }
  button.danger { color: var(--danger); }
  button.danger:hover { background: var(--danger); color: #fff; }
  .hint { color: var(--faint); font-size: 11px; }
  button:hover .hint { color: #ffffffb0; }
  .sep { height: 1px; background: var(--border); margin: 4px 2px; }
</style>
