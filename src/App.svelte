<script lang="ts">
  import { onMount } from 'svelte'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import TabBar from './lib/components/TabBar.svelte'
  import Sidebar from './lib/components/Sidebar.svelte'
  import RequestPane from './lib/components/RequestPane.svelte'
  import ResponsePane from './lib/components/ResponsePane.svelte'
  import ConsolePanel from './lib/components/ConsolePanel.svelte'
  import Splitter from './lib/components/Splitter.svelte'
  import ContextMenu from './lib/components/ContextMenu.svelte'
  import ConfirmDialog from './lib/dialogs/ConfirmDialog.svelte'
  import SaveAsDialog from './lib/dialogs/SaveAsDialog.svelte'
  import ContainerSettingsDialog from './lib/dialogs/ContainerSettingsDialog.svelte'
  import EnvEditorDialog from './lib/dialogs/EnvEditorDialog.svelte'
  import QuickSearchDialog from './lib/dialogs/QuickSearchDialog.svelte'
  import CookiesDialog from './lib/dialogs/CookiesDialog.svelte'
  import SettingsDialog from './lib/dialogs/SettingsDialog.svelte'
  import CurlImportDialog from './lib/dialogs/CurlImportDialog.svelte'
  import CurlExportDialog from './lib/dialogs/CurlExportDialog.svelte'
  import { ws } from './lib/state/workspace.svelte'
  import { tabs } from './lib/state/tabs.svelte'
  import { ui } from './lib/state/ui.svelte'
  import { saveActive } from './lib/actions.svelte'

  let main: HTMLDivElement | undefined = $state()
  let startSidebar = 0
  let startRatio = 0
  let startConsole = 0

  onMount(async () => {
    await ws.load()
    tabs.restore(ws.session)
    // Write pending saves before the window closes.
    getCurrentWindow()
      .onCloseRequested(async () => {
        await Promise.all([ws.flushAll(), tabs.flushSession()])
      })
      .catch(() => {})
  })

  const isMac = navigator.userAgent.includes('Mac')

  function onkey(e: KeyboardEvent) {
    if (e.defaultPrevented || ui.dialog || ui.confirmReq) return
    const mod = isMac ? e.metaKey : e.ctrlKey
    if (!mod || e.altKey) return
    const k = e.key.toLowerCase()
    const tab = tabs.active
    let handled = true
    if (k === 'enter') {
      if (tab) tabs.send(tab)
    } else if (k === 's') saveActive()
    else if (k === 'k' || k === 'p') ui.open({ kind: 'quickSearch' })
    else if (k === 't') tabs.newTab()
    else if (k === 'w') {
      if (tab) tabs.close(tab.id)
    } else if (k === 'b') ui.layout.sidebarCollapsed = !ui.layout.sidebarCollapsed
    else if (k === 'j') ws.consoleOpen = !ws.consoleOpen
    else if (k === 'e') ui.open({ kind: 'envEditor', envId: ws.envs.activeId })
    else if (k === ',') ui.open({ kind: 'settings' })
    else handled = false
    if (handled) e.preventDefault()
  }

  $effect(() => {
    if (ws.consoleOpen) ws.unreadConsole = 0
  })

  $effect(() => {
    JSON.stringify(ui.layout)
    if (ws.loaded) tabs.persist()
  })
</script>

<svelte:window onkeydown={onkey} />

{#if ws.loaded}
  <div class="app">
    <TabBar />
    <div class="body">
      {#if !ui.layout.sidebarCollapsed}
        <div class="side" style:width="{ui.layout.sidebarWidth}px">
          <Sidebar />
        </div>
        <Splitter
          direction="vertical"
          onstart={() => (startSidebar = ui.layout.sidebarWidth)}
          ondrag={(d) => (ui.layout.sidebarWidth = Math.max(180, Math.min(560, startSidebar + d)))}
        />
      {/if}
      <div class="main" bind:this={main}>
        {#if tabs.active}
          {#key tabs.active.id}
            <div class="req" style:flex-basis="{ui.layout.requestRatio * 100}%">
              <RequestPane tab={tabs.active} />
            </div>
            <Splitter
              direction="horizontal"
              onstart={() => (startRatio = ui.layout.requestRatio)}
              ondrag={(d) => {
                const h = main?.clientHeight ?? 1
                ui.layout.requestRatio = Math.max(0.15, Math.min(0.85, startRatio + d / h))
              }}
            />
            <div class="res">
              <ResponsePane tab={tabs.active} />
            </div>
          {/key}
        {:else}
          <div class="none">
            <p class="muted">No open tabs.</p>
            <button class="btn primary" onclick={() => tabs.newTab()}>New request</button>
          </div>
        {/if}
        {#if ws.consoleOpen}
          <Splitter
            direction="horizontal"
            onstart={() => (startConsole = ui.layout.consoleHeight)}
            ondrag={(d) => (ui.layout.consoleHeight = Math.max(80, Math.min(600, startConsole - d)))}
          />
          <div class="console" style:height="{ui.layout.consoleHeight}px">
            <ConsolePanel />
          </div>
        {/if}
      </div>
    </div>
    <footer class="statusbar">
      <button onclick={() => (ws.consoleOpen = !ws.consoleOpen)} class:on={ws.consoleOpen}>
        Console{#if ws.unreadConsole && !ws.consoleOpen}<span class="badge">{ws.unreadConsole > 99 ? '99+' : ws.unreadConsole}</span>{/if}
      </button>
      <button onclick={() => ui.open({ kind: 'curlImport' })}>Import curl</button>
      {#if tabs.active}
        <button onclick={() => tabs.active && ui.open({ kind: 'curlExport', tabId: tabs.active.id })}>Copy as curl</button>
      {/if}
      <span class="grow"></span>
      <button onclick={() => ui.open({ kind: 'cookies' })}>Cookies</button>
      <button onclick={() => ui.open({ kind: 'settings' })}>Settings</button>
      <span class="keys faint">{isMac ? '⌘' : 'Ctrl'}+K search · {isMac ? '⌘' : 'Ctrl'}+Enter send · {isMac ? '⌘' : 'Ctrl'}+S save</span>
    </footer>
  </div>

  {#if ui.dialog}
    {@const d = ui.dialog}
    {#if d.kind === 'saveAs'}
      <SaveAsDialog tabId={d.tabId} />
    {:else if d.kind === 'containerSettings'}
      <ContainerSettingsDialog collectionId={d.collectionId} folderId={d.folderId} />
    {:else if d.kind === 'envEditor'}
      <EnvEditorDialog envId={d.envId} />
    {:else if d.kind === 'quickSearch'}
      <QuickSearchDialog />
    {:else if d.kind === 'cookies'}
      <CookiesDialog />
    {:else if d.kind === 'settings'}
      <SettingsDialog />
    {:else if d.kind === 'curlImport'}
      <CurlImportDialog />
    {:else if d.kind === 'curlExport'}
      <CurlExportDialog tabId={d.tabId} />
    {/if}
  {/if}
  <ConfirmDialog />
  <ContextMenu />
  <div class="toasts">
    {#each ui.toasts as t (t.id)}<div class="toast">{t.message}</div>{/each}
  </div>
{/if}

<style>
  .app { display: flex; flex-direction: column; height: 100vh; }
  .body { flex: 1; display: flex; min-height: 0; }
  .side { flex-shrink: 0; min-width: 0; }
  .main { flex: 1; display: flex; flex-direction: column; min-width: 0; min-height: 0; }
  .req { flex-grow: 0; flex-shrink: 0; min-height: 120px; overflow: hidden; }
  .res { flex: 1; min-height: 100px; overflow: hidden; }
  .console { flex-shrink: 0; min-height: 0; }
  .none { flex: 1; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px; }
  .statusbar {
    display: flex; align-items: center; gap: 2px; height: 26px; padding: 0 6px; flex-shrink: 0;
    background: var(--bg-2); border-top: 1px solid var(--border); font-size: 12px;
  }
  .statusbar button { background: none; border: none; color: var(--muted); padding: 2px 8px; border-radius: 3px; display: flex; align-items: center; gap: 5px; }
  .statusbar button:hover { color: var(--text); background: #ffffff0d; }
  .statusbar button.on { color: var(--text-strong); }
  .badge { background: var(--accent); color: #fff; border-radius: 8px; padding: 0 5px; font-size: 10px; line-height: 15px; }
  .grow { flex: 1; }
  .keys { font-size: 11px; margin-left: 10px; }
  .toasts { position: fixed; bottom: 40px; left: 50%; transform: translateX(-50%); display: flex; flex-direction: column; gap: 6px; z-index: 300; pointer-events: none; }
  .toast { background: var(--panel-2); border: 1px solid var(--border-strong); border-radius: 6px; padding: 7px 14px; box-shadow: 0 6px 20px #0007; color: var(--text-strong); animation: pop .15s ease-out; }
  @keyframes pop { from { opacity: 0; transform: translateY(6px); } }
</style>
