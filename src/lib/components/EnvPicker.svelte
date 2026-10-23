<script lang="ts">
  import { ws } from '../state/workspace.svelte'
  import { ui } from '../state/ui.svelte'
</script>

<div class="env">
  <select
    class="select"
    class:none={!ws.envs.activeId}
    value={ws.envs.activeId ?? ''}
    onchange={(e) => ws.setActiveEnv((e.currentTarget as HTMLSelectElement).value || null)}
    title="Active environment"
  >
    <option value="">No environment</option>
    {#each ws.envs.environments as env (env.id)}
      <option value={env.id}>{env.name}</option>
    {/each}
  </select>
  <button class="icon-btn" title="Edit environments" onclick={() => ui.open({ kind: 'envEditor', envId: ws.envs.activeId })}>⚙</button>
</div>

<style>
  .env { display: flex; align-items: center; gap: 2px; flex-shrink: 0; }
  .select { height: 26px; min-width: 150px; max-width: 220px; font-size: 12px; }
  .select.none { color: var(--muted); }
</style>
