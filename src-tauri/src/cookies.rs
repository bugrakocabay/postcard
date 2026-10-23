//! Persistent cookie jar shared by all requests, plus listing/deleting for the cookie manager.

use std::fs::File;
use std::io::{self, BufReader};
use std::path::Path;
use std::sync::MutexGuard;

use cookie_store::CookieStore;
use reqwest_cookie_store::CookieStoreMutex;
use serde::Serialize;

/// A cookie as shown in the cookie manager.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CookieInfo {
    pub domain: String,
    pub path: String,
    pub name: String,
    pub value: String,
    /// RFC 3339 UTC timestamp, or `None` for session cookies.
    pub expires: Option<String>,
    pub secure: bool,
    pub http_only: bool,
}

fn lock(jar: &CookieStoreMutex) -> MutexGuard<'_, CookieStore> {
    // A poisoned jar is still usable data.
    jar.lock().unwrap_or_else(|e| e.into_inner())
}

/// Loads the jar from `path`. A missing or corrupt file yields an empty jar; expired cookies are dropped.
pub fn load_jar(path: &Path) -> CookieStoreMutex {
    let store = File::open(path)
        .ok()
        .and_then(|f| cookie_store::serde::json::load(BufReader::new(f)).ok())
        .unwrap_or_default();
    CookieStoreMutex::new(store)
}

/// Saves the jar to `path` atomically (temp file + rename).
///
/// Session (non-persistent) cookies are saved too, so they survive app restarts the way
/// they do in Postman; the only cookies dropped on load are the expired ones.
pub fn save_jar(jar: &CookieStoreMutex, path: &Path) -> io::Result<()> {
    let mut buf = Vec::new();
    cookie_store::serde::json::save_incl_expired_and_nonpersistent(&lock(jar), &mut buf)
        .map_err(|e| io::Error::other(e.to_string()))?;
    write_atomic(path, &buf)
}

/// Serialises saves: two requests finishing together must not interleave writes.
fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    static SAVE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    let _guard = SAVE.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    crate::storage::write_atomic(path, data)
}

/// All unexpired cookies, sorted by domain, path, name.
pub fn list(jar: &CookieStoreMutex) -> Vec<CookieInfo> {
    let store = lock(jar);
    let mut out: Vec<CookieInfo> = store
        .iter_unexpired()
        .map(|c| CookieInfo {
            domain: String::from(&c.domain),
            path: c.path.to_string(),
            name: c.name().to_string(),
            value: c.value().to_string(),
            expires: match &c.expires {
                cookie_store::CookieExpiration::AtUtc(t) => {
                    chrono::DateTime::from_timestamp(t.unix_timestamp(), 0).map(|d| d.to_rfc3339())
                }
                cookie_store::CookieExpiration::SessionEnd => None,
            },
            secure: c.secure().unwrap_or(false),
            http_only: c.http_only().unwrap_or(false),
        })
        .collect();
    out.sort_by(|a, b| (&a.domain, &a.path, &a.name).cmp(&(&b.domain, &b.path, &b.name)));
    out
}

/// Removes one cookie, identified as in [`CookieInfo`]. Missing cookies are ignored.
pub fn delete(jar: &CookieStoreMutex, domain: &str, path: &str, name: &str) {
    lock(jar).remove(domain, path, name);
}

/// Removes every cookie.
pub fn clear(jar: &CookieStoreMutex) {
    lock(jar).clear();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> url::Url {
        url::Url::parse(s).unwrap()
    }

    fn sample_jar() -> CookieStoreMutex {
        let jar = CookieStoreMutex::default();
        {
            let mut s = jar.lock().unwrap();
            s.parse("sid=abc; Path=/; HttpOnly", &url("http://example.com/")).unwrap();
            s.parse("pref=1; Path=/a; Max-Age=3600; Secure", &url("https://example.com/a/b")).unwrap();
            s.parse("other=x; Path=/", &url("http://127.0.0.1:8080/")).unwrap();
        }
        jar
    }

    #[test]
    fn list_reports_fields_sorted() {
        let l = list(&sample_jar());
        assert_eq!(l.len(), 3);
        assert_eq!(l[0].domain, "127.0.0.1");
        let sid = l.iter().find(|c| c.name == "sid").unwrap();
        assert_eq!((sid.domain.as_str(), sid.path.as_str(), sid.value.as_str()), ("example.com", "/", "abc"));
        assert!(sid.http_only && !sid.secure && sid.expires.is_none());
        let pref = l.iter().find(|c| c.name == "pref").unwrap();
        assert_eq!(pref.path, "/a");
        assert!(pref.secure && pref.expires.is_some());
        let json = serde_json::to_value(pref).unwrap();
        assert!(json.get("httpOnly").is_some());
    }

    #[test]
    fn delete_and_clear() {
        let jar = sample_jar();
        delete(&jar, "example.com", "/", "sid");
        delete(&jar, "example.com", "/", "nope");
        assert_eq!(list(&jar).len(), 2);
        clear(&jar);
        assert!(list(&jar).is_empty());
    }

    #[test]
    fn save_and_load_roundtrip_includes_session_cookies() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("sub").join("cookies.json");
        save_jar(&sample_jar(), &path).unwrap();
        assert!(!dir.path().join("sub").join("cookies.json.tmp").exists());
        let loaded = load_jar(&path);
        let l = list(&loaded);
        assert_eq!(l.len(), 3);
        assert!(l.iter().any(|c| c.name == "sid" && c.expires.is_none()));
        // Saving again overwrites the previous file.
        clear(&loaded);
        save_jar(&loaded, &path).unwrap();
        assert!(list(&load_jar(&path)).is_empty());
    }

    #[test]
    fn missing_or_corrupt_file_gives_empty_jar() {
        let dir = tempfile::tempdir().unwrap();
        assert!(list(&load_jar(&dir.path().join("nope.json"))).is_empty());
        let bad = dir.path().join("bad.json");
        std::fs::write(&bad, "{ this is not json").unwrap();
        assert!(list(&load_jar(&bad)).is_empty());
    }
}
