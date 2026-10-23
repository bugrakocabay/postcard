/**
 * Re-indents JSON text without parsing values, so numbers like 12345678901234567890 or 1.0
 * are shown exactly as the server sent them. Returns null if the text is not valid JSON.
 */
export function formatJson(text: string, indent = '  '): string | null {
  try {
    JSON.parse(text)
  } catch {
    return null
  }
  let out = ''
  let depth = 0
  let i = 0
  const n = text.length
  const newline = () => '\n' + indent.repeat(depth)
  while (i < n) {
    const ch = text[i]
    if (ch === '"') {
      // Copy the whole string, including escapes.
      let j = i + 1
      while (j < n && text[j] !== '"') j += text[j] === '\\' ? 2 : 1
      out += text.slice(i, j + 1)
      i = j + 1
      continue
    }
    if (ch === ' ' || ch === '\n' || ch === '\r' || ch === '\t') {
      i++
      continue
    }
    if (ch === '{' || ch === '[') {
      // Keep empty containers on one line.
      let j = i + 1
      while (j < n && /\s/.test(text[j])) j++
      const close = ch === '{' ? '}' : ']'
      if (text[j] === close) {
        out += ch + close
        i = j + 1
        continue
      }
      depth++
      out += ch + newline()
    } else if (ch === '}' || ch === ']') {
      depth--
      out += newline() + ch
    } else if (ch === ',') {
      out += ',' + newline()
    } else if (ch === ':') {
      out += ': '
    } else {
      out += ch
    }
    i++
  }
  return out
}
