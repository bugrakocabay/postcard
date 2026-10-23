//! Variable resolution: `{{name}}` substitution with runtime > environment > globals
//! priority, nested references, cycle protection and dynamic `$` variables.

use std::collections::{BTreeSet, HashMap};

use crate::model::{Auth, Body, Environment, FormField, KeyValue, Request, Variable};

/// Maximum depth of nested variable references (`a={{b}}`, `b={{c}}`, ...).
const MAX_DEPTH: usize = 10;

/// Variable values visible to a request, split by source. Lookup priority is
/// `runtime` (set by scripts) > `env` (active environment) > `globals`.
#[derive(Default, Clone, Debug)]
pub struct VarScope {
    pub runtime: HashMap<String, String>,
    pub env: HashMap<String, String>,
    pub globals: HashMap<String, String>,
}

/// Collects enabled, non-empty-key variables. If a key repeats, the first row wins.
fn enabled_map(vars: &[Variable]) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for v in vars.iter().filter(|v| v.enabled && !v.key.is_empty()) {
        map.entry(v.key.clone()).or_insert_with(|| v.value.clone());
    }
    map
}

impl VarScope {
    /// Builds a scope from the active environment and globals (enabled variables only).
    /// `runtime` starts empty; see [`VarScope::with_runtime`].
    pub fn from(env: Option<&Environment>, globals: &[Variable]) -> Self {
        Self {
            runtime: HashMap::new(),
            env: env.map(|e| enabled_map(&e.variables)).unwrap_or_default(),
            globals: enabled_map(globals),
        }
    }

    /// Replaces the runtime (script-set) variables.
    pub fn with_runtime(mut self, runtime: impl IntoIterator<Item = (String, String)>) -> Self {
        self.runtime = runtime.into_iter().collect();
        self
    }

    /// Looks up a user-defined variable: runtime, then environment, then globals.
    pub fn get(&self, key: &str) -> Option<String> {
        self.runtime
            .get(key)
            .or_else(|| self.env.get(key))
            .or_else(|| self.globals.get(key))
            .cloned()
    }
}

/// True if `s` contains at least one `{{name}}` token.
pub fn has_vars(s: &str) -> bool {
    s.contains("{{") && s.contains("}}") && find_token(s, 0).is_some()
}

/// Finds the next `{{ name }}` token at or after byte offset `from`.
/// Returns `(start, end, trimmed_name)` where `end` is exclusive. A token's inner text
/// may not contain `{`, `}` or a newline, and its trimmed name must be non-empty.
fn find_token(s: &str, from: usize) -> Option<(usize, usize, &str)> {
    let bytes = s.as_bytes();
    let mut i = from;
    while let Some(off) = s[i..].find("{{") {
        let start = i + off;
        let inner_start = start + 2;
        let mut j = inner_start;
        while j < bytes.len() && !matches!(bytes[j], b'{' | b'}' | b'\n') {
            j += 1;
        }
        if j + 1 < bytes.len() && bytes[j] == b'}' && bytes[j + 1] == b'}' {
            let name = s[inner_start..j].trim();
            if !name.is_empty() {
                return Some((start, j + 2, name));
            }
        }
        i = start + 1; // `{` is one byte, so this stays on a char boundary
    }
    None
}

/// Value of a dynamic (`$`-prefixed) variable, generated fresh on every call.
fn dynamic(name: &str) -> Option<String> {
    match name {
        "$uuid" => Some(uuid::Uuid::new_v4().to_string()),
        "$timestamp" => Some(chrono::Utc::now().timestamp().to_string()),
        "$isoTimestamp" => Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)),
        "$randomInt" => Some(rand::random_range(0..=1000u32).to_string()),
        _ => None,
    }
}

fn resolve_inner(input: &str, scope: &VarScope, unresolved: &mut BTreeSet<String>, stack: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(input.len());
    let mut pos = 0;
    while let Some((start, end, name)) = find_token(input, pos) {
        out.push_str(&input[pos..start]);
        pos = end;
        let literal = &input[start..end];
        // A user-defined variable shadows a dynamic one of the same name.
        let Some(value) = scope.get(name).or_else(|| dynamic(name)) else {
            unresolved.insert(name.to_string());
            out.push_str(literal);
            continue;
        };
        if stack.iter().any(|n| n == name) || stack.len() >= MAX_DEPTH {
            // Cycle or too deep: leave the reference untouched.
            unresolved.insert(name.to_string());
            out.push_str(literal);
        } else if has_vars(&value) {
            stack.push(name.to_string());
            out.push_str(&resolve_inner(&value, scope, unresolved, stack));
            stack.pop();
        } else {
            out.push_str(&value);
        }
    }
    out.push_str(&input[pos..]);
    out
}

/// Replaces every `{{name}}` in `input`. Unknown names stay literal and are added to
/// `unresolved`. Values containing further references are resolved recursively (depth
/// limit 10); cyclic references stay literal and are reported as unresolved.
pub fn resolve(input: &str, scope: &VarScope, unresolved: &mut BTreeSet<String>) -> String {
    if !has_vars(input) {
        return input.to_string();
    }
    resolve_inner(input, scope, unresolved, &mut Vec::new())
}

/// Resolves variables in every user-facing field of a request: URL, params, headers,
/// auth fields and body (including multipart file paths and binary path). Scripts, name
/// and other metadata are untouched. Returns the resolved copy and the names that could
/// not be resolved. Names that only occur in disabled rows are not reported.
pub fn resolve_request(req: &Request, scope: &VarScope) -> (Request, BTreeSet<String>) {
    let mut unresolved = BTreeSet::new();
    let mut out = req.clone();

    out.url = resolve(&req.url, scope, &mut unresolved);

    let kv = |items: &[KeyValue], unresolved: &mut BTreeSet<String>| -> Vec<KeyValue> {
        items
            .iter()
            .map(|item| {
                let mut ignored = BTreeSet::new();
                let sink = if item.enabled { &mut *unresolved } else { &mut ignored };
                KeyValue {
                    key: resolve(&item.key, scope, sink),
                    value: resolve(&item.value, scope, sink),
                    enabled: item.enabled,
                }
            })
            .collect()
    };

    out.params = kv(&req.params, &mut unresolved);
    out.headers = kv(&req.headers, &mut unresolved);

    out.auth = match &req.auth {
        Auth::Bearer { token } => Auth::Bearer { token: resolve(token, scope, &mut unresolved) },
        Auth::Basic { username, password } => Auth::Basic {
            username: resolve(username, scope, &mut unresolved),
            password: resolve(password, scope, &mut unresolved),
        },
        Auth::ApiKey { key, value, location } => Auth::ApiKey {
            key: resolve(key, scope, &mut unresolved),
            value: resolve(value, scope, &mut unresolved),
            location: location.clone(),
        },
        other => other.clone(),
    };

    out.body = match &req.body {
        Body::None => Body::None,
        Body::Json { text } => Body::Json { text: resolve(text, scope, &mut unresolved) },
        Body::Raw { text, content_type } => {
            Body::Raw { text: resolve(text, scope, &mut unresolved), content_type: content_type.clone() }
        }
        Body::UrlEncoded { fields } => Body::UrlEncoded { fields: kv(fields, &mut unresolved) },
        Body::Multipart { fields } => Body::Multipart {
            fields: fields
                .iter()
                .map(|f| {
                    let mut ignored = BTreeSet::new();
                    let sink = if f.enabled { &mut unresolved } else { &mut ignored };
                    FormField {
                        key: resolve(&f.key, scope, sink),
                        value: resolve(&f.value, scope, sink),
                        enabled: f.enabled,
                        kind: f.kind.clone(),
                    }
                })
                .collect(),
        },
        Body::Binary { path } => Body::Binary { path: resolve(path, scope, &mut unresolved) },
    };

    (out, unresolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ApiKeyLocation, FormFieldKind, Scripts};

    fn var(key: &str, value: &str, enabled: bool) -> Variable {
        Variable { key: key.into(), value: value.into(), enabled, secret: false }
    }

    fn scope_of(pairs: &[(&str, &str)]) -> VarScope {
        VarScope {
            env: pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect(),
            ..Default::default()
        }
    }

    fn res(input: &str, scope: &VarScope) -> (String, BTreeSet<String>) {
        let mut u = BTreeSet::new();
        let s = resolve(input, scope, &mut u);
        (s, u)
    }

    #[test]
    fn priority_runtime_over_env_over_globals() {
        let env = Environment {
            variables: vec![var("a", "env", true), var("b", "env", true)],
            ..Default::default()
        };
        let globals = vec![var("a", "global", true), var("b", "global", true), var("c", "global", true)];
        let scope = VarScope::from(Some(&env), &globals).with_runtime([("a".to_string(), "runtime".to_string())]);
        assert_eq!(res("{{a}} {{b}} {{c}}", &scope).0, "runtime env global");
        assert_eq!(scope.get("a").as_deref(), Some("runtime"));
        assert_eq!(scope.get("zzz"), None);
    }

    #[test]
    fn disabled_variables_are_ignored() {
        let env = Environment { variables: vec![var("a", "env", false)], ..Default::default() };
        let globals = vec![var("a", "global", true), var("g", "x", false)];
        let scope = VarScope::from(Some(&env), &globals);
        assert_eq!(res("{{a}}", &scope).0, "global");
        let (s, u) = res("{{g}}", &scope);
        assert_eq!(s, "{{g}}");
        assert!(u.contains("g"));
    }

    #[test]
    fn no_environment_uses_globals() {
        let scope = VarScope::from(None, &[var("a", "1", true)]);
        assert_eq!(res("{{a}}", &scope).0, "1");
    }

    #[test]
    fn unresolved_kept_literal_and_reported() {
        let scope = scope_of(&[("a", "1")]);
        let (s, u) = res("x={{a}} y={{missing}} z={{ also }}", &scope);
        assert_eq!(s, "x=1 y={{missing}} z={{ also }}");
        assert_eq!(u.into_iter().collect::<Vec<_>>(), vec!["also", "missing"]);
    }

    #[test]
    fn whitespace_inside_braces_tolerated() {
        let scope = scope_of(&[("token", "abc")]);
        assert_eq!(res("{{ token }}|{{token }}|{{  token}}", &scope).0, "abc|abc|abc");
    }

    #[test]
    fn names_may_contain_odd_characters() {
        let scope = scope_of(&[("my var.v-2", "ok"), ("ключ", "да")]);
        assert_eq!(res("{{my var.v-2}} {{ключ}}", &scope).0, "ok да");
    }

    #[test]
    fn malformed_tokens_stay_untouched() {
        let scope = scope_of(&[("a", "1")]);
        for s in ["{{a", "a}}", "{{}}", "{{ }}", "{ {a} }", "{{a\n}}", "{{a{b}}", "plain"] {
            let (out, u) = res(s, &scope);
            assert_eq!(out, s, "input {s:?}");
            assert!(u.is_empty(), "input {s:?}: {u:?}");
        }
        // Extra braces around a valid token.
        assert_eq!(res("{{{a}}}", &scope).0, "{1}");
        assert_eq!(res("{{a}}}", &scope).0, "1}");
    }

    #[test]
    fn multibyte_text_survives() {
        let scope = scope_of(&[("a", "é")]);
        assert_eq!(res("日本{{a}}語{{x}}ü{{", &scope).0, "日本é語{{x}}ü{{");
    }

    #[test]
    fn nested_resolution() {
        let scope = scope_of(&[("host", "example.com"), ("baseUrl", "https://{{host}}/api")]);
        let (s, u) = res("{{baseUrl}}/users", &scope);
        assert_eq!(s, "https://example.com/api/users");
        assert!(u.is_empty());
    }

    #[test]
    fn nested_unresolved_reported() {
        let scope = scope_of(&[("baseUrl", "https://{{host}}/api")]);
        let (s, u) = res("{{baseUrl}}", &scope);
        assert_eq!(s, "https://{{host}}/api");
        assert!(u.contains("host"));
    }

    #[test]
    fn cycle_stops_and_stays_literal() {
        let scope = scope_of(&[("a", "{{b}}"), ("b", "{{a}}")]);
        let (s, u) = res("{{a}}", &scope);
        assert_eq!(s, "{{a}}");
        assert!(u.contains("a"));
        let scope = scope_of(&[("self", "x{{self}}y")]);
        let (s, u) = res("{{self}}", &scope);
        assert_eq!(s, "x{{self}}y");
        assert!(u.contains("self"));
    }

    #[test]
    fn repeated_non_cyclic_reference_is_fine() {
        let scope = scope_of(&[("a", "{{b}}-{{b}}"), ("b", "x")]);
        let (s, u) = res("{{a}}", &scope);
        assert_eq!(s, "x-x");
        assert!(u.is_empty());
    }

    #[test]
    fn depth_limit_is_ten() {
        // v0 -> v1 -> ... -> v11 -> "end": chain longer than the limit stays partially literal.
        let mut pairs: Vec<(String, String)> = (0..12).map(|i| (format!("v{i}"), format!("{{{{v{}}}}}", i + 1))).collect();
        pairs.push(("v12".into(), "end".into()));
        let scope = VarScope { env: pairs.into_iter().collect(), ..Default::default() };
        let (s, u) = res("{{v0}}", &scope);
        assert!(s.contains("{{"), "{s}");
        assert!(!u.is_empty());

        // A chain of exactly 10 nested levels still resolves.
        let mut pairs: Vec<(String, String)> = (0..9).map(|i| (format!("v{i}"), format!("{{{{v{}}}}}", i + 1))).collect();
        pairs.push(("v9".into(), "end".into()));
        let scope = VarScope { env: pairs.into_iter().collect(), ..Default::default() };
        assert_eq!(res("{{v0}}", &scope).0, "end");
    }

    #[test]
    fn dynamic_uuid_timestamp_iso_random() {
        let scope = VarScope::default();
        let (s, u) = res("{{$uuid}}", &scope);
        assert_eq!(s.len(), 36);
        assert!(u.is_empty());
        assert_ne!(res("{{$uuid}}", &scope).0, s);

        let (s, _) = res("{{$timestamp}}", &scope);
        assert!(s.parse::<i64>().unwrap() > 1_600_000_000);

        let (s, _) = res("{{$isoTimestamp}}", &scope);
        assert!(chrono::DateTime::parse_from_rfc3339(&s).is_ok(), "{s}");
        assert!(s.ends_with('Z') && s.len() == 24, "{s}");

        for _ in 0..50 {
            let n: u32 = res("{{ $randomInt }}", &scope).0.parse().unwrap();
            assert!(n <= 1000);
        }
    }

    #[test]
    fn user_variable_shadows_dynamic() {
        let scope = scope_of(&[("$uuid", "mine")]);
        assert_eq!(res("{{$uuid}}", &scope).0, "mine");
    }

    #[test]
    fn unknown_dynamic_is_unresolved() {
        let (s, u) = res("{{$nope}}", &VarScope::default());
        assert_eq!(s, "{{$nope}}");
        assert!(u.contains("$nope"));
    }

    #[test]
    fn has_vars_helper() {
        assert!(has_vars("a{{b}}c"));
        assert!(has_vars("{{ b }}"));
        assert!(!has_vars("{{}}"));
        assert!(!has_vars("{ {b} }"));
        assert!(!has_vars("plain"));
    }

    #[test]
    fn resolve_request_covers_all_fields() {
        let scope = scope_of(&[("h", "host"), ("k", "key"), ("v", "val"), ("p", "/tmp/f")]);
        let req = Request {
            name: "{{h}} name".into(),
            url: "https://{{h}}/x?a={{v}}".into(),
            params: vec![KeyValue::new("{{k}}", "{{v}}"), KeyValue { key: "off{{k}}".into(), value: "{{v}}".into(), enabled: false }],
            headers: vec![KeyValue::new("X-{{k}}", "{{v}}")],
            auth: Auth::Basic { username: "{{k}}".into(), password: "{{v}}".into() },
            body: Body::Multipart {
                fields: vec![
                    FormField { key: "{{k}}".into(), value: "{{v}}".into(), enabled: true, kind: FormFieldKind::Text },
                    FormField { key: "f".into(), value: "{{p}}".into(), enabled: true, kind: FormFieldKind::File },
                ],
            },
            scripts: Scripts { pre: "{{h}}".into(), post: "{{h}}".into() },
            ..Default::default()
        };
        let (r, u) = resolve_request(&req, &scope);
        assert!(u.is_empty(), "{u:?}");
        assert_eq!(r.name, "{{h}} name");
        assert_eq!(r.scripts.pre, "{{h}}");
        assert_eq!(r.url, "https://host/x?a=val");
        assert_eq!(r.params[0], KeyValue::new("key", "val"));
        assert_eq!(r.params[1].key, "offkey");
        assert!(!r.params[1].enabled);
        assert_eq!(r.headers[0], KeyValue::new("X-key", "val"));
        assert_eq!(r.auth, Auth::Basic { username: "key".into(), password: "val".into() });
        let Body::Multipart { fields } = &r.body else { panic!() };
        assert_eq!(fields[0].key, "key");
        assert_eq!(fields[0].value, "val");
        assert_eq!(fields[1].value, "/tmp/f");
        assert_eq!(fields[1].kind, FormFieldKind::File);
    }

    #[test]
    fn resolve_request_auth_and_bodies() {
        let scope = scope_of(&[("t", "tok"), ("n", "1")]);
        let mut req = Request { auth: Auth::Bearer { token: "{{t}}".into() }, ..Default::default() };
        assert_eq!(resolve_request(&req, &scope).0.auth, Auth::Bearer { token: "tok".into() });

        req.auth = Auth::ApiKey { key: "{{t}}".into(), value: "{{n}}".into(), location: ApiKeyLocation::Query };
        assert_eq!(
            resolve_request(&req, &scope).0.auth,
            Auth::ApiKey { key: "tok".into(), value: "1".into(), location: ApiKeyLocation::Query }
        );

        req.auth = Auth::Inherit;
        req.body = Body::Json { text: r#"{"a":"{{t}}","b":{{n}}}"#.into() };
        assert_eq!(resolve_request(&req, &scope).0.body, Body::Json { text: r#"{"a":"tok","b":1}"#.into() });

        req.body = Body::Raw { text: "{{t}}".into(), content_type: "text/plain".into() };
        assert_eq!(resolve_request(&req, &scope).0.body, Body::Raw { text: "tok".into(), content_type: "text/plain".into() });

        req.body = Body::UrlEncoded { fields: vec![KeyValue::new("{{t}}", "{{n}}")] };
        assert_eq!(resolve_request(&req, &scope).0.body, Body::UrlEncoded { fields: vec![KeyValue::new("tok", "1")] });

        req.body = Body::Binary { path: "/data/{{t}}.bin".into() };
        assert_eq!(resolve_request(&req, &scope).0.body, Body::Binary { path: "/data/tok.bin".into() });
    }

    #[test]
    fn resolve_request_reports_unresolved_from_enabled_only() {
        let req = Request {
            url: "{{host}}/x".into(),
            headers: vec![
                KeyValue::new("A", "{{h1}}"),
                KeyValue { key: "B".into(), value: "{{h2}}".into(), enabled: false },
            ],
            body: Body::Json { text: "{{b}}".into() },
            ..Default::default()
        };
        let (r, u) = resolve_request(&req, &VarScope::default());
        assert_eq!(r.url, "{{host}}/x");
        assert_eq!(u.into_iter().collect::<Vec<_>>(), vec!["b", "h1", "host"]);
    }
}
