import { EditorView } from '@codemirror/view'
import { HighlightStyle, syntaxHighlighting } from '@codemirror/language'
import { tags as t } from '@lezer/highlight'

export const baseTheme = EditorView.theme(
  {
    '&': { color: 'var(--text)', backgroundColor: 'transparent', height: '100%' },
    '.cm-scroller': { fontFamily: 'var(--mono)', lineHeight: '1.5' },
    '.cm-content': { caretColor: 'var(--accent)' },
    '&.cm-focused': { outline: 'none' },
    '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--accent)' },
    '&.cm-focused > .cm-scroller > .cm-selectionLayer .cm-selectionBackground, .cm-selectionBackground, ::selection':
      { backgroundColor: '#34466e !important' },
    '.cm-gutters': { backgroundColor: 'transparent', color: 'var(--faint)', border: 'none' },
    '.cm-activeLine': { backgroundColor: '#ffffff06' },
    '.cm-activeLineGutter': { backgroundColor: 'transparent', color: 'var(--muted)' },
    '.cm-foldPlaceholder': { backgroundColor: 'var(--panel-2)', border: 'none', color: 'var(--muted)' },
    '.cm-tooltip': { backgroundColor: 'var(--panel-2)', border: '1px solid var(--border-strong)', color: 'var(--text)' },
    '.cm-panels': { backgroundColor: 'var(--panel)', color: 'var(--text)' },
    '.cm-panels.cm-panels-top': { borderBottom: '1px solid var(--border)' },
    '.cm-search input, .cm-search button': { fontSize: '12px' },
    '.cm-textfield': { backgroundColor: 'var(--input)', border: '1px solid var(--border)', borderRadius: '3px' },
    '.cm-button': { backgroundImage: 'none', backgroundColor: 'var(--panel-2)', border: '1px solid var(--border)', borderRadius: '3px' },
    '.cm-searchMatch': { backgroundColor: '#e8b04a40' },
    '.cm-searchMatch-selected': { backgroundColor: '#e8b04a80' },
    '.cm-matchingBracket': { backgroundColor: '#ffffff18 !important', outline: 'none' },
    '.cm-placeholder': { color: 'var(--faint)' },
    '.cm-lintRange-error': { backgroundImage: 'none', textDecoration: 'underline wavy var(--danger)' },
  },
  { dark: true },
)

export const highlight = syntaxHighlighting(
  HighlightStyle.define([
    { tag: [t.propertyName], color: '#8fc1ff' },
    { tag: [t.string, t.special(t.string)], color: '#e3a77b' },
    { tag: [t.number, t.bool, t.null, t.atom], color: '#b8d98a' },
    { tag: [t.keyword, t.operatorKeyword, t.controlKeyword, t.definitionKeyword, t.moduleKeyword], color: '#c792ea' },
    { tag: [t.comment, t.lineComment, t.blockComment], color: '#6a7080', fontStyle: 'italic' },
    { tag: [t.function(t.variableName), t.function(t.propertyName)], color: '#82aaff' },
    { tag: [t.variableName], color: '#dcdde0' },
    { tag: [t.tagName], color: '#f07178' },
    { tag: [t.attributeName], color: '#ffcb6b' },
    { tag: [t.attributeValue], color: '#e3a77b' },
    { tag: [t.punctuation, t.bracket, t.separator], color: '#9da1aa' },
    { tag: [t.invalid], color: 'var(--danger)' },
  ]),
)
