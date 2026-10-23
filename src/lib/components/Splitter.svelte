<script lang="ts">
  // Drag handle. Reports the pointer delta in pixels since drag start.
  interface Props {
    direction: 'horizontal' | 'vertical'
    onstart?: () => void
    ondrag: (delta: number) => void
    onend?: () => void
  }
  let { direction, onstart, ondrag, onend }: Props = $props()
  let dragging = $state(false)

  function down(e: PointerEvent) {
    e.preventDefault()
    const start = direction === 'vertical' ? e.clientX : e.clientY
    dragging = true
    onstart?.()
    const el = e.currentTarget as HTMLElement
    el.setPointerCapture(e.pointerId)
    const move = (ev: PointerEvent) => ondrag((direction === 'vertical' ? ev.clientX : ev.clientY) - start)
    const up = () => {
      dragging = false
      el.removeEventListener('pointermove', move)
      el.removeEventListener('pointerup', up)
      onend?.()
    }
    el.addEventListener('pointermove', move)
    el.addEventListener('pointerup', up)
  }
</script>

<div class="splitter {direction}" class:dragging onpointerdown={down} role="separator"></div>

<style>
  .splitter { flex-shrink: 0; position: relative; z-index: 5; }
  .splitter.vertical { width: 1px; cursor: col-resize; background: var(--border); }
  .splitter.horizontal { height: 1px; cursor: row-resize; background: var(--border); }
  .splitter::after { content: ''; position: absolute; }
  .splitter.vertical::after { inset: 0 -4px; }
  .splitter.horizontal::after { inset: -4px 0; }
  .splitter:hover, .splitter.dragging { background: var(--accent); }
</style>
