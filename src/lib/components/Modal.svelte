<script lang="ts" module>
  const modalStack: symbol[] = []
</script>

<script lang="ts">
  import type { Snippet } from 'svelte'

  interface Props {
    title: string
    width?: number
    onclose: () => void
    children: Snippet
    footer?: Snippet
    head?: Snippet
  }
  let { title, width = 520, onclose, children, footer, head }: Props = $props()

  // Only the topmost modal reacts to Esc.
  const id = Symbol()
  $effect(() => {
    modalStack.push(id)
    return () => {
      const i = modalStack.indexOf(id)
      if (i >= 0) modalStack.splice(i, 1)
    }
  })

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Escape' && modalStack[modalStack.length - 1] === id) {
      e.preventDefault()
      onclose()
    }
  }
</script>

<svelte:window onkeydown={onkey} />

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="modal-backdrop" onmousedown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="modal" style:width="{width}px" role="dialog" aria-label={title}>
    <div class="modal-head">
      <h2>{title}</h2>
      {@render head?.()}
      <button class="icon-btn" onclick={onclose} title="Close (Esc)">✕</button>
    </div>
    <div class="modal-body">
      {@render children()}
    </div>
    {#if footer}
      <div class="modal-foot">{@render footer()}</div>
    {/if}
  </div>
</div>
