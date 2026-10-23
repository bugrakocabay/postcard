//! Data types shared by storage, the send pipeline and the frontend (via IPC).
//! Field names are camelCase on the wire; enums are tagged with `"type"`.

use serde::{Deserialize, Serialize};

pub const FILE_VERSION: u32 = 1;

fn default_true() -> bool {
    true
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct KeyValue {
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl KeyValue {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Self {
        Self { key: key.into(), value: value.into(), enabled: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Variable {
    pub key: String,
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub secret: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum ApiKeyLocation {
    #[default]
    Header,
    Query,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Auth {
    None,
    #[default]
    Inherit,
    Bearer {
        #[serde(default)]
        token: String,
    },
    Basic {
        #[serde(default)]
        username: String,
        #[serde(default)]
        password: String,
    },
    ApiKey {
        #[serde(default)]
        key: String,
        #[serde(default)]
        value: String,
        #[serde(default)]
        location: ApiKeyLocation,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum FormFieldKind {
    #[default]
    Text,
    File,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct FormField {
    pub key: String,
    /// Text value, or a file path when `kind` is `File`.
    pub value: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub kind: FormFieldKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Body {
    #[default]
    None,
    Json {
        #[serde(default)]
        text: String,
    },
    Raw {
        #[serde(default)]
        text: String,
        #[serde(default, rename = "contentType")]
        content_type: String,
    },
    UrlEncoded {
        #[serde(default)]
        fields: Vec<KeyValue>,
    },
    Multipart {
        #[serde(default)]
        fields: Vec<FormField>,
    },
    Binary {
        #[serde(default)]
        path: String,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Scripts {
    pub pre: String,
    pub post: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct RequestSettings {
    pub timeout_ms: u64,
    pub follow_redirects: bool,
    pub verify_ssl: bool,
}

impl Default for RequestSettings {
    fn default() -> Self {
        Self { timeout_ms: 30_000, follow_redirects: true, verify_ssl: true }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Request {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    /// Mirrors the URL query (plus disabled rows). The URL is the source of truth when sending.
    pub params: Vec<KeyValue>,
    pub headers: Vec<KeyValue>,
    pub auth: Auth,
    pub body: Body,
    pub scripts: Scripts,
    pub settings: RequestSettings,
}

impl Default for Request {
    fn default() -> Self {
        Self {
            id: new_id(),
            name: "New request".into(),
            method: "GET".into(),
            url: String::new(),
            params: vec![],
            headers: vec![],
            auth: Auth::Inherit,
            body: Body::None,
            scripts: Scripts::default(),
            settings: RequestSettings::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Folder {
    pub id: String,
    pub name: String,
    pub items: Vec<Item>,
    pub auth: Auth,
    pub scripts: Scripts,
}

impl Default for Folder {
    fn default() -> Self {
        Self { id: new_id(), name: "New folder".into(), items: vec![], auth: Auth::Inherit, scripts: Scripts::default() }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Item {
    Folder(Folder),
    Request(Request),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Collection {
    pub version: u32,
    pub id: String,
    pub name: String,
    pub items: Vec<Item>,
    pub auth: Auth,
    pub scripts: Scripts,
}

impl Default for Collection {
    fn default() -> Self {
        Self {
            version: FILE_VERSION,
            id: new_id(),
            name: "New collection".into(),
            items: vec![],
            auth: Auth::None,
            scripts: Scripts::default(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Environment {
    pub id: String,
    pub name: String,
    pub variables: Vec<Variable>,
}

impl Default for Environment {
    fn default() -> Self {
        Self { id: new_id(), name: "New environment".into(), variables: vec![] }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct EnvironmentsFile {
    pub version: u32,
    pub environments: Vec<Environment>,
    pub globals: Vec<Variable>,
    pub active_id: Option<String>,
}

impl Default for EnvironmentsFile {
    fn default() -> Self {
        Self { version: FILE_VERSION, environments: vec![], globals: vec![], active_id: None }
    }
}

impl EnvironmentsFile {
    pub fn active(&self) -> Option<&Environment> {
        let id = self.active_id.as_ref()?;
        self.environments.iter().find(|e| &e.id == id)
    }

    pub fn active_mut(&mut self) -> Option<&mut Environment> {
        let id = self.active_id.clone()?;
        self.environments.iter_mut().find(|e| e.id == id)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct HistoryEntry {
    pub id: String,
    /// Unix time in milliseconds.
    pub timestamp: i64,
    pub method: String,
    /// URL after variable resolution.
    pub url: String,
    pub status: Option<u16>,
    pub time_ms: u64,
    /// The request as written (variables not resolved).
    pub request: Request,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub enum WindowsShell {
    #[default]
    Wsl,
    Cmd,
    PowerShell,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct ShellSettings {
    pub windows: WindowsShell,
    pub wsl_distro: Option<String>,
    pub unix_custom: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub version: u32,
    pub default_timeout_ms: u64,
    pub default_verify_ssl: bool,
    pub cookie_jar_enabled: bool,
    pub shell: ShellSettings,
    pub editor_font_size: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: FILE_VERSION,
            default_timeout_ms: 30_000,
            default_verify_ssl: true,
            cookie_jar_enabled: true,
            shell: ShellSettings::default(),
            editor_font_size: 13,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Workspace {
    pub collections: Vec<Collection>,
    pub environments: EnvironmentsFile,
    pub history: Vec<HistoryEntry>,
    pub settings: Settings,
    /// Opaque UI session state owned by the frontend (open tabs, drafts, layout).
    pub session: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_roundtrip_json() {
        let r = Request {
            auth: Auth::Bearer { token: "{{token}}".into() },
            body: Body::Json { text: "{\"a\":1}".into() },
            headers: vec![KeyValue::new("X-A", "1")],
            ..Default::default()
        };
        let s = serde_json::to_string(&r).unwrap();
        assert!(s.contains("\"type\":\"bearer\""), "{s}");
        assert!(s.contains("\"timeoutMs\":30000"), "{s}");
        assert!(s.contains("\"type\":\"json\""), "{s}");
        let back: Request = serde_json::from_str(&s).unwrap();
        assert_eq!(back, r);
    }

    #[test]
    fn partial_request_uses_defaults() {
        let r: Request = serde_json::from_str(r#"{"url":"x"}"#).unwrap();
        assert_eq!(r.method, "GET");
        assert_eq!(r.url, "x");
        assert_eq!(r.settings.timeout_ms, 30_000);
        assert!(r.settings.follow_redirects);
        assert_eq!(r.auth, Auth::Inherit);
    }

    #[test]
    fn enum_wire_names() {
        let a = serde_json::to_string(&Auth::ApiKey { key: "k".into(), value: "v".into(), location: ApiKeyLocation::Query }).unwrap();
        assert_eq!(a, r#"{"type":"apiKey","key":"k","value":"v","location":"query"}"#);
        let b = serde_json::to_string(&Body::Raw { text: "x".into(), content_type: "text/plain".into() }).unwrap();
        assert_eq!(b, r#"{"type":"raw","text":"x","contentType":"text/plain"}"#);
        let u = serde_json::to_string(&Body::UrlEncoded { fields: vec![] }).unwrap();
        assert_eq!(u, r#"{"type":"urlEncoded","fields":[]}"#);
        let item = serde_json::to_value(Item::Folder(Folder::default())).unwrap();
        assert_eq!(item["type"], "folder");
        let sh = serde_json::to_string(&WindowsShell::PowerShell).unwrap();
        assert_eq!(sh, "\"powerShell\"");
    }

    #[test]
    fn keyvalue_enabled_defaults_true() {
        let kv: KeyValue = serde_json::from_str(r#"{"key":"a","value":"b"}"#).unwrap();
        assert!(kv.enabled);
    }
}
