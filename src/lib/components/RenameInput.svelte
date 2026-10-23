<script lang="ts">
  interface Props {
    value: string
    oncommit: (value: string) => void
    oncancel: () => void
  }
  let { value, oncommit, oncancel }: Props = $props()
  let el: HTMLInputElement
  let done = false

  $effect(() => {
    el.focus()
    el.select()
  })

  function commit() {
    if (done) return
    done = true
    oncommit(el.value)
  }
</script>

<input
  bind:this={el}
  class="rename"
  {value}
  onkeydown={(e) => {
    e.stopPropagation()
    if (e.key === 'Enter') commit()
    if (e.key === 'Escape') {
      done = true
      oncancel()
    }
  }}
  onblur={commit}
  onclick={(e) => e.stopPropagation()}
/>

<style>
  .rename {
    flex: 1; min-width: 0; height: 20px; padding: 0 4px;
    background: var(--input); border: 1px solid var(--accent); border-radius: 3px; color: var(--text); outline: none;
  }
</style>
