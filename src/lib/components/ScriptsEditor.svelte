<script lang="ts">
  import CodeEditor from '../editor/CodeEditor.svelte'
  import type { Scripts } from '../types'
  import { pmCompletions } from '../editor/pmCompletions'

  let { scripts, onsend }: { scripts: Scripts; onsend?: () => void } = $props()
  let which = $state<'pre' | 'post'>('pre')

  const hints = {
    pre: 'Runs before the request is sent. e.g. pm.environment.set("token", pm.shell("gcloud auth print-access-token"))',
    post: 'Runs after the response arrives. e.g. pm.environment.set("token", pm.response.json().access_token)',
  }
</script>

<div class="scripts">
  <div class="switch">
    <button class:active={which === 'pre'} onclick={() => (which = 'pre')}>
      Pre-request{#if scripts.pre.trim()}<span class="dot">●</span>{/if}
    </button>
    <button class:active={which === 'post'} onclick={() => (which = 'post')}>
      Post-response{#if scripts.post.trim()}<span class="dot">●</span>{/if}
    </button>
    <span class="hint faint">{hints[which]}</span>
  </div>
  <div class="editor">
    {#key which}
      <CodeEditor
        value={scripts[which]}
        lang="javascript"
        completions={pmCompletions}
        onchange={(v) => (scripts[which] = v)}
        onmodenter={onsend}
        placeholder={which === 'pre' ? '// pre-request script (JavaScript)' : '// post-response script (JavaScript)'}
      />
    {/key}
  </div>
</div>

<style>
  .scripts { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .switch { display: flex; align-items: center; gap: 4px; padding: 6px 10px; border-bottom: 1px solid var(--border); }
  .switch button { background: none; border: 1px solid transparent; border-radius: 4px; padding: 3px 10px; color: var(--muted); font-size: 12px; }
  .switch button.active { background: var(--panel-2); border-color: var(--border); color: var(--text-strong); }
  .dot { color: var(--ok); font-size: 8px; margin-left: 5px; vertical-align: middle; }
  .hint { font-size: 11.5px; margin-left: 8px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-family: var(--mono); }
  .editor { flex: 1; min-height: 0; }
</style>
