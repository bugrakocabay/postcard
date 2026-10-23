// Typed wrappers around the Rust commands in src-tauri/src/commands.rs.
import { invoke } from '@tauri-apps/api/core'
import type {
  Ancestor,
  Collection,
  CookieInfo,
  EnvironmentsFile,
  LoadResult,
  ParsedCurl,
  Request,
  SendInput,
  SendResult,
  Settings,
} from './types'

export const api = {
  loadWorkspace: () => invoke<LoadResult>('load_workspace'),
  saveCollection: (collection: Collection) => invoke<void>('save_collection', { collection }),
  deleteCollection: (id: string) => invoke<void>('delete_collection', { id }),
  saveEnvironments: (file: EnvironmentsFile) => invoke<void>('save_environments', { file }),
  saveSettings: (settings: Settings) => invoke<void>('save_settings', { settings }),
  saveSession: (session: unknown) => invoke<void>('save_session', { session }),
  clearHistory: () => invoke<void>('clear_history'),
  sendRequest: (input: SendInput) => invoke<SendResult>('send_request', { input }),
  cancelRequest: (sendId: string) => invoke<void>('cancel_request', { sendId }),
  parseCurl: (text: string) => invoke<ParsedCurl>('parse_curl', { text }),
  toCurl: (request: Request, ancestors: Ancestor[], resolved: boolean) =>
    invoke<string>('to_curl', { request, ancestors, resolved }),
  saveResponseBody: (responseId: string, path: string) =>
    invoke<void>('save_response_body', { responseId, path }),
  listCookies: () => invoke<CookieInfo[]>('list_cookies'),
  deleteCookie: (domain: string, path: string, name: string) =>
    invoke<void>('delete_cookie', { domain, path, name }),
  clearCookies: () => invoke<void>('clear_cookies'),
}

export function errorMessage(e: unknown): string {
  if (typeof e === 'string') return e
  if (e instanceof Error) return e.message
  return JSON.stringify(e)
}
