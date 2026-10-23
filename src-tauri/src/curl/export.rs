//! Renders a [`Request`] as a bash-style curl command.

use super::parse::{append_query, percent_encode};
use crate::model::{ApiKeyLocation, Auth, Body, FormFieldKind, Request};

/// Single-quotes `s` for bash (`'` becomes `'\''`).
fn quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

fn has_header(req: &Request, name: &str) -> bool {
    req.headers.iter().any(|h| h.enabled && h.key.eq_ignore_ascii_case(name))
}

fn header_flag(key: &str, value: &str) -> String {
    if value.is_empty() {
        format!("-H {}", quote(&format!("{key};")))
    } else {
        format!("-H {}", quote(&format!("{key}: {value}")))
    }
}

/// Exports a request as a multi-line curl command (bash quoting, `\` continuations).
///
/// Variables are written as they are; resolve them first if a runnable command is needed.
/// Scripts are not exported. Inherited auth is not exported either (the caller can
/// replace `Auth::Inherit` with the effective auth first). Field names of urlencoded
/// bodies are percent-encoded because curl sends the part before `=` as it is.
pub fn to_curl(req: &Request) -> String {
    let mut url = req.url.clone();
    let mut flags: Vec<String> = Vec::new();
    let mut body_flags: Vec<String> = Vec::new();

    let enabled_form = |fields: &[crate::model::FormField]| -> Vec<crate::model::FormField> {
        fields.iter().filter(|f| f.enabled && !(f.key.is_empty() && f.value.is_empty())).cloned().collect()
    };

    let mut extra_headers: Vec<(String, String)> = Vec::new();
    match &req.body {
        Body::None => {}
        Body::Json { text } => {
            if !text.trim().is_empty() {
                if !has_header(req, "content-type") {
                    extra_headers.push(("Content-Type".into(), "application/json".into()));
                }
                body_flags.push(format!("--data-raw {}", quote(text)));
            }
        }
        Body::Raw { text, content_type } => {
            if !text.trim().is_empty() {
                if !content_type.trim().is_empty() && !has_header(req, "content-type") {
                    extra_headers.push(("Content-Type".into(), content_type.trim().into()));
                }
                body_flags.push(format!("--data-raw {}", quote(text)));
            }
        }
        Body::UrlEncoded { fields } => {
            for f in fields.iter().filter(|f| f.enabled && !(f.key.is_empty() && f.value.is_empty())) {
                body_flags
                    .push(format!("--data-urlencode {}", quote(&format!("{}={}", percent_encode(&f.key), f.value))));
            }
        }
        Body::Multipart { fields } => {
            for f in enabled_form(fields) {
                let flag = match f.kind {
                    FormFieldKind::File => format!("-F {}", quote(&format!("{}=@{}", f.key, f.value))),
                    FormFieldKind::Text if f.value.starts_with(['@', '<']) || f.value.contains(';') => {
                        format!("--form-string {}", quote(&format!("{}={}", f.key, f.value)))
                    }
                    FormFieldKind::Text => format!("-F {}", quote(&format!("{}={}", f.key, f.value))),
                };
                body_flags.push(flag);
            }
        }
        Body::Binary { path } => {
            if !path.is_empty() {
                body_flags.push(format!("--data-binary {}", quote(&format!("@{path}"))));
            }
        }
    }
    let has_body = !body_flags.is_empty();
    let multipart = matches!(req.body, Body::Multipart { .. }) && has_body;

    for h in req.headers.iter().filter(|h| h.enabled && !h.key.trim().is_empty()) {
        // A bare multipart Content-Type has no boundary; curl must generate its own.
        if multipart
            && h.key.eq_ignore_ascii_case("content-type")
            && h.value.trim().eq_ignore_ascii_case("multipart/form-data")
        {
            continue;
        }
        flags.push(header_flag(h.key.trim(), &h.value));
    }

    match &req.auth {
        Auth::Bearer { token } if !has_header(req, "authorization") => {
            flags.push(header_flag("Authorization", &format!("Bearer {token}")));
        }
        Auth::Basic { username, password } if !has_header(req, "authorization") => {
            flags.push(format!("-u {}", quote(&format!("{username}:{password}"))));
        }
        Auth::ApiKey { key, value, location } if !key.is_empty() => match location {
            ApiKeyLocation::Header if !has_header(req, key) => flags.push(header_flag(key, value)),
            ApiKeyLocation::Header => {}
            ApiKeyLocation::Query => {
                url = append_query(&url, &format!("{}={}", percent_encode(key), percent_encode(value)))
            }
        },
        _ => {}
    }

    for (k, v) in &extra_headers {
        flags.push(header_flag(k, v));
    }
    flags.extend(body_flags);

    if !req.settings.verify_ssl {
        flags.push("-k".into());
    }
    if req.settings.follow_redirects {
        flags.push("-L".into());
    }
    if req.settings.timeout_ms != 30_000 {
        flags.push(format!("--max-time {}", req.settings.timeout_ms.div_ceil(1000)));
    }

    let method = if req.method.trim().is_empty() { "GET".to_string() } else { req.method.trim().to_uppercase() };
    let method_part = match method.as_str() {
        "GET" if !has_body => String::new(),
        "POST" if has_body => String::new(),
        "HEAD" => " -I".to_string(),
        other => format!(" -X {other}"),
    };
    let mut lines = vec![format!("curl{method_part} {}", quote(&url))];
    lines.extend(flags);
    lines.join(" \\\n  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curl::parse_curl;
    use crate::model::{ApiKeyLocation, Auth, Body, FormField, FormFieldKind, KeyValue};

    fn req(method: &str, url: &str) -> Request {
        Request { method: method.into(), url: url.into(), auth: Auth::None, ..Default::default() }
    }

    #[test]
    fn simple_get() {
        let r = req("GET", "https://x.test/a?b=1");
        assert_eq!(to_curl(&r), "curl 'https://x.test/a?b=1' \\\n  -L");
    }

    #[test]
    fn minimal_without_redirects_is_one_line() {
        let mut r = req("GET", "http://x");
        r.settings.follow_redirects = false;
        assert_eq!(to_curl(&r), "curl 'http://x'");
    }

    #[test]
    fn method_flag_only_when_needed() {
        let mut r = req("GET", "http://x");
        r.settings.follow_redirects = false;
        assert!(!to_curl(&r).contains("-X"));
        r.method = "POST".into();
        assert!(to_curl(&r).starts_with("curl -X POST 'http://x'"));
        r.body = Body::Raw { text: "a".into(), content_type: "".into() };
        assert!(!to_curl(&r).contains("-X"));
        r.method = "PUT".into();
        assert!(to_curl(&r).starts_with("curl -X PUT 'http://x'"));
        r.method = "GET".into();
        assert!(to_curl(&r).starts_with("curl -X GET 'http://x'"));
        r.body = Body::None;
        r.method = "HEAD".into();
        assert!(to_curl(&r).starts_with("curl -I 'http://x'"));
        r.method = "delete".into();
        assert!(to_curl(&r).starts_with("curl -X DELETE 'http://x'"));
    }

    #[test]
    fn headers_skip_disabled_and_escape_quotes() {
        let mut r = req("GET", "http://x");
        r.settings.follow_redirects = false;
        r.headers = vec![
            KeyValue::new("X-A", "it's"),
            KeyValue { key: "X-Off".into(), value: "1".into(), enabled: false },
            KeyValue::new("X-Empty", ""),
        ];
        assert_eq!(to_curl(&r), "curl 'http://x' \\\n  -H 'X-A: it'\\''s' \\\n  -H 'X-Empty;'");
    }

    #[test]
    fn auth_variants() {
        let mut r = req("GET", "http://x/p?a=1#h");
        r.settings.follow_redirects = false;
        r.auth = Auth::Bearer { token: "tok".into() };
        assert!(to_curl(&r).contains("-H 'Authorization: Bearer tok'"));
        r.auth = Auth::Basic { username: "u".into(), password: "p'w".into() };
        assert!(to_curl(&r).contains("-u 'u:p'\\''w'"));
        r.auth = Auth::ApiKey { key: "X-Key".into(), value: "v".into(), location: ApiKeyLocation::Header };
        assert!(to_curl(&r).contains("-H 'X-Key: v'"));
        r.auth = Auth::ApiKey { key: "api key".into(), value: "a&b{{tok}}".into(), location: ApiKeyLocation::Query };
        assert!(to_curl(&r).starts_with("curl 'http://x/p?a=1&api%20key=a%26b{{tok}}#h'"), "{}", to_curl(&r));
        r.url = "http://x/p".into();
        assert!(to_curl(&r).starts_with("curl 'http://x/p?api%20key=a%26b{{tok}}'"));
        r.auth = Auth::Inherit;
        assert_eq!(to_curl(&r), "curl 'http://x/p'");
    }

    #[test]
    fn explicit_authorization_header_is_not_duplicated() {
        let mut r = req("GET", "http://x");
        r.headers = vec![KeyValue::new("authorization", "Bearer mine")];
        r.auth = Auth::Bearer { token: "other".into() };
        let out = to_curl(&r);
        assert!(out.contains("Bearer mine") && !out.contains("other"), "{out}");
    }

    #[test]
    fn json_body_adds_content_type_once() {
        let mut r = req("POST", "http://x");
        r.body = Body::Json { text: "{\"a\":\"it's\"}".into() };
        let out = to_curl(&r);
        assert!(out.contains("-H 'Content-Type: application/json'"), "{out}");
        assert!(out.contains("--data-raw '{\"a\":\"it'\\''s\"}'"), "{out}");
        r.headers = vec![KeyValue::new("content-type", "application/vnd.api+json")];
        let out = to_curl(&r);
        assert_eq!(out.to_lowercase().matches("content-type").count(), 1, "{out}");
        assert!(!out.contains("application/json'"), "{out}");
    }

    #[test]
    fn raw_body_uses_its_content_type() {
        let mut r = req("POST", "http://x");
        r.body = Body::Raw { text: "<a/>".into(), content_type: "application/xml".into() };
        let out = to_curl(&r);
        assert!(out.contains("-H 'Content-Type: application/xml'") && out.contains("--data-raw '<a/>'"), "{out}");
    }

    #[test]
    fn empty_bodies_emit_nothing() {
        let mut r = req("POST", "http://x");
        r.settings.follow_redirects = false;
        r.body = Body::Json { text: "  ".into() };
        assert_eq!(to_curl(&r), "curl -X POST 'http://x'");
        r.body = Body::UrlEncoded { fields: vec![KeyValue { key: "a".into(), value: "1".into(), enabled: false }] };
        assert_eq!(to_curl(&r), "curl -X POST 'http://x'");
    }

    #[test]
    fn urlencoded_multipart_binary() {
        let mut r = req("POST", "http://x");
        r.settings.follow_redirects = false;
        r.body = Body::UrlEncoded { fields: vec![KeyValue::new("a", "b c"), KeyValue::new("d", "")] };
        assert_eq!(to_curl(&r), "curl 'http://x' \\\n  --data-urlencode 'a=b c' \\\n  --data-urlencode 'd='");
        r.body = Body::Multipart {
            fields: vec![
                FormField { key: "t".into(), value: "v".into(), enabled: true, kind: FormFieldKind::Text },
                FormField { key: "f".into(), value: "/tmp/a b.png".into(), enabled: true, kind: FormFieldKind::File },
                FormField { key: "off".into(), value: "x".into(), enabled: false, kind: FormFieldKind::Text },
                FormField { key: "at".into(), value: "@literal".into(), enabled: true, kind: FormFieldKind::Text },
            ],
        };
        assert_eq!(
            to_curl(&r),
            "curl 'http://x' \\\n  -F 't=v' \\\n  -F 'f=@/tmp/a b.png' \\\n  --form-string 'at=@literal'"
        );
        r.body = Body::Binary { path: "/tmp/x.bin".into() };
        assert_eq!(to_curl(&r), "curl 'http://x' \\\n  --data-binary '@/tmp/x.bin'");
    }

    #[test]
    fn multipart_content_type_header_without_boundary_is_dropped() {
        let mut r = req("POST", "http://x");
        r.settings.follow_redirects = false;
        r.headers = vec![KeyValue::new("Content-Type", "multipart/form-data")];
        r.body = Body::Multipart {
            fields: vec![FormField { key: "t".into(), value: "v".into(), enabled: true, kind: FormFieldKind::Text }],
        };
        assert!(!to_curl(&r).contains("Content-Type"));
    }

    #[test]
    fn settings_flags() {
        let mut r = req("GET", "http://x");
        r.settings.verify_ssl = false;
        r.settings.timeout_ms = 2500;
        assert_eq!(to_curl(&r), "curl 'http://x' \\\n  -k \\\n  -L \\\n  --max-time 3");
        r.settings.timeout_ms = 30_000;
        assert!(!to_curl(&r).contains("--max-time"));
        r.settings.timeout_ms = 0;
        assert!(to_curl(&r).contains("--max-time 0"));
    }

    fn roundtrip(r: &Request) -> Request {
        let out = to_curl(r);
        parse_curl(&out).unwrap_or_else(|e| panic!("{e}\n{out}")).request
    }

    fn assert_same(a: &Request, b: &Request) {
        assert_eq!(a.method, b.method);
        assert_eq!(a.url, b.url);
        assert_eq!(a.headers, b.headers);
        assert_eq!(a.body, b.body);
        assert_eq!(a.settings, b.settings);
    }

    #[test]
    fn roundtrip_json_with_quotes_and_newlines() {
        let mut r = req("PATCH", "https://api.test/v1/items?x=1&y=a%20b");
        r.headers = vec![KeyValue::new("Content-Type", "application/json"), KeyValue::new("X-Note", "it's \"fine\"")];
        r.body = Body::Json { text: "{\n  \"msg\": \"it's\\n 100%\",\n  \"u\": \"é\"\n}".into() };
        let back = roundtrip(&r);
        assert_same(&r, &back);
        assert_eq!(back.params, vec![KeyValue::new("x", "1"), KeyValue::new("y", "a%20b")]);
    }

    #[test]
    fn roundtrip_json_without_header_gains_content_type() {
        let mut r = req("POST", "http://x/a");
        r.body = Body::Json { text: "{\"a\":1}".into() };
        let back = roundtrip(&r);
        assert_eq!(back.body, r.body);
        assert_eq!(back.headers, vec![KeyValue::new("Content-Type", "application/json")]);
    }

    #[test]
    fn roundtrip_urlencoded() {
        let mut r = req("POST", "http://x/form");
        r.body = Body::UrlEncoded {
            fields: vec![KeyValue::new("a", "b c"), KeyValue::new("d&e", "f=g+h%"), KeyValue::new("u", "é{{v}}")],
        };
        assert_same(&r, &roundtrip(&r));
    }

    #[test]
    fn roundtrip_multipart() {
        let mut r = req("POST", "http://x/up");
        r.body = Body::Multipart {
            fields: vec![
                FormField { key: "name".into(), value: "Jane Doe".into(), enabled: true, kind: FormFieldKind::Text },
                FormField {
                    key: "avatar".into(),
                    value: "/tmp/me.png".into(),
                    enabled: true,
                    kind: FormFieldKind::File,
                },
                FormField { key: "lit".into(), value: "@not-a-file".into(), enabled: true, kind: FormFieldKind::Text },
            ],
        };
        assert_same(&r, &roundtrip(&r));
    }

    #[test]
    fn roundtrip_raw_binary_and_basic_auth() {
        let mut r = req("PUT", "http://x/raw");
        r.body = Body::Raw { text: "plain 'text'".into(), content_type: "text/plain".into() };
        assert_same(&r, &roundtrip(&r));
        r.body = Body::Binary { path: "/tmp/a b.bin".into() };
        assert_same(&r, &roundtrip(&r));
        r.body = Body::None;
        r.method = "DELETE".into();
        r.auth = Auth::Basic { username: "user".into(), password: "p:a'ss".into() };
        let back = roundtrip(&r);
        assert_same(&r, &back);
        assert_eq!(back.auth, r.auth);
    }

    #[test]
    fn roundtrip_settings_and_empty_header() {
        let mut r = req("GET", "http://x");
        r.settings.verify_ssl = false;
        r.settings.timeout_ms = 5000;
        r.headers = vec![KeyValue::new("X-Empty", ""), KeyValue::new("Authorization", "Bearer abc")];
        assert_same(&r, &roundtrip(&r));
    }
}
