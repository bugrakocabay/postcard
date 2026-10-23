//! On-disk persistence under the app data directory.
//!
//! Layout: `collections/<id>.json`, `environments.json`, `history.json`,
//! `cookies.json` (owned by the cookie module), `settings.json`, `session.json`.
//! Files are pretty-printed JSON and written atomically. Loading never fails:
//! a missing file yields defaults, and an unreadable or corrupt file is renamed
//! to `<name>.corrupt-<unix>` and replaced by defaults, with a warning.

use crate::model::{Collection, EnvironmentsFile, FILE_VERSION, HistoryEntry, Settings, Workspace};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

const COLLECTIONS_DIR: &str = "collections";
const ENVIRONMENTS_FILE: &str = "environments.json";
const HISTORY_FILE: &str = "history.json";
const SETTINGS_FILE: &str = "settings.json";
const SESSION_FILE: &str = "session.json";
const COOKIES_FILE: &str = "cookies.json";

/// Error from a save/delete operation. The `Display` text is suitable for the UI.
#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("could not access {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("could not serialize data: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("invalid collection id {0:?}: only letters, digits, '-' and '_' are allowed")]
    InvalidId(String),
}

/// On-disk shape of `history.json`.
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct HistoryFile {
    version: u32,
    entries: Vec<HistoryEntry>,
}

impl Default for HistoryFile {
    fn default() -> Self {
        Self { version: FILE_VERSION, entries: vec![] }
    }
}

/// Handle to the data directory.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
}

/// Keeps only `[A-Za-z0-9_-]`.
fn sanitize_id(id: &str) -> String {
    id.chars().filter(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_').collect()
}

/// Returns the id unchanged if it is non-empty and already safe as a file name.
fn validate_id(id: &str) -> Result<&str, StoreError> {
    if id.is_empty() || sanitize_id(id) != id {
        return Err(StoreError::InvalidId(id.to_string()));
    }
    Ok(id)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// Writes `bytes` to `<path>.tmp-<random>` in the same directory, fsyncs it, then
/// renames it over `path`. The temp file is removed if anything fails.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    let file_name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let mut tmp_name = file_name.to_os_string();
    tmp_name.push(format!(".tmp-{}", uuid::Uuid::new_v4().simple()));
    let tmp = parent.join(tmp_name);

    let result = (|| {
        let mut f = fs::OpenOptions::new().write(true).create_new(true).open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, path)
    })();
    match result {
        Ok(()) => {
            // Best effort: persist the rename itself (not possible on Windows).
            #[cfg(unix)]
            if let Ok(dir) = fs::File::open(parent) {
                let _ = dir.sync_all();
            }
            Ok(())
        }
        Err(e) => {
            let _ = fs::remove_file(&tmp);
            Err(e)
        }
    }
}

/// Pretty-printed JSON with a trailing newline.
fn to_pretty_bytes<T: Serialize + ?Sized>(value: &T) -> Result<Vec<u8>, serde_json::Error> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn strip_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]).unwrap_or(bytes)
}

/// Renames a bad file to `<name>.corrupt-<unix>` (with a numeric suffix on collision).
fn backup_corrupt(path: &Path) -> io::Result<PathBuf> {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let stamp = unix_now();
    let mut n = 0u32;
    loop {
        let suffix = if n == 0 { format!("{stamp}") } else { format!("{stamp}-{n}") };
        let target = path.with_file_name(format!("{name}.corrupt-{suffix}"));
        if !target.exists() {
            fs::rename(path, &target)?;
            return Ok(target);
        }
        n += 1;
    }
}

/// Moves a bad file aside and describes what happened.
fn quarantine(path: &Path, outcome: &str, reason: &dyn std::fmt::Display) -> String {
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    match backup_corrupt(path) {
        Ok(backup) => {
            let backup_name = backup.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
            format!("{name} was unreadable ({reason}); it was renamed to {backup_name} and {outcome}")
        }
        Err(e) => format!(
            "{name} was unreadable ({reason}); it could not be renamed ({e}) and {outcome}"
        ),
    }
}

/// Outcome of reading one JSON file.
enum Loaded<T> {
    Missing,
    Ok(T),
    Bad(String),
}

fn read_json<T: DeserializeOwned>(path: &Path) -> Loaded<T> {
    match fs::read(path) {
        Ok(bytes) => match serde_json::from_slice(strip_bom(&bytes)) {
            Ok(v) => Loaded::Ok(v),
            Err(e) => Loaded::Bad(e.to_string()),
        },
        Err(e) if e.kind() == io::ErrorKind::NotFound => Loaded::Missing,
        Err(e) => Loaded::Bad(e.to_string()),
    }
}

impl Store {
    /// Creates a store rooted at `root` (the directory is created lazily).
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// The data directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Where the cookie jar is persisted (read and written by the cookies module).
    pub fn cookies_path(&self) -> PathBuf {
        self.root.join(COOKIES_FILE)
    }

    fn collections_dir(&self) -> PathBuf {
        self.root.join(COLLECTIONS_DIR)
    }

    /// Loads one file, falling back to `T::default()` (and quarantining the file) when bad.
    fn load_or_default<T: DeserializeOwned + Default>(&self, name: &str, warnings: &mut Vec<String>) -> T {
        let path = self.root.join(name);
        match read_json(&path) {
            Loaded::Ok(v) => v,
            Loaded::Missing => T::default(),
            Loaded::Bad(reason) => {
                warnings.push(quarantine(&path, "defaults were used", &reason));
                T::default()
            }
        }
    }

    /// Loads everything. Never fails: problems become human-readable warnings.
    pub fn load_workspace(&self) -> (Workspace, Vec<String>) {
        let mut warnings = Vec::new();
        if let Err(e) = fs::create_dir_all(self.collections_dir()) {
            warnings.push(format!("could not create data directory {}: {e}", self.root.display()));
        }
        let collections = self.load_collections(&mut warnings);
        let environments = self.load_or_default::<EnvironmentsFile>(ENVIRONMENTS_FILE, &mut warnings);
        let history = self.load_or_default::<HistoryFile>(HISTORY_FILE, &mut warnings).entries;
        let settings = self.load_or_default::<Settings>(SETTINGS_FILE, &mut warnings);
        let session = self.load_or_default::<serde_json::Value>(SESSION_FILE, &mut warnings);
        (Workspace { collections, environments, history, settings, session }, warnings)
    }

    /// Loads every `collections/*.json`, sorted by file name. Bad files are skipped.
    fn load_collections(&self, warnings: &mut Vec<String>) -> Vec<Collection> {
        let dir = self.collections_dir();
        let entries = match fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return vec![],
            Err(e) => {
                warnings.push(format!("could not read {}: {e}", dir.display()));
                return vec![];
            }
        };
        let mut paths: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json") && p.is_file())
            .collect();
        paths.sort();

        let mut out = Vec::new();
        for path in paths {
            let stem = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            let file = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            match read_json::<Collection>(&path) {
                Loaded::Ok(mut c) => {
                    if validate_id(&stem).is_err() {
                        warnings.push(format!(
                            "collections/{file} was skipped: its file name is not a valid collection id"
                        ));
                        continue;
                    }
                    if c.id != stem {
                        // The file name is authoritative, so saving later overwrites this file.
                        warnings.push(format!(
                            "collections/{file} had id {:?}; it was changed to {stem:?} to match the file name",
                            c.id
                        ));
                        c.id = stem;
                    }
                    out.push(c);
                }
                Loaded::Missing => {}
                Loaded::Bad(reason) => {
                    let msg = quarantine(&path, "it was skipped", &reason);
                    warnings.push(format!("collections/{msg}"));
                }
            }
        }
        out
    }

    fn write_json<T: Serialize + ?Sized>(&self, path: PathBuf, value: &T) -> Result<(), StoreError> {
        let bytes = to_pretty_bytes(value)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| StoreError::Io { path: parent.to_path_buf(), source })?;
        }
        write_atomic(&path, &bytes).map_err(|source| StoreError::Io { path, source })
    }

    /// Saves one collection to `collections/<id>.json`.
    pub fn save_collection(&self, collection: &Collection) -> Result<(), StoreError> {
        let id = validate_id(&collection.id)?;
        self.write_json(self.collections_dir().join(format!("{id}.json")), collection)
    }

    /// Deletes a collection's file. A missing file is not an error.
    pub fn delete_collection(&self, id: &str) -> Result<(), StoreError> {
        let path = self.collections_dir().join(format!("{}.json", validate_id(id)?));
        match fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(StoreError::Io { path, source }),
        }
    }

    /// Saves environments, globals and the active environment id.
    pub fn save_environments(&self, file: &EnvironmentsFile) -> Result<(), StoreError> {
        self.write_json(self.root.join(ENVIRONMENTS_FILE), file)
    }

    /// Saves history as `{ "version": 1, "entries": [...] }`. The caller applies any cap.
    pub fn save_history(&self, entries: &[HistoryEntry]) -> Result<(), StoreError> {
        #[derive(Serialize)]
        struct Out<'a> {
            version: u32,
            entries: &'a [HistoryEntry],
        }
        self.write_json(self.root.join(HISTORY_FILE), &Out { version: FILE_VERSION, entries })
    }

    /// Saves application settings.
    pub fn save_settings(&self, settings: &Settings) -> Result<(), StoreError> {
        self.write_json(self.root.join(SETTINGS_FILE), settings)
    }

    /// Saves the opaque UI session value as is.
    pub fn save_session(&self, session: &serde_json::Value) -> Result<(), StoreError> {
        self.write_json(self.root.join(SESSION_FILE), session)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Environment, Item, Request};
    use tempfile::TempDir;

    fn store() -> (TempDir, Store) {
        let dir = TempDir::new().unwrap();
        let store = Store::new(dir.path().join("data"));
        (dir, store)
    }

    fn names_in(dir: &Path) -> Vec<String> {
        let mut v: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        v.sort();
        v
    }

    fn sample_collection(id: &str) -> Collection {
        Collection {
            id: id.into(),
            name: format!("Coll {id}"),
            items: vec![Item::Request(Request { name: "r".into(), url: "http://x".into(), ..Default::default() })],
            ..Default::default()
        }
    }

    #[test]
    fn missing_dir_gives_defaults() {
        let (_d, s) = store();
        let (ws, warnings) = s.load_workspace();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(ws.collections.is_empty());
        assert!(ws.environments.environments.is_empty());
        assert_eq!(ws.environments.active_id, None);
        assert!(ws.history.is_empty());
        assert_eq!(ws.settings, Settings::default());
        assert_eq!(ws.session, serde_json::Value::Null);
        assert!(s.root().is_dir());
    }

    #[test]
    fn save_then_load_everything() {
        let (_d, s) = store();
        let c = sample_collection("abc-123");
        s.save_collection(&c).unwrap();
        let envs = EnvironmentsFile {
            environments: vec![Environment { id: "e1".into(), name: "local".into(), variables: vec![] }],
            active_id: Some("e1".into()),
            ..Default::default()
        };
        s.save_environments(&envs).unwrap();
        let settings = Settings { editor_font_size: 17, ..Default::default() };
        s.save_settings(&settings).unwrap();
        let hist = vec![HistoryEntry { id: "h1".into(), method: "GET".into(), ..Default::default() }];
        s.save_history(&hist).unwrap();
        let session = serde_json::json!({"tabs": [1, 2], "layout": {"w": 300}});
        s.save_session(&session).unwrap();

        let (ws, warnings) = s.load_workspace();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(ws.collections, vec![c]);
        assert_eq!(ws.environments, envs);
        assert_eq!(ws.settings, settings);
        assert_eq!(ws.history, hist);
        assert_eq!(ws.session, session);
    }

    #[test]
    fn save_then_load_collection() {
        let (_d, s) = store();
        let c = sample_collection("one");
        s.save_collection(&c).unwrap();
        let text = std::fs::read_to_string(s.root().join("collections/one.json")).unwrap();
        assert!(text.contains("\"version\": 1"), "pretty printed with version: {text}");
        let (ws, w) = s.load_workspace();
        assert!(w.is_empty());
        assert_eq!(ws.collections, vec![c.clone()]);
        // Saving again overwrites rather than duplicating.
        let mut c2 = c;
        c2.name = "renamed".into();
        s.save_collection(&c2).unwrap();
        let (ws, _) = s.load_workspace();
        assert_eq!(ws.collections, vec![c2]);
    }

    #[test]
    fn history_file_format() {
        let (_d, s) = store();
        s.save_history(&[HistoryEntry { id: "h".into(), ..Default::default() }]).unwrap();
        let v: serde_json::Value =
            serde_json::from_slice(&std::fs::read(s.root().join("history.json")).unwrap()).unwrap();
        assert_eq!(v["version"], 1);
        assert_eq!(v["entries"][0]["id"], "h");
    }

    #[test]
    fn delete_collection_removes_file() {
        let (_d, s) = store();
        s.save_collection(&sample_collection("gone")).unwrap();
        assert!(s.root().join("collections/gone.json").exists());
        s.delete_collection("gone").unwrap();
        assert!(!s.root().join("collections/gone.json").exists());
        // Deleting something that does not exist is fine.
        s.delete_collection("gone").unwrap();
        let (ws, _) = s.load_workspace();
        assert!(ws.collections.is_empty());
    }

    #[test]
    fn corrupt_file_is_backed_up_and_defaulted() {
        let (_d, s) = store();
        std::fs::create_dir_all(s.root()).unwrap();
        std::fs::write(s.root().join("environments.json"), "{").unwrap();
        let (ws, warnings) = s.load_workspace();
        assert!(ws.environments.environments.is_empty());
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("environments.json"), "{warnings:?}");
        assert!(warnings[0].contains("environments.json.corrupt-"), "{warnings:?}");
        let names = names_in(s.root());
        assert!(!names.contains(&"environments.json".to_string()), "{names:?}");
        let backup = names.iter().find(|n| n.starts_with("environments.json.corrupt-")).expect("backup");
        assert_eq!(std::fs::read_to_string(s.root().join(backup)).unwrap(), "{");
        // A second load is clean (the bad file is gone).
        let (_, warnings) = s.load_workspace();
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn corrupt_settings_history_and_session() {
        let (_d, s) = store();
        std::fs::create_dir_all(s.root()).unwrap();
        for n in ["settings.json", "history.json", "session.json"] {
            std::fs::write(s.root().join(n), [0xff, 0xfe, b'x']).unwrap();
        }
        let (ws, warnings) = s.load_workspace();
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert_eq!(ws.settings, Settings::default());
        assert!(ws.history.is_empty());
        assert_eq!(ws.session, serde_json::Value::Null);
    }

    #[test]
    fn corrupt_collection_skipped_others_load() {
        let (_d, s) = store();
        let good = sample_collection("good");
        s.save_collection(&good).unwrap();
        std::fs::write(s.root().join("collections/bad.json"), "not json").unwrap();
        let (ws, warnings) = s.load_workspace();
        assert_eq!(ws.collections, vec![good]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("bad.json.corrupt-"), "{warnings:?}");
        let names = names_in(&s.root().join("collections"));
        assert!(names.iter().any(|n| n.starts_with("bad.json.corrupt-")), "{names:?}");
        assert!(!names.contains(&"bad.json".to_string()));
    }

    #[test]
    fn collection_id_follows_filename() {
        let (_d, s) = store();
        std::fs::create_dir_all(s.root().join("collections")).unwrap();
        std::fs::write(s.root().join("collections/mine.json"), r#"{"id":"other","name":"N"}"#).unwrap();
        let (ws, warnings) = s.load_workspace();
        assert_eq!(ws.collections.len(), 1);
        assert_eq!(ws.collections[0].id, "mine");
        assert_eq!(ws.collections[0].name, "N");
        assert_eq!(warnings.len(), 1, "{warnings:?}");
    }

    #[test]
    fn partial_hand_edited_files_use_defaults() {
        let (_d, s) = store();
        std::fs::create_dir_all(s.root()).unwrap();
        std::fs::write(s.root().join("settings.json"), "\u{feff}{\"editorFontSize\": 20}").unwrap();
        let (ws, warnings) = s.load_workspace();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(ws.settings.editor_font_size, 20);
        assert_eq!(ws.settings.default_timeout_ms, 30_000);
    }

    #[test]
    fn unrelated_files_in_collections_are_ignored() {
        let (_d, s) = store();
        let dir = s.root().join("collections");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("notes.txt"), "hi").unwrap();
        std::fs::write(dir.join("x.json.tmp-1"), "{").unwrap();
        std::fs::create_dir(dir.join("sub.json")).unwrap();
        let (ws, warnings) = s.load_workspace();
        assert!(ws.collections.is_empty());
        assert!(warnings.is_empty(), "{warnings:?}");
    }

    #[test]
    fn atomic_write_leaves_no_tmp() {
        let dir = TempDir::new().unwrap();
        let p = dir.path().join("a.json");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), b"two");
        assert_eq!(names_in(dir.path()), vec!["a.json".to_string()]);
    }

    #[test]
    fn atomic_write_failure_leaves_target_and_no_tmp() {
        let dir = TempDir::new().unwrap();
        // The target is a non-empty directory, so the final rename fails.
        let p = dir.path().join("t");
        std::fs::create_dir(&p).unwrap();
        std::fs::write(p.join("f"), "x").unwrap();
        assert!(write_atomic(&p, b"data").is_err());
        assert_eq!(names_in(dir.path()), vec!["t".to_string()]);
    }

    #[test]
    fn bad_collection_ids_are_rejected() {
        let (_d, s) = store();
        for id in ["../evil", "a/b", "a\\b", "", "x.json", "a b", "..", "é"] {
            let c = sample_collection(id);
            assert!(matches!(s.save_collection(&c), Err(StoreError::InvalidId(_))), "id {id:?}");
            assert!(matches!(s.delete_collection(id), Err(StoreError::InvalidId(_))), "id {id:?}");
        }
        assert!(!s.root().join("evil.json").exists());
        assert!(!s.root().parent().unwrap().join("evil.json").exists());
    }

    #[test]
    fn sanitize_id_basics() {
        assert_eq!(sanitize_id("abc-DEF_123"), "abc-DEF_123");
        assert_eq!(sanitize_id("../a"), "a");
    }

    #[test]
    fn cookies_path_is_in_root() {
        let (_d, s) = store();
        assert_eq!(s.cookies_path(), s.root().join("cookies.json"));
    }
}
