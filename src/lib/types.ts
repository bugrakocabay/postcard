// Mirrors src-tauri/src/model.rs and the IPC types in pipeline.rs / http.rs / cookies.rs.

export interface KeyValue {
  key: string
  value: string
  enabled: boolean
}

export interface Variable {
  key: string
  value: string
  enabled: boolean
  secret: boolean
}

export type ApiKeyLocation = 'header' | 'query'

export type Auth =
  | { type: 'none' }
  | { type: 'inherit' }
  | { type: 'bearer'; token: string }
  | { type: 'basic'; username: string; password: string }
  | { type: 'apiKey'; key: string; value: string; location: ApiKeyLocation }

export type FormFieldKind = 'text' | 'file'

export interface FormField {
  key: string
  value: string
  enabled: boolean
  kind: FormFieldKind
}

export type Body =
  | { type: 'none' }
  | { type: 'json'; text: string }
  | { type: 'raw'; text: string; contentType: string }
  | { type: 'urlEncoded'; fields: KeyValue[] }
  | { type: 'multipart'; fields: FormField[] }
  | { type: 'binary'; path: string }

export type BodyType = Body['type']

export interface Scripts {
  pre: string
  post: string
}

export interface RequestSettings {
  timeoutMs: number
  followRedirects: boolean
  verifySsl: boolean
}

export interface Request {
  id: string
  name: string
  method: string
  url: string
  params: KeyValue[]
  headers: KeyValue[]
  auth: Auth
  body: Body
  scripts: Scripts
  settings: RequestSettings
}

export interface Folder {
  id: string
  name: string
  items: Item[]
  auth: Auth
  scripts: Scripts
}

export type Item = ({ type: 'folder' } & Folder) | ({ type: 'request' } & Request)
export type FolderItem = Extract<Item, { type: 'folder' }>
export type RequestItem = Extract<Item, { type: 'request' }>

export interface Collection {
  version: number
  id: string
  name: string
  items: Item[]
  auth: Auth
  scripts: Scripts
}

export interface Environment {
  id: string
  name: string
  variables: Variable[]
}

export interface EnvironmentsFile {
  version: number
  environments: Environment[]
  globals: Variable[]
  activeId: string | null
}

export interface HistoryEntry {
  id: string
  timestamp: number
  method: string
  url: string
  status: number | null
  timeMs: number
  request: Request
}

export type WindowsShell = 'wsl' | 'cmd' | 'powerShell'

export interface ShellSettings {
  windows: WindowsShell
  wslDistro: string | null
  unixCustom: string | null
}

export interface Settings {
  version: number
  defaultTimeoutMs: number
  defaultVerifySsl: boolean
  cookieJarEnabled: boolean
  shell: ShellSettings
  editorFontSize: number
}

export interface Workspace {
  collections: Collection[]
  environments: EnvironmentsFile
  history: HistoryEntry[]
  settings: Settings
  session: unknown
}

export interface LoadResult {
  workspace: Workspace
  warnings: string[]
}

// ---- send pipeline ----

export interface Ancestor {
  auth: Auth
  scripts: Scripts
}

export interface SendInput {
  sendId: string
  request: Request
  ancestors: Ancestor[]
}

export type LogLevel = 'log' | 'info' | 'warn' | 'error'

export interface LogLine {
  level: LogLevel
  message: string
}

export type BodyPayload =
  | { type: 'text'; text: string }
  | { type: 'image'; base64: string; mime: string }
  | { type: 'binary' }
  | { type: 'tooLarge' }

export interface ResponseMeta {
  status: number
  statusText: string
  headers: [string, string][]
  sizeBytes: number
  timeMs: number
  contentType: string | null
  setCookies: string[]
  finalUrl: string
}

export interface SendResult {
  responseId: string
  response: ResponseMeta | null
  body: BodyPayload | null
  error: string | null
  logs: LogLine[]
  environments: EnvironmentsFile | null
  historyEntry: HistoryEntry
}

export interface ParsedCurl {
  request: Request
  warnings: string[]
}

export interface CookieInfo {
  domain: string
  path: string
  name: string
  value: string
  expires: string | null
  secure: boolean
  httpOnly: boolean
}
