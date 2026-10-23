<script lang="ts">
  import { tabs } from '../state/tabs.svelte'
  import { ui } from '../state/ui.svelte'
  import { methodColor, shortMethod } from '../util/format'
  import EnvPicker from './EnvPicker.svelte'

  function menu(e: MouseEvent, id: string) {
    ui.showMenu(e, [
      { label: 'Close', hint: 'Ctrl+W', action: () => tabs.close(id) },
      {
        label: 'Close others',
        action: async () => {
          for (const t of tabs.tabs.filter((x) => x.id !== id)) await tabs.close(t.id)
        },
      },
      {
        label: 'Duplicate',
        action: () => {
          const t = tabs.tabs.find((x) => x.id === id)
          if (t) tabs.openCopy(t.draft)
        },
      },
    ])
  }
</script>

<div class="tabbar">
  <button class="icon-btn toggle" title="Toggle sidebar (Ctrl+B)" onclick={() => (ui.layout.sidebarCollapsed = !ui.layout.sidebarCollapsed)}>☰</button>
  <div class="tabs" role="tablist">
    {#each tabs.tabs as t (t.id)}
      <!-- svelte-ignore a11y_click_events_have_key_events -->
      <div
        class="tab"
        class:active={t.id === tabs.activeId}
        role="tab"
        tabindex="-1"
        aria-selected={t.id === tabs.activeId}
        title={t.draft.url || t.draft.name}
        onclick={() => tabs.activate(t.id)}
        onauxclick={(e) => e.button === 1 && tabs.close(t.id)}
        oncontextmenu={(e) => menu(e, t.id)}
      >
        <span class="method" style:color={methodColor(t.draft.method)}>{shortMethod(t.draft.method)}</span>
        <span class="name" class:unsaved={!t.ref}>{t.draft.name}</span>
        {#if t.sending}<span class="spin" title="Sending…"></span>{/if}
        <button
          class="close"
          class:dirty={tabs.isDirty(t)}
          title="Close"
          onclick={(e) => {
            e.stopPropagation()
            tabs.close(t.id)
          }}
        >
          <span class="x">✕</span><span class="d">●</span>
        </button>
      </div>
    {/each}
    <button class="icon-btn add" title="New tab (Ctrl+T)" onclick={() => tabs.newTab()}>+</button>
  </div>
  <EnvPicker />
</div>

<style>
  .tabbar {
    display: flex; align-items: center; gap: 6px; height: 38px; padding: 0 8px 0 6px;
    background: var(--bg-2); border-bottom: 1px solid var(--border); flex-shrink: 0;
  }
  .tabs { flex: 1; display: flex; align-items: center; gap: 2px; overflow-x: auto; min-width: 0; height: 100%; scrollbar-width: none; }
  .tabs::-webkit-scrollbar { display: none; }
  .tab {
    display: flex; align-items: center; gap: 6px; height: 28px; padding: 0 4px 0 10px; max-width: 220px; min-width: 90px;
    border-radius: 5px; cursor: pointer; color: var(--muted); flex-shrink: 0; border: 1px solid transparent;
  }
  .tab:hover { background: #ffffff08; color: var(--text); }
  .tab.active { background: var(--panel-2); color: var(--text-strong); border-color: var(--border); }
  .method { font-family: var(--mono); font-size: 10px; font-weight: 700; }
  .name { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .name.unsaved { font-style: italic; }
  .close { width: 18px; height: 18px; border: none; background: none; border-radius: 3px; color: var(--muted); padding: 0; font-size: 10px; display: grid; place-items: center; }
  .close:hover { background: #ffffff18; color: var(--text); }
  .close .d { display: none; color: var(--accent); font-size: 9px; }
  .close.dirty .x { display: none; }
  .close.dirty .d { display: inline; }
  .close.dirty:hover .x { display: inline; }
  .close.dirty:hover .d { display: none; }
  .add { flex-shrink: 0; }
  .spin { width: 10px; height: 10px; border: 2px solid var(--accent); border-right-color: transparent; border-radius: 50%; animation: spin .7s linear infinite; }
  @keyframes spin { to { transform: rotate(360deg); } }
</style>
