<script lang="ts">
  import { save } from '@tauri-apps/plugin-dialog'
  import { writeText } from '@tauri-apps/plugin-clipboard-manager'
  import CodeEditor, { type Lang } from '../editor/CodeEditor.svelte'
  import { tabs, type ResponseSubTab, type Tab } from '../state/tabs.svelte'
  import { ui } from '../state/ui.svelte'
  import { ws } from '../state/workspace.svelte'
  import { api, errorMessage } from '../api'
  import { formatBytes, formatMs, statusClass } from '../util/format'
  import { formatJson } from '../util/jsonFormat'

  let { tab }: { tab: Tab } = $props()

  const PRETTY_LIMIT = 10 * 1024 * 1024

  let mode = $state<'pretty' | 'raw'>('pretty')
  let wrap = $state(false)
  let forcePretty = $state(false)

  const result = $derived(tab.result)
  const meta = $derived(result?.response ?? null)
  const body = $derived(result?.body ?? null)
  const ct = $derived((meta?.contentType ?? '').toLowerCase())

  // Reset per-response view options when a new response arrives.
  $effect(() => {
    result?.responseId
    forcePretty = false
  })

  const lang = $derived.by<Lang>(() => {
    if (ct.includes('json')) return 'json'
    if (ct.includes('html')) return 'html'
    if (ct.includes('xml')) return 'xml'
    if (ct.includes('javascript')) return 'javascript'
    if (body?.type === 'text' && /^\s*[[{]/.test(body.text.slice(0, 100))) return 'json'
    return 'text'
  })

  const tooBigForPretty = $derived(body?.type === 'text' && body.text.length > PRETTY_LIMIT && !forcePretty)

  const shown = $derived.by(() => {
    if (body?.type !== 'text') return ''
    if (mode === 'raw' || tooBigForPretty || lang !== 'json') return body.text
    return formatJson(body.text) ?? body.text
  })

  const cookies = $derived(
    (meta?.setCookies ?? []).map((c) => {
      const [pair, ...attrs] = c.split(';')
      const eq = pair.indexOf('=')
      return {
        name: eq < 0 ? pair.trim() : pair.slice(0, eq).trim(),
        value: eq < 0 ? '' : pair.slice(eq + 1).trim(),
        attrs: attrs.map((a) => a.trim()).filter(Boolean).join('; '),
      }
    }),
  )

  async function saveBody() {
    if (!result) return
    const ext = lang === 'json' ? 'json' : lang === 'html' ? 'html' : lang === 'xml' ? 'xml' : body?.type === 'image' ? body.mime.split('/')[1] : 'txt'
    const path = await save({ defaultPath: `response.${ext}` })
    if (!path) return
    try {
      await api.saveResponseBody(result.responseId, path)
      ui.toast('Response saved')
    } catch (e) {
      ws.log('error', `Could not save response: ${errorMessage(e)}`)
    }
  }

  async function copyBody() {
    await writeText(shown)
    ui.toast('Copied response body')
  }

  const subtabs: [ResponseSubTab, string][] = [
    ['body', 'Body'],
    ['headers', 'Headers'],
    ['cookies', 'Cookies'],
  ]
</script>

<div class="response">
  {#if tab.sending}
    <div class="center">
      <div class="spinner"></div>
      <p class="muted">Sending request…</p>
      <button class="btn small" onclick={() => tabs.cancel(tab)}>Cancel</button>
    </div>
  {:else if tab.error}
    <div class="error-box"><strong>Error</strong><pre>{tab.error}</pre></div>
  {:else if result && !meta}
    <div class="error-box">
      <strong>Could not get a response</strong>
      <pre>{result.error}</pre>
      <p class="muted">See the console for details.</p>
    </div>
  {:else if meta}
    <div class="subtabs">
      {#each subtabs as [id, label] (id)}
        <button class:active={tab.resTab === id} onclick={() => (tab.resTab = id)}>
          {label}
          {#if id === 'headers'}<span class="count">{meta.headers.length}</span>{/if}
          {#if id === 'cookies' && cookies.length}<span class="count">{cookies.length}</span>{/if}
        </button>
      {/each}
      <span class="spacer"></span>
      <span class="status">
        <span class="code {statusClass(meta.status)}">{meta.status} {meta.statusText}</span>
        <span class="muted">{formatMs(meta.timeMs)}</span>
        <span class="muted">{formatBytes(meta.sizeBytes)}</span>
      </span>
    </div>
    {#if result?.error}
      <div class="warn-strip">{result.error}</div>
    {/if}
    <div class="content">
      {#if tab.resTab === 'body'}
        {#if body?.type === 'text'}
          <div class="body-tools">
            <div class="seg">
              <button class:active={mode === 'pretty'} onclick={() => (mode = 'pretty')}>Pretty</button>
              <button class:active={mode === 'raw'} onclick={() => (mode = 'raw')}>Raw</button>
            </div>
            <label class="check small"><input type="checkbox" bind:checked={wrap} /> Wrap</label>
            {#if tooBigForPretty && mode === 'pretty'}
              <span class="faint small">Large response shown raw.</span>
              <button class="btn small" onclick={() => (forcePretty = true)}>Pretty print anyway</button>
            {/if}
            <span class="spacer"></span>
            <span class="faint small">Ctrl+F to search</span>
            <button class="btn small" onclick={copyBody}>Copy</button>
            <button class="btn small" onclick={saveBody}>Save…</button>
          </div>
          <div class="editor">
            <CodeEditor value={shown} lang={mode === 'raw' && lang === 'json' ? 'text' : lang} readonly {wrap} />
          </div>
        {:else if body?.type === 'image'}
          <div class="body-tools">
            <span class="muted small">{body.mime}</span>
            <span class="spacer"></span>
            <button class="btn small" onclick={saveBody}>Save…</button>
          </div>
          <div class="image"><img src="data:{body.mime};base64,{body.base64}" alt="Response" /></div>
        {:else if body?.type === 'binary' || body?.type === 'tooLarge'}
          <div class="center">
            <p>
              {body.type === 'binary' ? 'Binary response' : 'Response is too large to show'} ({formatBytes(meta.sizeBytes)}).
            </p>
            <button class="btn primary" onclick={saveBody}>Save to file…</button>
          </div>
        {:else}
          <div class="center muted">Empty body</div>
        {/if}
      {:else if tab.resTab === 'headers'}
        <div class="scroll">
          <table>
            <tbody>
              {#each meta.headers as [k, v], i (i)}
                <tr><td class="k">{k}</td><td class="v">{v}</td></tr>
              {/each}
            </tbody>
          </table>
        </div>
      {:else}
        <div class="scroll">
          {#if cookies.length}
            <table>
              <thead><tr><th>Name</th><th>Value</th><th>Attributes</th></tr></thead>
              <tbody>
                {#each cookies as c, i (i)}
                  <tr><td class="k">{c.name}</td><td class="v">{c.value}</td><td class="muted">{c.attrs}</td></tr>
                {/each}
              </tbody>
            </table>
          {:else}
            <div class="empty">This response set no cookies.</div>
          {/if}
        </div>
      {/if}
    </div>
  {:else}
    <div class="center faint">
      <p>Send a request to see the response.</p>
      <p class="keys"><kbd>Ctrl</kbd> + <kbd>Enter</kbd> to send</p>
    </div>
  {/if}
</div>

<style>
  .response { display: flex; flex-direction: column; height: 100%; min-height: 0; background: var(--bg); }
  .status { display: flex; gap: 12px; align-items: center; font-family: var(--mono); font-size: 12px; }
  .code { font-weight: 700; }
  .content { flex: 1; min-height: 0; display: flex; flex-direction: column; }
  .body-tools { display: flex; align-items: center; gap: 10px; padding: 5px 10px; border-bottom: 1px solid var(--border); flex-shrink: 0; }
  .seg { display: flex; border: 1px solid var(--border); border-radius: 4px; overflow: hidden; }
  .seg button { background: none; border: none; padding: 2px 10px; font-size: 12px; color: var(--muted); }
  .seg button.active { background: var(--panel-2); color: var(--text-strong); }
  .small { font-size: 12px; }
  .spacer { flex: 1; }
  .editor { flex: 1; min-height: 0; }
  .scroll { flex: 1; overflow: auto; user-select: text; -webkit-user-select: text; }
  table { width: 100%; border-collapse: collapse; font-family: var(--mono); font-size: 12px; }
  th { text-align: left; color: var(--faint); font-weight: 500; padding: 6px 12px; border-bottom: 1px solid var(--border); font-family: var(--font); }
  td { padding: 4px 12px; border-bottom: 1px solid #ffffff08; vertical-align: top; word-break: break-all; }
  td.k { color: #8fc1ff; white-space: nowrap; width: 1%; }
  .center { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; }
  .center p { margin: 0; }
  .keys { font-size: 12px; }
  kbd { background: var(--panel-2); border: 1px solid var(--border-strong); border-radius: 3px; padding: 1px 5px; font-family: var(--mono); font-size: 11px; }
  .image { flex: 1; overflow: auto; padding: 16px; display: flex; align-items: flex-start; justify-content: center;
    background-image: conic-gradient(#2a2c30 25%, #232428 0 50%, #2a2c30 0 75%, #232428 0); background-size: 16px 16px; }
  .image img { max-width: 100%; }
  .error-box { margin: 16px; padding: 12px 14px; border: 1px solid #6b3131; background: #f06a6a12; border-radius: 6px; }
  .error-box strong { color: var(--danger); }
  .error-box pre { white-space: pre-wrap; word-break: break-word; font-family: var(--mono); font-size: 12px; margin: 8px 0 0; user-select: text; -webkit-user-select: text; }
  .error-box p { margin: 8px 0 0; font-size: 12px; }
  .warn-strip { padding: 5px 12px; background: #e8b04a14; color: var(--warn); font-size: 12px; border-bottom: 1px solid var(--border); }
  .spinner { width: 22px; height: 22px; border: 3px solid var(--accent); border-right-color: transparent; border-radius: 50%; animation: spin .7s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
