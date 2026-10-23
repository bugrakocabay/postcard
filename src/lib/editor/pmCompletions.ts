import type { CompletionContext, CompletionResult } from '@codemirror/autocomplete'

const API: [string, string][] = [
  ['pm.environment.get(key)', 'Get a variable from the active environment'],
  ['pm.environment.set(key, value)', 'Set a variable in the active environment (saved)'],
  ['pm.environment.unset(key)', 'Remove a variable from the active environment'],
  ['pm.environment.has(key)', 'True if the environment has the variable'],
  ['pm.environment.toObject()', 'All environment variables as an object'],
  ['pm.globals.get(key)', 'Get a global variable'],
  ['pm.globals.set(key, value)', 'Set a global variable (saved)'],
  ['pm.globals.unset(key)', 'Remove a global variable'],
  ['pm.globals.has(key)', 'True if the global exists'],
  ['pm.variables.get(key)', 'Resolve through runtime → environment → globals'],
  ['pm.variables.set(key, value)', 'Set a runtime variable for this send only'],
  ['pm.request.method', 'Request method (assignable in pre-request)'],
  ['pm.request.url', 'Request URL (assignable in pre-request)'],
  ['pm.request.body', 'Request body text (assignable in pre-request)'],
  ['pm.request.headers.add({ key, value })', 'Add a header'],
  ['pm.request.headers.upsert({ key, value })', 'Add or replace a header'],
  ['pm.request.headers.remove(key)', 'Remove a header'],
  ['pm.request.headers.get(key)', 'Get a header value'],
  ['pm.response.code', 'Status code'],
  ['pm.response.status', 'Status text'],
  ['pm.response.responseTime', 'Response time in ms'],
  ['pm.response.json()', 'Parse body as JSON'],
  ['pm.response.text()', 'Body as text'],
  ['pm.response.headers.get(name)', 'Response header (case-insensitive)'],
  ['pm.shell(command)', 'Run a terminal command and return its output'],
  ['console.log()', 'Print to the console panel'],
]

export function pmCompletions(ctx: CompletionContext): CompletionResult | null {
  const word = ctx.matchBefore(/[\w.]+/)
  if (!word || (word.from === word.to && !ctx.explicit)) return null
  if (!/^(p|pm|c|co|con|cons|conso|consol|console)/.test(word.text) && !ctx.explicit) return null
  return {
    from: word.from,
    options: API.map(([label, info]) => {
      const call = label.indexOf('(')
      const apply = call >= 0 ? label.slice(0, call + 1) : label
      return { label, detail: '', info, apply, type: call >= 0 ? 'function' : 'property' }
    }),
    validFor: /^[\w.]*$/,
  }
}
