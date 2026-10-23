<script lang="ts">
  import { ws } from '../state/workspace.svelte'

  let list: HTMLDivElement | undefined = $state()

  $effect(() => {
    ws.consoleLines.length
    if (list) queueMicrotask(() => list && (list.scrollTop = list.scrollHeight))
  })

  function time(ts: number) {
    return new Date(ts).toLocaleTimeString(undefined, { hour12: false })
  }
</script>

<div class="console">
  <div class="head">
    <span class="label">Console</span>
    <span class="spacer"></span>
    <button class="btn small" onclick={() => ws.clearConsole()}>Clear</button>
    <button class="icon-btn" title="Hide console" onclick={() => (ws.consoleOpen = false)}>✕</button>
  </div>
  <div class="lines" bind:this={list}>
    {#each ws.consoleLines as l (l.id)}
      <div class="line {l.level}">
        <span class="t">{time(l.ts)}</span>
        <span class="msg">{l.message}</span>
      </div>
    {:else}
      <div class="faint pad">console.log output, script errors and request summaries appear here.</div>
    {/each}
  </div>
</div>

<style>
  .console { display: flex; flex-direction: column; height: 100%; background: var(--bg-2); min-height: 0; }
  .head { display: flex; align-items: center; gap: 6px; padding: 4px 8px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
  .spacer { flex: 1; }
  .lines { flex: 1; overflow: auto; font-family: var(--mono); font-size: 12px; padding: 4px 0; user-select: text; -webkit-user-select: text; }
  .line { display: flex; gap: 10px; padding: 1px 10px; white-space: pre-wrap; word-break: break-word; }
  .line .t { color: var(--faint); flex-shrink: 0; }
  .line.warn { color: var(--warn); background: #e8b04a0c; }
  .line.error { color: var(--danger); background: #f06a6a0f; }
  .line.info { color: var(--muted); }
  .pad { padding: 8px 10px; font-family: var(--font); }
</style>
