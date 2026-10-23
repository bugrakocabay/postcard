//! Turns a curl command line into a [`Request`].

use super::{CurlError, tokenize};
use crate::model::{Auth, Body, FormField, FormFieldKind, KeyValue, Request};
use serde::Serialize;

/// Result of importing a curl command.
#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ParsedCurl {
    pub request: Request,
    /// Things that were ignored or could not be represented (shown in the console).
    pub warnings: Vec<String>,
}

/// Parses a pasted curl command (bash or cmd style) into a request.
///
/// Auth is `Auth::None` unless `-u` is given (an `Authorization` header stays a header).
pub fn parse_curl(input: &str) -> Result<ParsedCurl, CurlError> {
    let input = input.trim();
    if !input.starts_with("curl") {
        return Err(CurlError::NotCurl);
    }
    let tokens = tokenize(input)?;
    match tokens.first().map(String::as_str) {
        Some("curl" | "curl.exe") => {}
        _ => return Err(CurlError::NotCurl),
    }
    let mut state = State::default();
    state.read_args(&tokens[1..])?;
    state.finish()
}

/// Maps a short flag to its long name. `None` means unknown.
fn short_to_long(c: char) -> Option<&'static str> {
    Some(match c {
        'X' => "request",
        'H' => "header",
        'd' => "data",
        'F' => "form",
        'u' => "user",
        'b' => "cookie",
        'A' => "user-agent",
        'e' => "referer",
        'm' => "max-time",
        'k' => "insecure",
        'L' => "location",
        'G' => "get",
        'I' => "head",
        's' => "silent",
        'S' => "show-error",
        'v' => "verbose",
        'i' => "include",
        'f' => "fail",
        'g' => "globoff",
        'N' => "no-buffer",
        'o' => "output",
        'O' => "remote-name",
        'w' => "write-out",
        'x' => "proxy",
        'U' => "proxy-user",
        'E' => "cert",
        'c' => "cookie-jar",
        'D' => "dump-header",
        'K' => "config",
        'r' => "range",
        'T' => "upload-file",
        'C' => "continue-at",
        'y' => "speed-time",
        'Y' => "speed-limit",
        'z' => "time-cond",
        't' => "telnet-option",
        'P' => "ftp-port",
        'Q' => "quote",
        '0' => "http1.0",
        '4' => "ipv4",
        '6' => "ipv6",
        'J' => "remote-header-name",
        'R' => "remote-time",
        'q' => "disable",
        'n' => "netrc",
        'Z' => "parallel",
        '#' => "progress-bar",
        _ => return None,
    })
}

/// Long options that consume the next argument (handled or ignored).
const VALUE_FLAGS: &[&str] = &[
    "request",
    "header",
    "data",
    "data-raw",
    "data-binary",
    "data-ascii",
    "data-urlencode",
    "json",
    "form",
    "form-string",
    "user",
    "cookie",
    "user-agent",
    "referer",
    "max-time",
    "url",
    "output",
    "write-out",
    "proxy",
    "proxy-user",
    "cert",
    "cookie-jar",
    "dump-header",
    "config",
    "range",
    "upload-file",
    "continue-at",
    "speed-time",
    "speed-limit",
    "time-cond",
    "telnet-option",
    "ftp-port",
    "quote",
    "connect-timeout",
    "retry",
    "retry-delay",
    "retry-max-time",
    "max-redirs",
    "key",
    "cacert",
    "capath",
    "cert-type",
    "key-type",
    "pass",
    "resolve",
    "connect-to",
    "interface",
    "limit-rate",
    "local-port",
    "dns-servers",
    "max-filesize",
    "expect100-timeout",
    "keepalive-time",
    "aws-sigv4",
    "oauth2-bearer",
    "pinnedpubkey",
    "proxy-header",
    "proxy-cacert",
    "proxy-cert",
    "proxy-key",
    "trace",
    "trace-ascii",
    "stderr",
    "unix-socket",
    "abstract-unix-socket",
    "tls-max",
    "ciphers",
    "tls13-ciphers",
    "curves",
    "request-target",
    "noproxy",
    "proto",
    "proto-redir",
    "proto-default",
    "socks5",
    "socks5-hostname",
    "socks4",
    "socks4a",
    "netrc-file",
    "output-dir",
    "variable",
    "parallel-max",
    "rate",
    "mail-from",
    "mail-rcpt",
    "login-options",
    "service-name",
    "delegation",
    "hostpubmd5",
    "happy-eyeballs-timeout-ms",
    "etag-save",
    "etag-compare",
    "doh-url",
    "ech",
    "ca-native",
    "engine",
    "egd-file",
    "random-file",
    "krb",
    "preproxy",
    "proxy-service-name",
    "proxy-ciphers",
    "sasl-authzid",
    "time-cond",
    "write-out",
    "ftp-account",
    "ftp-alternative-to-user",
    "ftp-method",
    "ftp-ssl-ccc-mode",
    "continue-at",
    "alt-svc",
    "hsts",
    "url-query",
    "expand-data",
    "expand-url",
    "expand-header",
    "expand-form",
];

/// Flags that take no argument and carry nothing we need.
const SILENT_FLAGS: &[&str] = &[
    "silent",
    "show-error",
    "verbose",
    "include",
    "compressed",
    "http1.1",
    "http2",
    "http1.0",
    "http2-prior-knowledge",
    "fail",
    "globoff",
    "no-buffer",
    "location",
    "ipv4",
    "ipv6",
    "progress-bar",
    "no-progress-meter",
    "tcp-nodelay",
    "tcp-fastopen",
    "path-as-is",
    "fail-with-body",
    "no-keepalive",
    "tr-encoding",
    "disable",
    "remote-name",
    "remote-header-name",
    "remote-time",
    "ssl-no-revoke",
    "location-trusted",
    "anyauth",
    "basic",
    "digest",
    "ntlm",
    "negotiate",
    "netrc",
    "parallel",
    "compressed-ssh",
    "raw",
    "styled-output",
    "no-styled-output",
    "ssl",
    "ssl-reqd",
    "tlsv1",
    "tlsv1.0",
    "tlsv1.1",
    "tlsv1.2",
    "tlsv1.3",
    "sslv2",
    "sslv3",
    "no-sessionid",
    "junk-session-cookies",
    "fail-early",
    "retry-connrefused",
    "retry-all-errors",
    "proxy-anyauth",
    "proxytunnel",
    "suppress-connect-headers",
];

enum DataPart {
    /// Literal text (already without `@file` handling).
    Text(String),
    /// `--data-urlencode name=content`: the content gets percent-encoded when joined.
    UrlEncode { name: Option<String>, content: String },
    /// `-d @path`.
    File(String),
}

#[derive(Default)]
struct State {
    method: Option<String>,
    urls: Vec<String>,
    headers: Vec<(String, String)>,
    data: Vec<DataPart>,
    forms: Vec<FormField>,
    user: Option<String>,
    cookie: Option<String>,
    user_agent: Option<String>,
    referer: Option<String>,
    insecure: bool,
    get: bool,
    head: bool,
    json_flag: bool,
    max_time_ms: Option<u64>,
    warnings: Vec<String>,
}

impl State {
    fn read_args(&mut self, args: &[String]) -> Result<(), CurlError> {
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            i += 1;
            if let Some(long) = arg.strip_prefix("--").filter(|l| !l.is_empty()) {
                let (name, inline) = match long.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_string())),
                    None => (long, None),
                };
                if VALUE_FLAGS.contains(&name) {
                    let value = match inline {
                        Some(v) => v,
                        None => {
                            i += 1;
                            args.get(i - 1).cloned().ok_or_else(|| CurlError::MissingArgument(arg.clone()))?
                        }
                    };
                    self.apply(&format!("--{name}"), name, Some(value));
                } else {
                    self.apply(&format!("--{name}"), name, None);
                }
            } else if arg.len() > 1 && arg.starts_with('-') && !arg.starts_with("--") {
                let rest = &arg[1..];
                for (idx, c) in rest.char_indices() {
                    let display = format!("-{c}");
                    match short_to_long(c) {
                        Some(long) if VALUE_FLAGS.contains(&long) => {
                            let attached = &rest[idx + c.len_utf8()..];
                            let value = if attached.is_empty() {
                                i += 1;
                                args.get(i - 1).cloned().ok_or_else(|| CurlError::MissingArgument(display.clone()))?
                            } else {
                                attached.to_string()
                            };
                            self.apply(&display, long, Some(value));
                            break;
                        }
                        Some(long) => self.apply(&display, long, None),
                        None => self.warnings.push(format!("unknown flag {display} ignored")),
                    }
                }
            } else {
                self.urls.push(arg.clone());
            }
        }
        Ok(())
    }

    fn apply(&mut self, flag: &str, name: &str, value: Option<String>) {
        let v = value.clone().unwrap_or_default();
        match name {
            "request" => self.method = Some(v.trim().to_uppercase()),
            "header" => self.add_header(&v),
            "data" | "data-ascii" | "data-binary" => self.add_data(v, true),
            "data-raw" => self.add_data(v, false),
            "json" => {
                self.json_flag = true;
                self.add_data(v, false);
            }
            "data-urlencode" => self.add_data_urlencode(&v),
            "form" => self.add_form(&v, false),
            "form-string" => self.add_form(&v, true),
            "user" => self.user = Some(v),
            "cookie" => {
                if v.contains('=') {
                    self.cookie = Some(v);
                } else {
                    self.warnings.push(format!("cookie file ignored: {flag} {v}"));
                }
            }
            "user-agent" => self.user_agent = Some(v),
            "referer" => self.referer = Some(v),
            "max-time" => match v.trim().parse::<f64>() {
                Ok(secs) if secs.is_finite() && secs >= 0.0 => self.max_time_ms = Some((secs * 1000.0).round() as u64),
                _ => self.warnings.push(format!("invalid {flag} value: {v}")),
            },
            "url" => self.urls.push(v),
            "insecure" => self.insecure = true,
            "get" => self.get = true,
            "head" => self.head = true,
            "location" => {}
            _ if SILENT_FLAGS.contains(&name) => {}
            _ => match value {
                Some(v) => self.warnings.push(format!("ignored option {flag} {v}")),
                None => self.warnings.push(format!("unknown flag {flag} ignored")),
            },
        }
    }

    fn add_header(&mut self, raw: &str) {
        if let Some((k, v)) = raw.split_once(':') {
            let (k, v) = (k.trim(), v.trim());
            // `Name:` with nothing after it removes a header in curl; there is nothing to remove here.
            if !k.is_empty() && !v.is_empty() {
                self.headers.push((k.to_string(), v.to_string()));
            }
        } else if let Some(k) = raw.trim().strip_suffix(';') {
            // `Name;` sends an empty header.
            self.headers.push((k.trim().to_string(), String::new()));
        } else {
            self.warnings.push(format!("ignored malformed header: {raw}"));
        }
    }

    fn add_data(&mut self, v: String, at_file: bool) {
        match v.strip_prefix('@').filter(|_| at_file) {
            Some(path) => self.data.push(DataPart::File(path.to_string())),
            None => self.data.push(DataPart::Text(v)),
        }
    }

    fn add_data_urlencode(&mut self, v: &str) {
        if let Some((name, content)) = v.split_once('=') {
            let name = (!name.is_empty()).then(|| name.to_string());
            self.data.push(DataPart::UrlEncode { name, content: content.to_string() });
        } else if v.contains('@') {
            self.warnings.push(format!("file data not inlined: {v}"));
        } else {
            self.data.push(DataPart::UrlEncode { name: None, content: v.to_string() });
        }
    }

    fn add_form(&mut self, raw: &str, literal: bool) {
        let Some((key, value)) = raw.split_once('=') else {
            self.warnings.push(format!("ignored malformed form field: {raw}"));
            return;
        };
        let value = strip_form_params(value);
        let field =
            |value: &str, kind| FormField { key: key.to_string(), value: value.to_string(), enabled: true, kind };
        if !literal && let Some(path) = value.strip_prefix('@') {
            self.forms.push(field(unquote(path), FormFieldKind::File));
        } else if !literal && value.starts_with('<') {
            self.warnings.push(format!("file data not inlined: {key}={value}"));
        } else {
            self.forms.push(field(value, FormFieldKind::Text));
        }
    }

    fn finish(mut self) -> Result<ParsedCurl, CurlError> {
        let mut url = self.pick_url()?;

        // Content type as sent (before any header is moved into the body).
        let content_type =
            self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("content-type")).map(|(_, v)| v.clone());
        let is_form_ct =
            content_type.as_deref().is_some_and(|c| c.to_ascii_lowercase().contains("x-www-form-urlencoded"));

        let data = std::mem::take(&mut self.data);
        let forms = std::mem::take(&mut self.forms);
        let has_data = !data.is_empty();
        let mut body = Body::None;

        if !forms.is_empty() {
            if has_data {
                self.warnings.push("both -F and -d given; ignoring -d".into());
            }
            body = Body::Multipart { fields: forms };
        } else if has_data && self.get {
            let (text, _) = self.join_data(data);
            url = append_query(&url, &text);
        } else if has_data {
            let single_file = match data.as_slice() {
                [DataPart::File(path)] if !is_form_ct => Some(path.clone()),
                _ => None,
            };
            if let Some(path) = single_file {
                if path == "-" {
                    self.warnings.push("file data not inlined: @- (stdin)".into());
                } else {
                    body = Body::Binary { path };
                }
            } else {
                let (text, all_urlencoded) = self.join_data(data);
                if !text.is_empty() {
                    body = infer_body(&text, content_type.as_deref(), all_urlencoded);
                }
            }
        }

        if let Body::Raw { content_type: ct, .. } = &body
            && !ct.is_empty()
        {
            self.headers.retain(|(k, _)| !k.eq_ignore_ascii_case("content-type"));
        }
        if self.json_flag {
            self.add_header_if_missing("Content-Type", "application/json");
            self.add_header_if_missing("Accept", "application/json");
        }
        if let Some(ua) = self.user_agent.take() {
            self.add_header_if_missing("User-Agent", &ua);
        }
        if let Some(r) = self.referer.take() {
            self.add_header_if_missing("Referer", &r);
        }
        if let Some(cookie) = self.cookie.take() {
            match self.headers.iter_mut().find(|(k, _)| k.eq_ignore_ascii_case("cookie")) {
                Some((_, existing)) => *existing = format!("{existing}; {cookie}"),
                None => self.headers.push(("Cookie".into(), cookie)),
            }
        }

        let method = match (&self.method, self.head, has_data || matches!(body, Body::Multipart { .. }), self.get) {
            (Some(m), ..) => m.clone(),
            (None, true, ..) => "HEAD".into(),
            (None, false, true, false) => "POST".into(),
            _ => "GET".into(),
        };
        let auth = match self.user.take() {
            Some(u) => match u.split_once(':') {
                Some((user, pass)) => Auth::Basic { username: user.into(), password: pass.into() },
                None => Auth::Basic { username: u, password: String::new() },
            },
            None => Auth::None,
        };
        let mut request = Request {
            name: format!("{method} {}", path_of(&url)),
            method,
            params: parse_query(&url),
            url,
            headers: self.headers.into_iter().map(|(k, v)| KeyValue::new(k, v)).collect(),
            auth,
            body,
            ..Default::default()
        };
        request.settings.verify_ssl = !self.insecure;
        if let Some(ms) = self.max_time_ms {
            request.settings.timeout_ms = ms;
        }
        Ok(ParsedCurl { request, warnings: self.warnings })
    }

    fn add_header_if_missing(&mut self, name: &str, value: &str) {
        if !self.headers.iter().any(|(k, _)| k.eq_ignore_ascii_case(name)) {
            self.headers.push((name.into(), value.into()));
        }
    }

    /// First argument that looks like a URL (has `://`), else the first argument.
    fn pick_url(&mut self) -> Result<String, CurlError> {
        let idx = self.urls.iter().position(|u| u.contains("://")).unwrap_or(0);
        if self.urls.is_empty() {
            return Err(CurlError::MissingUrl);
        }
        let url = self.urls.remove(idx);
        if !self.urls.is_empty() {
            self.warnings.push(format!("multiple URLs given; using {url}"));
        }
        Ok(url)
    }

    /// Joins data parts with `&` like curl. File parts are dropped with a warning.
    /// The flag is true when every part came from `--data-urlencode`.
    fn join_data(&mut self, parts: Vec<DataPart>) -> (String, bool) {
        let all_urlencoded = parts.iter().all(|p| matches!(p, DataPart::UrlEncode { .. }));
        let mut pieces = Vec::new();
        for part in parts {
            match part {
                DataPart::Text(t) => pieces.push(t),
                DataPart::UrlEncode { name: Some(n), content } => {
                    pieces.push(format!("{n}={}", percent_encode(&content)))
                }
                DataPart::UrlEncode { name: None, content } => pieces.push(percent_encode(&content)),
                DataPart::File(path) => self.warnings.push(format!("file data not inlined: @{path}")),
            }
        }
        (pieces.join("&"), all_urlencoded)
    }
}

/// Picks the body type for `-d` style text.
fn infer_body(text: &str, content_type: Option<&str>, force_form: bool) -> Body {
    if let Some(ct) = content_type {
        let lower = ct.to_ascii_lowercase();
        return if lower.contains("x-www-form-urlencoded") {
            Body::UrlEncoded { fields: parse_form(text) }
        } else if lower.contains("json") {
            Body::Json { text: text.to_string() }
        } else {
            Body::Raw { text: text.to_string(), content_type: ct.trim().to_string() }
        };
    }
    if force_form || (!looks_like_json(text) && looks_like_form(text)) {
        Body::UrlEncoded { fields: parse_form(text) }
    } else if looks_like_json(text) {
        Body::Json { text: text.to_string() }
    } else {
        Body::Raw { text: text.to_string(), content_type: String::new() }
    }
}

fn looks_like_json(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with('{') || t.starts_with('[')) && serde_json::from_str::<serde_json::Value>(t).is_ok()
}

fn looks_like_form(text: &str) -> bool {
    !text.contains(['\n', '\r'])
        && text.split('&').all(
            |piece| matches!(piece.split_once('='), Some((k, _)) if !k.is_empty() && !k.contains(char::is_whitespace)),
        )
}

/// Splits `a=b&c=d` into decoded key/value rows.
fn parse_form(text: &str) -> Vec<KeyValue> {
    text.split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => KeyValue::new(percent_decode(k), percent_decode(v)),
            None => KeyValue::new(percent_decode(p), ""),
        })
        .collect()
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() && bytes[i + 1].is_ascii_hexdigit() && bytes[i + 2].is_ascii_hexdigit() => {
                let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).unwrap_or("00");
                out.push(u8::from_str_radix(hex, 16).unwrap_or(0));
                i += 3;
            }
            b'+' => {
                out.push(b' ');
                i += 1;
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Percent-encodes everything except unreserved characters (RFC 3986). `{{var}}` placeholders stay as they are.
pub(crate) fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while !rest.is_empty() {
        if rest.starts_with("{{")
            && let Some(end) = rest.find("}}")
        {
            out.push_str(&rest[..end + 2]);
            rest = &rest[end + 2..];
            continue;
        }
        let Some(c) = rest.chars().next() else { break };
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '~') {
            out.push(c);
        } else {
            let mut buf = [0u8; 4];
            for b in c.encode_utf8(&mut buf).bytes() {
                out.push_str(&format!("%{b:02X}"));
            }
        }
        rest = &rest[c.len_utf8()..];
    }
    out
}

/// Appends `extra` (`a=b&c=d`) to the URL's query, keeping a `#fragment`.
pub(crate) fn append_query(url: &str, extra: &str) -> String {
    if extra.is_empty() {
        return url.to_string();
    }
    let (base, hash) = match url.find('#') {
        Some(h) => url.split_at(h),
        None => (url, ""),
    };
    let sep = if !base.contains('?') {
        "?"
    } else if base.ends_with('?') || base.ends_with('&') {
        ""
    } else {
        "&"
    };
    format!("{base}{sep}{extra}{hash}")
}

/// Query rows of a URL, raw (same rules as the frontend's `parseQuery`).
fn parse_query(url: &str) -> Vec<KeyValue> {
    let no_hash = url.split('#').next().unwrap_or("");
    let Some((_, query)) = no_hash.split_once('?') else { return vec![] };
    query
        .split('&')
        .filter(|p| !p.is_empty())
        .map(|p| match p.split_once('=') {
            Some((k, v)) => KeyValue::new(k, v),
            None => KeyValue::new(p, ""),
        })
        .collect()
}

/// Path part for the request name: no scheme/host/variable prefix, no query, `/` if empty.
fn path_of(url: &str) -> String {
    let mut s = url.split(['?', '#']).next().unwrap_or("");
    if let Some((_, after)) = s.split_once("://") {
        s = after.find('/').map_or("", |i| &after[i..]);
    } else if s.starts_with("{{")
        && let Some(end) = s.find("}}")
    {
        s = &s[end + 2..];
    } else if !s.starts_with('/') {
        s = s.find('/').map_or("", |i| &s[i..]);
    }
    if s.is_empty() { "/".into() } else { s.into() }
}

/// Drops curl's `;type=...`, `;filename=...` suffixes from a `-F` value.
fn strip_form_params(value: &str) -> &str {
    let cut = [";type=", ";filename=", ";headers=", ";encoder="].iter().filter_map(|m| value.find(m)).min();
    cut.map_or(value, |i| &value[..i])
}

fn unquote(s: &str) -> &str {
    s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).unwrap_or(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{ApiKeyLocation, Auth, Body, FormField, FormFieldKind, KeyValue};

    fn p(s: &str) -> ParsedCurl {
        parse_curl(s).unwrap_or_else(|e| panic!("{e}: {s}"))
    }

    fn header<'a>(r: &'a Request, name: &str) -> Option<&'a str> {
        r.headers.iter().find(|h| h.key.eq_ignore_ascii_case(name)).map(|h| h.value.as_str())
    }

    #[test]
    fn rejects_non_curl_and_missing_url() {
        assert_eq!(parse_curl("wget http://x").unwrap_err(), CurlError::NotCurl);
        assert_eq!(parse_curl("curling http://x").unwrap_err(), CurlError::NotCurl);
        assert_eq!(parse_curl("").unwrap_err(), CurlError::NotCurl);
        assert_eq!(parse_curl("curl -X POST").unwrap_err(), CurlError::MissingUrl);
        assert_eq!(parse_curl("curl").unwrap_err(), CurlError::MissingUrl);
        assert_eq!(parse_curl("curl -H").unwrap_err(), CurlError::MissingArgument("-H".into()));
        assert_eq!(parse_curl("curl 'http://x").unwrap_err(), CurlError::UnterminatedQuote);
    }

    #[test]
    fn minimal_get() {
        let r = p("curl example.com");
        assert_eq!(r.request.method, "GET");
        assert_eq!(r.request.url, "example.com");
        assert_eq!(r.request.name, "GET /");
        assert!(r.request.headers.is_empty());
        assert_eq!(r.request.body, Body::None);
        assert_eq!(r.request.auth, Auth::None);
        assert!(r.warnings.is_empty());
        assert_eq!(p("  curl.exe https://a.b/c  ").request.url, "https://a.b/c");
    }

    #[test]
    fn chrome_bash_copy() {
        let input = r#"curl 'https://api.example.com/v1/items?x=1&y=a%20b' \
  -H 'accept: */*' \
  -H 'accept-language: en-US,en;q=0.9' \
  -H 'authorization: Bearer abc.def' \
  -H 'content-type: application/json' \
  -b 'session=abc; theme=dark' \
  -H 'user-agent: Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36' \
  --data-raw $'{"title":"it\'s","note":"line1\\nline2","bang":"wow!"}' \
  --compressed"#;
        let parsed = p(input);
        let r = &parsed.request;
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(r.method, "POST");
        assert_eq!(r.name, "POST /v1/items");
        assert_eq!(r.url, "https://api.example.com/v1/items?x=1&y=a%20b");
        assert_eq!(r.params, vec![KeyValue::new("x", "1"), KeyValue::new("y", "a%20b")]);
        assert_eq!(header(r, "authorization"), Some("Bearer abc.def"));
        assert_eq!(header(r, "cookie"), Some("session=abc; theme=dark"));
        assert_eq!(r.auth, Auth::None);
        assert_eq!(r.body, Body::Json { text: r#"{"title":"it's","note":"line1\nline2","bang":"wow!"}"#.into() });
    }

    #[test]
    fn chrome_cmd_copy() {
        let input = "curl ^\"https://api.example.com/v1/users^?page=1^&limit=10^\" ^\r\n  -H ^\"accept: application/json^\" ^\r\n  -H ^\"content-type: application/json^\" ^\r\n  --data-raw ^\"^{^\\^\"name^\\^\":^\\^\"Jane^\\^\",^\\^\"pct^\\^\":^\\^\"100^%^\\^\"^}^\" ^\r\n  --compressed";
        let parsed = p(input);
        let r = &parsed.request;
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(r.url, "https://api.example.com/v1/users?page=1&limit=10");
        assert_eq!(r.method, "POST");
        assert_eq!(r.body, Body::Json { text: r#"{"name":"Jane","pct":"100%"}"#.into() });
        assert_eq!(r.headers.len(), 2);
    }

    #[test]
    fn firefox_copy_with_quote_escapes() {
        let input = r#"curl 'https://example.com/api/echo' --compressed -X POST -H 'User-Agent: Mozilla/5.0 (X11; Linux x86_64; rv:115.0) Gecko/20100101 Firefox/115.0' -H 'Accept: */*' -H 'Content-Type: application/json' -H 'TE: trailers' --data-raw '{"msg":"it'\''s fine"}'"#;
        let parsed = p(input);
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.request.method, "POST");
        assert_eq!(parsed.request.headers.len(), 4);
        assert_eq!(parsed.request.body, Body::Json { text: r#"{"msg":"it's fine"}"#.into() });
    }

    #[test]
    fn double_quoted_with_single_quote_inside() {
        let r = p(r#"curl "http://x/a" -H "X-Name: O'Neil" -d "{\"n\":\"it's\"}""#).request;
        assert_eq!(header(&r, "x-name"), Some("O'Neil"));
        assert_eq!(r.body, Body::Json { text: r#"{"n":"it's"}"#.into() });
    }

    #[test]
    fn put_with_file_is_binary_without_warning() {
        let parsed = p("curl -X PUT -d @payload.bin https://x.test/upload");
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.request.method, "PUT");
        assert_eq!(parsed.request.body, Body::Binary { path: "payload.bin".into() });
        let parsed = p("curl --data-binary @/tmp/a.png https://x.test/up");
        assert_eq!(parsed.request.method, "POST");
        assert_eq!(parsed.request.body, Body::Binary { path: "/tmp/a.png".into() });
    }

    #[test]
    fn file_data_mixed_or_urlencoded_warns() {
        let parsed = p("curl -d a=1 -d @f.txt https://x.test");
        assert_eq!(parsed.warnings, vec!["file data not inlined: @f.txt".to_string()]);
        assert_eq!(parsed.request.body, Body::UrlEncoded { fields: vec![KeyValue::new("a", "1")] });
        let parsed = p("curl -H 'Content-Type: application/x-www-form-urlencoded' -d @f.txt https://x.test");
        assert_eq!(parsed.warnings.len(), 1);
        assert_eq!(parsed.request.body, Body::None);
        assert_eq!(parsed.request.method, "POST");
    }

    #[test]
    fn basic_auth_and_cookie() {
        let r = p("curl -u 'user:pa:ss' -b 'a=1' https://x.test").request;
        assert_eq!(r.auth, Auth::Basic { username: "user".into(), password: "pa:ss".into() });
        assert_eq!(header(&r, "cookie"), Some("a=1"));
        let r = p("curl --user bob https://x.test").request;
        assert_eq!(r.auth, Auth::Basic { username: "bob".into(), password: "".into() });
    }

    #[test]
    fn multipart_forms() {
        let parsed = p(
            "curl -F 'name=Jane Doe' -F 'avatar=@/tmp/me.png;type=image/png' -F 'note=@\"/tmp/x y.txt\"' https://x.test/up",
        );
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.request.method, "POST");
        let fields = vec![
            FormField { key: "name".into(), value: "Jane Doe".into(), enabled: true, kind: FormFieldKind::Text },
            FormField { key: "avatar".into(), value: "/tmp/me.png".into(), enabled: true, kind: FormFieldKind::File },
            FormField { key: "note".into(), value: "/tmp/x y.txt".into(), enabled: true, kind: FormFieldKind::File },
        ];
        assert_eq!(parsed.request.body, Body::Multipart { fields });
    }

    #[test]
    fn get_flag_moves_data_to_query() {
        let r = p("curl -G -d q=1 -d 'x=a b' https://x.test/search").request;
        assert_eq!(r.method, "GET");
        assert_eq!(r.body, Body::None);
        assert_eq!(r.url, "https://x.test/search?q=1&x=a b");
        assert_eq!(r.params, vec![KeyValue::new("q", "1"), KeyValue::new("x", "a b")]);
        let r = p("curl -G -d q=1 'https://x.test/s?a=2'").request;
        assert_eq!(r.url, "https://x.test/s?a=2&q=1");
        let r = p("curl -G --data-urlencode 'q=a b&c' 'https://x.test/s#frag'").request;
        assert_eq!(r.url, "https://x.test/s?q=a%20b%26c#frag");
    }

    #[test]
    fn multiple_data_flags_join_with_ampersand() {
        let r = p("curl -d a=1 -d b=2 --data c=3 https://x.test").request;
        assert_eq!(r.method, "POST");
        assert_eq!(
            r.body,
            Body::UrlEncoded {
                fields: vec![KeyValue::new("a", "1"), KeyValue::new("b", "2"), KeyValue::new("c", "3")]
            }
        );
        let r = p("curl -d 'a=1&b=hello%20w%26' https://x.test").request;
        assert_eq!(r.body, Body::UrlEncoded { fields: vec![KeyValue::new("a", "1"), KeyValue::new("b", "hello w&")] });
    }

    #[test]
    fn data_urlencode_forms() {
        let r = p("curl --data-urlencode 'a=b c&d' --data-urlencode 'e=é' https://x.test").request;
        assert_eq!(r.body, Body::UrlEncoded { fields: vec![KeyValue::new("a", "b c&d"), KeyValue::new("e", "é")] });
    }

    #[test]
    fn body_type_inference() {
        // JSON by parse, no content type.
        let r = p(r#"curl -d '{"a":1}' https://x.test"#).request;
        assert_eq!(r.body, Body::Json { text: r#"{"a":1}"#.into() });
        // Plain text without content type.
        let r = p("curl -d 'hello world' https://x.test").request;
        assert_eq!(r.body, Body::Raw { text: "hello world".into(), content_type: "".into() });
        // Text content type: kept on the body, header moved there.
        let r = p("curl -H 'Content-Type: text/plain' -d '{\"a\":1}' https://x.test").request;
        assert_eq!(r.body, Body::Raw { text: "{\"a\":1}".into(), content_type: "text/plain".into() });
        assert!(header(&r, "content-type").is_none());
        // XML.
        let r = p("curl -H 'content-type: application/xml' -d '<a/>' https://x.test").request;
        assert_eq!(r.body, Body::Raw { text: "<a/>".into(), content_type: "application/xml".into() });
        // JSON content type with unparseable text stays Json.
        let r = p("curl -H 'content-type: application/json; charset=utf-8' -d '{oops' https://x.test").request;
        assert_eq!(r.body, Body::Json { text: "{oops".into() });
        assert!(header(&r, "content-type").is_some());
        // Form content type.
        let r = p("curl -H 'Content-Type: application/x-www-form-urlencoded' -d 'a=1&b' https://x.test").request;
        assert_eq!(r.body, Body::UrlEncoded { fields: vec![KeyValue::new("a", "1"), KeyValue::new("b", "")] });
    }

    #[test]
    fn json_flag_sets_headers() {
        let r = p(r#"curl --json '{"a":1}' https://x.test"#).request;
        assert_eq!(r.method, "POST");
        assert_eq!(r.body, Body::Json { text: r#"{"a":1}"#.into() });
        assert_eq!(header(&r, "content-type"), Some("application/json"));
        assert_eq!(header(&r, "accept"), Some("application/json"));
    }

    #[test]
    fn method_inference_and_override() {
        assert_eq!(p("curl -d a=1 http://x").request.method, "POST");
        assert_eq!(p("curl -X PATCH -d a=1 http://x").request.method, "PATCH");
        assert_eq!(p("curl -X delete http://x").request.method, "DELETE");
        assert_eq!(p("curl --request=PUT http://x").request.method, "PUT");
        assert_eq!(p("curl -I http://x").request.method, "HEAD");
        assert_eq!(p("curl -X GET -d a=1 http://x").request.method, "GET");
    }

    #[test]
    fn combined_and_attached_short_flags() {
        let parsed = p("curl -sSL -XPOST -HContent-Type:text/plain -d x http://x.test/a");
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
        assert_eq!(parsed.request.method, "POST");
        assert_eq!(parsed.request.body, Body::Raw { text: "x".into(), content_type: "text/plain".into() });
        let r = p("curl -sk -m5 --header=X-A:1 --url=http://x.test").request;
        assert!(!r.settings.verify_ssl);
        assert_eq!(r.settings.timeout_ms, 5000);
        assert_eq!(header(&r, "x-a"), Some("1"));
        assert_eq!(r.url, "http://x.test");
    }

    #[test]
    fn settings_flags() {
        let r = p("curl -k -L --max-time 2.5 http://x").request;
        assert!(!r.settings.verify_ssl);
        assert!(r.settings.follow_redirects);
        assert_eq!(r.settings.timeout_ms, 2500);
        let r = p("curl http://x").request;
        assert!(r.settings.verify_ssl);
        assert_eq!(r.settings.timeout_ms, 30_000);
    }

    #[test]
    fn user_agent_and_referer_become_headers() {
        let r = p("curl -A 'my-agent/1' -e 'http://ref' http://x").request;
        assert_eq!(header(&r, "user-agent"), Some("my-agent/1"));
        assert_eq!(header(&r, "referer"), Some("http://ref"));
    }

    #[test]
    fn silent_flags_produce_no_warnings() {
        let parsed = p(
            "curl -s -S -v -i --compressed -L --http1.1 --http2 -f --fail -g --globoff -N --no-buffer --location http://x",
        );
        assert!(parsed.warnings.is_empty(), "{:?}", parsed.warnings);
    }

    #[test]
    fn unknown_flags_warn_and_value_flags_consume_their_argument() {
        let parsed = p("curl -o out.txt --connect-timeout 5 -w '%{http_code}' --weird-flag https://x.test/a");
        assert_eq!(parsed.request.url, "https://x.test/a");
        assert_eq!(parsed.warnings.len(), 4, "{:?}", parsed.warnings);
        assert!(parsed.warnings.iter().any(|w| w.contains("-o")));
        assert!(parsed.warnings.iter().any(|w| w.contains("--connect-timeout")));
        assert!(parsed.warnings.iter().any(|w| w.contains("--weird-flag")));
        // Value attached to a short flag.
        let parsed = p("curl -oout.txt https://x.test/a");
        assert_eq!(parsed.request.url, "https://x.test/a");
        assert_eq!(parsed.warnings.len(), 1);
    }

    #[test]
    fn header_edge_cases() {
        let parsed = p("curl -H 'X-Empty;' -H 'Accept:' -H 'bad header' -H 'A:  spaced  value ' http://x");
        let hs: Vec<(&str, &str)> = parsed.request.headers.iter().map(|h| (h.key.as_str(), h.value.as_str())).collect();
        assert_eq!(hs, vec![("X-Empty", ""), ("A", "spaced  value")]);
        assert_eq!(parsed.warnings.len(), 1);
    }

    #[test]
    fn name_from_url_path() {
        assert_eq!(p("curl https://h.test/api/users?x=1#f").request.name, "GET /api/users");
        assert_eq!(p("curl -X POST '{{baseUrl}}/api/users'").request.name, "POST /api/users");
        assert_eq!(p("curl https://h.test").request.name, "GET /");
        assert_eq!(p("curl 'https://h.test?x=1'").request.name, "GET /");
        assert_eq!(p("curl localhost:3000/a/b").request.name, "GET /a/b");
    }

    #[test]
    fn extra_urls_warn_and_url_flag_works() {
        let parsed = p("curl --url http://a.test http://b.test");
        assert_eq!(parsed.request.url, "http://a.test");
        assert_eq!(parsed.warnings.len(), 1);
    }

    #[test]
    fn serializes_camel_case() {
        let v = serde_json::to_value(p("curl http://x")).unwrap();
        assert!(v.get("request").is_some() && v.get("warnings").is_some());
        let _ = ApiKeyLocation::Header;
    }
}
