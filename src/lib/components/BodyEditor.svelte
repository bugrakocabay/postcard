<script lang="ts">
  import { open } from '@tauri-apps/plugin-dialog'
  import CodeEditor from '../editor/CodeEditor.svelte'
  import KeyValueTable from './KeyValueTable.svelte'
  import FormTable from './FormTable.svelte'
  import type { Body, BodyType, Request } from '../types'
  import { formatJson } from '../util/jsonFormat'

  let { request, onsend }: { request: Request; onsend: () => void } = $props()

  const types: [BodyType, string][] = [
    ['none', 'None'],
    ['json', 'JSON'],
    ['raw', 'Raw'],
    ['urlEncoded', 'Form URL-encoded'],
    ['multipart', 'Multipart form'],
    ['binary', 'Binary file'],
  ]
  const rawTypes = ['text/plain', 'application/xml', 'text/html', 'application/javascript', 'text/csv']

  function textOf(b: Body): string {
    return b.type === 'json' || b.type === 'raw' ? b.text : ''
  }

  function setType(type: BodyType) {
    const b = request.body
    if (b.type === type) return
    switch (type) {
      case 'none':
        request.body = { type }
        break
      case 'json':
        request.body = { type, text: textOf(b) }
        break
      case 'raw':
        request.body = { type, text: textOf(b), contentType: 'text/plain' }
        break
      case 'urlEncoded':
        request.body = {
          type,
          fields: b.type === 'multipart' ? b.fields.filter((f) => f.kind === 'text').map(({ key, value, enabled }) => ({ key, value, enabled })) : [],
        }
        break
      case 'multipart':
        request.body = {
          type,
          fields: b.type === 'urlEncoded' ? b.fields.map((f) => ({ ...f, kind: 'text' as const })) : [],
        }
        break
      case 'binary':
        request.body = { type, path: '' }
        break
    }
  }

  function prettify() {
    const b = request.body
    if (b.type !== 'json') return
    // Invalid JSON is left as is; the linter shows the error.
    b.text = formatJson(b.text) ?? b.text
  }

  async function pickBinary() {
    const path = await open({ multiple: false, directory: false })
    if (typeof path === 'string' && request.body.type === 'binary') request.body.path = path
  }

  const rawLang = $derived.by(() => {
    const b = request.body
    if (b.type !== 'raw') return 'text' as const
    if (b.contentType.includes('xml')) return 'xml' as const
    if (b.contentType.includes('html')) return 'html' as const
    if (b.contentType.includes('javascript')) return 'javascript' as const
    return 'text' as const
  })
</script>

<div class="body">
  <div class="types">
    {#each types as [t, label] (t)}
      <label class="check">
        <input type="radio" name="body-type-{request.id}" checked={request.body.type === t} onchange={() => setType(t)} />
        {label}
      </label>
    {/each}
    <span class="spacer"></span>
    {#if request.body.type === 'json'}
      <button class="btn small" onclick={prettify}>Beautify</button>
    {:else if request.body.type === 'raw'}
      <input class="input ct mono" list="raw-types" bind:value={request.body.contentType} placeholder="Content-Type" />
      <datalist id="raw-types">{#each rawTypes as t (t)}<option value={t}></option>{/each}</datalist>
    {/if}
  </div>

  <div class="content">
    {#if request.body.type === 'none'}
      <div class="empty">This request has no body.</div>
    {:else if request.body.type === 'json'}
      {@const b = request.body}
      <CodeEditor value={b.text} lang="json" lint onchange={(v) => (b.text = v)} onmodenter={onsend} placeholder={'{\n  "key": "value"\n}'} />
    {:else if request.body.type === 'raw'}
      {@const b = request.body}
      <CodeEditor value={b.text} lang={rawLang} onchange={(v) => (b.text = v)} onmodenter={onsend} />
    {:else if request.body.type === 'urlEncoded'}
      <div class="scroll"><KeyValueTable rows={request.body.fields} /></div>
    {:else if request.body.type === 'multipart'}
      <div class="scroll"><FormTable rows={request.body.fields} /></div>
    {:else if request.body.type === 'binary'}
      {@const b = request.body}
      <div class="binary">
        <button class="btn" onclick={pickBinary}>Choose file…</button>
        <span class="mono path">{b.path || 'No file selected'}</span>
        <input class="input mono" placeholder="…or type a path (variables allowed)" bind:value={b.path} />
      </div>
    {/if}
  </div>
</div>

<style>
  .body { display: flex; flex-direction: column; height: 100%; min-height: 0; }
  .types { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 14px; padding: 8px 12px; border-bottom: 1px solid var(--border); font-size: 12px; }
  .types input[type='radio'] { accent-color: var(--accent); margin: 0; }
  .spacer { flex: 1; }
  .ct { height: 24px; width: 200px; font-size: 12px; }
  .content { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .scroll { overflow: auto; flex: 1; }
  .binary { padding: 14px; display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .binary .input { width: 100%; max-width: 560px; }
  .path { color: var(--muted); word-break: break-all; }
</style>
