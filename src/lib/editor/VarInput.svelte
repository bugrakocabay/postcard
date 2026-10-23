<script lang="ts">
  // Single-line input that highlights {{variables}} (green when defined, red when not)
  // and shows the resolved value on hover.
  import { onDestroy, onMount } from 'svelte'
  import { EditorState, StateEffect, type Extension } from '@codemirror/state'
  import {
    Decoration,
    EditorView,
    ViewPlugin,
    hoverTooltip,
    keymap,
    tooltips,
    placeholder as cmPlaceholder,
    type DecorationSet,
    type ViewUpdate,
  } from '@codemirror/view'
  import { defaultKeymap, history, historyKeymap } from '@codemirror/commands'
  import { findVarTokens, DYNAMIC_VARS } from '../util/varTokens'
  import { ws, type VarInfo } from '../state/workspace.svelte'

  interface Props {
    value: string
    placeholder?: string
    mono?: boolean
    /** Return true if the paste was handled (default paste is then skipped). */
    onpaste?: (text: string) => boolean
    onenter?: () => void
    onchange?: (value: string) => void
    /** Called on Mod-z; return true to swallow the normal undo. */
    onundo?: () => boolean
  }

  let { value, placeholder = '', mono = true, onpaste, onenter, onchange, onundo }: Props = $props()

  let host: HTMLDivElement
  let view: EditorView | null = null
  let vars: Map<string, VarInfo> = new Map()
  const refresh = StateEffect.define<null>()

  function isKnown(name: string): boolean {
    return vars.has(name) || DYNAMIC_VARS.includes(name)
  }

  function build(v: EditorView): DecorationSet {
    const text = v.state.doc.toString()
    const ranges = findVarTokens(text).map((t) =>
      Decoration.mark({ class: isKnown(t.name) ? 'cm-var-ok' : 'cm-var-bad' }).range(t.from, t.to),
    )
    return Decoration.set(ranges)
  }

  const varPlugin = ViewPlugin.fromClass(
    class {
      decorations: DecorationSet
      constructor(v: EditorView) {
        this.decorations = build(v)
      }
      update(u: ViewUpdate) {
        if (u.docChanged || u.transactions.some((tr) => tr.effects.some((e) => e.is(refresh)))) {
          this.decorations = build(u.view)
        }
      }
    },
    { decorations: (p) => p.decorations },
  )

  const varTooltip = hoverTooltip((v, pos) => {
    const text = v.state.doc.toString()
    const tok = findVarTokens(text).find((t) => pos >= t.from && pos <= t.to)
    if (!tok) return null
    return {
      pos: tok.from,
      end: tok.to,
      create() {
        const dom = document.createElement('div')
        dom.className = 'var-tip'
        const info = vars.get(tok.name)
        const label = document.createElement('div')
        label.className = 'var-tip-label'
        if (info) {
          label.textContent = `${tok.name} · ${info.source}`
          const val = document.createElement('div')
          val.className = 'var-tip-value'
          val.textContent = info.secret ? '••••••••' : info.value || '(empty)'
          dom.append(label, val)
        } else if (DYNAMIC_VARS.includes(tok.name)) {
          label.textContent = `${tok.name} · generated on send`
          dom.append(label)
        } else {
          label.textContent = `${tok.name} · undefined`
          label.style.color = 'var(--var-bad)'
          dom.append(label)
        }
        return { dom }
      },
    }
  })

  // Keep the document a single line.
  const singleLine: Extension = EditorState.transactionFilter.of((tr) => {
    if (!tr.docChanged || tr.newDoc.lines <= 1) return tr
    const flat = tr.newDoc.toString().replace(/\r?\n\s*/g, '')
    return [
      tr,
      {
        changes: { from: 0, to: tr.newDoc.length, insert: flat },
        sequential: true,
        selection: { anchor: Math.min(tr.newSelection.main.head, flat.length) },
      },
    ]
  })

  const theme = EditorView.theme({
    '&': { color: 'var(--text)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-scroller': { overflow: 'hidden', fontFamily: 'inherit', lineHeight: '20px' },
    '.cm-content': { padding: '4px 0', caretColor: 'var(--accent)' },
    '.cm-line': { padding: '0 8px' },
    '.cm-cursor': { borderLeftColor: 'var(--accent)' },
    '.cm-selectionBackground, &.cm-focused .cm-selectionBackground, ::selection': {
      backgroundColor: '#34466e !important',
    },
    '.cm-placeholder': { color: 'var(--faint)' },
    '.cm-var-ok': { color: 'var(--var-ok)', backgroundColor: '#5fcf8614', borderRadius: '3px' },
    '.cm-var-bad': { color: 'var(--var-bad)', backgroundColor: '#f06a6a14', borderRadius: '3px' },
    '.cm-tooltip': {
      backgroundColor: 'var(--panel-2)',
      border: '1px solid var(--border-strong)',
      borderRadius: '4px',
      padding: '6px 8px',
      maxWidth: '420px',
    },
    '.var-tip-label': { fontSize: '11px', color: 'var(--muted)' },
    '.var-tip-value': { fontFamily: 'var(--mono)', wordBreak: 'break-all', marginTop: '2px' },
  })

  onMount(() => {
    vars = ws.vars
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          history(),
          singleLine,
          keymap.of([
            {
              key: 'Enter',
              run: () => {
                onenter?.()
                return true
              },
            },
            {
              key: 'Mod-Enter',
              run: () => {
                onenter?.()
                return true
              },
            },
            { key: 'Mod-z', run: () => onundo?.() ?? false },
            ...defaultKeymap,
            ...historyKeymap,
          ]),
          cmPlaceholder(placeholder),
          varPlugin,
          varTooltip,
          tooltips({ parent: document.body, position: 'fixed' }),
          theme,
          EditorView.domEventHandlers({
            paste: (e) => {
              const text = e.clipboardData?.getData('text/plain') ?? ''
              if (onpaste && onpaste(text)) {
                e.preventDefault()
                return true
              }
              return false
            },
          }),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange?.(u.state.doc.toString())
          }),
        ],
      }),
    })
  })

  onDestroy(() => view?.destroy())

  $effect(() => {
    const v = value
    if (view && v !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: v } })
    }
  })

  // Re-colour variables when environments change.
  $effect(() => {
    vars = ws.vars
    view?.dispatch({ effects: refresh.of(null) })
  })

  export function focus() {
    view?.focus()
  }
</script>

<div class="var-input" class:mono bind:this={host}></div>

<style>
  .var-input {
    flex: 1;
    min-width: 0;
    user-select: text;
    -webkit-user-select: text;
  }
  .var-input.mono {
    font-family: var(--mono);
  }
</style>
