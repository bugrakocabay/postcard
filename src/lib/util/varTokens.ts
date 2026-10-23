export interface VarToken {
  from: number
  to: number
  name: string
}

const VAR_RE = /\{\{\s*([^{}\n]+?)\s*\}\}/g

/** Finds `{{name}}` spans in text. */
export function findVarTokens(text: string): VarToken[] {
  const out: VarToken[] = []
  for (const m of text.matchAll(VAR_RE)) {
    out.push({ from: m.index!, to: m.index! + m[0].length, name: m[1] })
  }
  return out
}

export const DYNAMIC_VARS = ['$uuid', '$timestamp', '$isoTimestamp', '$randomInt']
