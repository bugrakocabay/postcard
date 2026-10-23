<script lang="ts" module>
  export type Lang = 'json' | 'javascript' | 'xml' | 'html' | 'text'
</script>

<script lang="ts">
  // CodeMirror 6 wrapper used for bodies, scripts and the response viewer.
  import { onDestroy, onMount } from 'svelte'
  import { EditorState, Compartment, type Extension } from '@codemirror/state'
  import {
    EditorView,
    keymap,
    lineNumbers,
    highlightActiveLine,
    highlightActiveLineGutter,
    drawSelection,
    placeholder as cmPlaceholder,
  } from '@codemirror/view'
  import { defaultKeymap, history, historyKeymap, indentWithTab } from '@codemirror/commands'
  import { searchKeymap, highlightSelectionMatches } from '@codemirror/search'
  import {
    bracketMatching,
    foldGutter,
    foldKeymap,
    indentOnInput,
    codeFolding,
  } from '@codemirror/language'
  import { closeBrackets, closeBracketsKeymap, autocompletion, type CompletionSource } from '@codemirror/autocomplete'
  import { linter, lintGutter } from '@codemirror/lint'
  import { json, jsonParseLinter } from '@codemirror/lang-json'
  import { javascript } from '@codemirror/lang-javascript'
  import { xml } from '@codemirror/lang-xml'
  import { html } from '@codemirror/lang-html'
  import { baseTheme, highlight } from './theme'
  import { ws } from '../state/workspace.svelte'

  interface Props {
    value: string
    lang?: Lang
    readonly?: boolean
    placeholder?: string
    lint?: boolean
    wrap?: boolean
    completions?: CompletionSource | null
    onchange?: (value: string) => void
    onmodenter?: () => void
  }

  let {
    value,
    lang = 'text',
    readonly = false,
    placeholder = '',
    lint = false,
    wrap = false,
    completions = null,
    onchange,
    onmodenter,
  }: Props = $props()

  let host: HTMLDivElement
  let view: EditorView | null = null
  const langC = new Compartment()
  const roC = new Compartment()
  const wrapC = new Compartment()
  const fontC = new Compartment()

  function langExt(l: Lang, withLint: boolean, comp: CompletionSource | null): Extension {
    const exts: Extension[] = []
    if (l === 'json') {
      exts.push(json())
      if (withLint) exts.push(linter(jsonParseLinter(), { delay: 400 }), lintGutter())
    } else if (l === 'javascript') {
      exts.push(javascript())
      if (comp) exts.push(javascript().language.data.of({ autocomplete: comp }))
    } else if (l === 'xml') exts.push(xml())
    else if (l === 'html') exts.push(html())
    return exts
  }

  const fontTheme = (size: number) => EditorView.theme({ '.cm-scroller': { fontSize: `${size}px` } })

  onMount(() => {
    view = new EditorView({
      parent: host,
      state: EditorState.create({
        doc: value,
        extensions: [
          lineNumbers(),
          highlightActiveLineGutter(),
          foldGutter({ openText: '▾', closedText: '▸' }),
          codeFolding(),
          history(),
          drawSelection(),
          indentOnInput(),
          bracketMatching(),
          closeBrackets(),
          autocompletion({ activateOnTyping: true }),
          highlightActiveLine(),
          highlightSelectionMatches(),
          keymap.of([
            {
              key: 'Mod-Enter',
              run: () => {
                onmodenter?.()
                return !!onmodenter
              },
            },
            ...closeBracketsKeymap,
            ...defaultKeymap,
            ...searchKeymap,
            ...historyKeymap,
            ...foldKeymap,
            indentWithTab,
          ]),
          baseTheme,
          highlight,
          cmPlaceholder(placeholder),
          langC.of(langExt(lang, lint, completions)),
          roC.of(EditorState.readOnly.of(readonly)),
          wrapC.of(wrap ? EditorView.lineWrapping : []),
          fontC.of(fontTheme(ws.settings.editorFontSize)),
          EditorView.updateListener.of((u) => {
            if (u.docChanged) onchange?.(u.state.doc.toString())
          }),
        ],
      }),
    })
  })

  onDestroy(() => view?.destroy())

  // External value changes (e.g. switching tabs, curl paste) replace the document.
  $effect(() => {
    const v = value
    if (view && v !== view.state.doc.toString()) {
      view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: v } })
    }
  })
  $effect(() => {
    view?.dispatch({ effects: langC.reconfigure(langExt(lang, lint, completions)) })
  })
  $effect(() => {
    view?.dispatch({
      effects: roC.reconfigure(EditorState.readOnly.of(readonly)),
    })
  })
  $effect(() => {
    view?.dispatch({ effects: wrapC.reconfigure(wrap ? EditorView.lineWrapping : []) })
  })
  $effect(() => {
    const size = ws.settings.editorFontSize
    view?.dispatch({ effects: fontC.reconfigure(fontTheme(size)) })
  })

  export function focus() {
    view?.focus()
  }
</script>

<div class="code-editor" bind:this={host}></div>

<style>
  .code-editor {
    height: 100%;
    min-height: 0;
    overflow: hidden;
    user-select: text;
    -webkit-user-select: text;
  }
  .code-editor :global(.cm-editor) {
    height: 100%;
  }
</style>
