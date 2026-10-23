//! Embedded JavaScript engine (QuickJS) for pre-request and post-response scripts.
//!
//! Each run gets a fresh runtime. The state goes in as JSON, `prelude.js` builds
//! `pm` and `console` over it, and the changed state is read back afterwards.

pub mod shell;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rquickjs::{CatchResultExt, Context, Ctx, Function, Runtime};
use serde::{Deserialize, Serialize};

use crate::model::ShellSettings;

const PRELUDE: &str = include_str!("prelude.js");
/// Time limit for one script, not counting time spent in `pm.shell`.
const SCRIPT_LIMIT: Duration = Duration::from_secs(60);
/// Time limit for one `pm.shell` call.
const SHELL_TIMEOUT: Duration = Duration::from_secs(30);
const MEMORY_LIMIT: usize = 256 * 1024 * 1024;
const STACK_LIMIT: usize = 1024 * 1024;

/// Everything a script can read or change.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptState {
    /// Active environment variables.
    pub env: BTreeMap<String, String>,
    pub globals: BTreeMap<String, String>,
    /// Runtime variables, for this send only.
    pub runtime: BTreeMap<String, String>,
    pub request: ScriptRequest,
    /// `None` in pre-request scripts.
    pub response: Option<ScriptResponse>,
}

/// The request as scripts see it. Headers keep their order; names may repeat.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
}

/// The response, available to post-response scripts (read only).
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ScriptResponse {
    pub code: u16,
    pub status: String,
    pub headers: Vec<(String, String)>,
    pub body: String,
    pub time_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    #[default]
    Log,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    fn parse(s: &str) -> Self {
        match s {
            "info" => Self::Info,
            "warn" => Self::Warn,
            "error" => Self::Error,
            _ => Self::Log,
        }
    }
}

/// One `console.*` call.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct LogLine {
    pub level: LogLevel,
    pub message: String,
}

/// A script failed. The message reads `<name>: <message>` plus ` (line N)` when known.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, thiserror::Error)]
#[serde(rename_all = "camelCase", default)]
#[error("{message}")]
pub struct ScriptError {
    pub message: String,
}

/// The parts of the state a script may change.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FinalState {
    env: BTreeMap<String, String>,
    globals: BTreeMap<String, String>,
    runtime: BTreeMap<String, String>,
    request: ScriptRequest,
}

/// Runs `source` with the default 60 s limit and 30 s shell timeout.
///
/// `name` labels errors (for example "Collection pre-request"). `state` is
/// updated only when the script finishes without error. Console output is
/// appended to `logs` either way. Empty source returns at once.
pub fn run_script(
    source: &str,
    name: &str,
    state: &mut ScriptState,
    shell: &ShellSettings,
    logs: &mut Vec<LogLine>,
) -> Result<(), ScriptError> {
    run_script_with_limit(source, name, state, shell, logs, SCRIPT_LIMIT, SHELL_TIMEOUT)
}

/// Like [`run_script`] with explicit limits. `limit` does not count time spent
/// inside `pm.shell`; `shell_timeout` caps each `pm.shell` call.
pub fn run_script_with_limit(
    source: &str,
    name: &str,
    state: &mut ScriptState,
    shell: &ShellSettings,
    logs: &mut Vec<LogLine>,
    limit: Duration,
    shell_timeout: Duration,
) -> Result<(), ScriptError> {
    if source.trim().is_empty() {
        return Ok(());
    }
    let fail = |msg: String| ScriptError { message: format!("{name}: {msg}") };

    let state_json = serde_json::to_string(&*state).map_err(|e| fail(format!("cannot pass state to script: {e}")))?;
    let runtime = Runtime::new().map_err(|e| fail(format!("cannot start script engine: {e}")))?;
    runtime.set_memory_limit(MEMORY_LIMIT);
    runtime.set_max_stack_size(STACK_LIMIT);

    // Deadline bookkeeping: shell time is subtracted from the elapsed time.
    let start = Instant::now();
    let shell_nanos = Arc::new(AtomicU64::new(0));
    let timed_out = Arc::new(AtomicBool::new(false));
    {
        let (shell_nanos, timed_out) = (Arc::clone(&shell_nanos), Arc::clone(&timed_out));
        runtime.set_interrupt_handler(Some(Box::new(move || {
            let spent = Duration::from_nanos(shell_nanos.load(Ordering::Relaxed));
            let over = start.elapsed().saturating_sub(spent) >= limit;
            if over {
                timed_out.store(true, Ordering::Relaxed);
            }
            over
        })));
    }

    let context = Context::full(&runtime).map_err(|e| fail(format!("cannot start script engine: {e}")))?;
    let collected: Arc<Mutex<Vec<LogLine>>> = Arc::default();

    let outcome = context.with(|ctx| -> Result<String, String> {
        install_natives(&ctx, shell, shell_timeout, &shell_nanos, &collected, &state_json)
            .map_err(|e| format!("cannot start script engine: {e}"))?;
        eval_named(&ctx, PRELUDE, "prelude").map_err(|e| format!("internal error: {e}"))?;
        eval_named(&ctx, source, "script").map_err(|e| describe(e, &timed_out, limit))?;
        // Let promise callbacks run; scripts cannot await real I/O anyway.
        while ctx.execute_pending_job() {}
        ctx.eval::<String, _>("__finish()").catch(&ctx).map_err(|e| describe_caught(e.to_string()))
    });

    if let Ok(lines) = collected.lock() {
        logs.extend(lines.iter().cloned());
    }
    let json = outcome.map_err(fail)?;
    let new: FinalState = serde_json::from_str(&json).map_err(|e| fail(format!("cannot read script result: {e}")))?;
    state.env = new.env;
    state.globals = new.globals;
    state.runtime = new.runtime;
    state.request = new.request;
    Ok(())
}

/// Defines `__state`, `__log` and `__shell` on the global object.
fn install_natives<'js>(
    ctx: &Ctx<'js>,
    shell: &ShellSettings,
    shell_timeout: Duration,
    shell_nanos: &Arc<AtomicU64>,
    logs: &Arc<Mutex<Vec<LogLine>>>,
    state_json: &str,
) -> rquickjs::Result<()> {
    let globals = ctx.globals();
    globals.set("__state", state_json)?;

    let logs = Arc::clone(logs);
    globals.set(
        "__log",
        Function::new(ctx.clone(), move |level: String, message: String| {
            if let Ok(mut lines) = logs.lock() {
                lines.push(LogLine { level: LogLevel::parse(&level), message });
            }
        })?,
    )?;

    let shell = shell.clone();
    let shell_nanos = Arc::clone(shell_nanos);
    globals.set(
        "__shell",
        Function::new(ctx.clone(), move |cmd: String| -> String {
            let began = Instant::now();
            let result = shell::run_shell(&cmd, &shell, shell_timeout);
            let spent = u64::try_from(began.elapsed().as_nanos()).unwrap_or(u64::MAX);
            shell_nanos.fetch_add(spent, Ordering::Relaxed);
            match result {
                Ok(out) => serde_json::json!({ "ok": true, "out": out }),
                Err(err) => serde_json::json!({ "ok": false, "err": err }),
            }
            .to_string()
        })?,
    )?;
    Ok(())
}

fn eval_named<'js>(ctx: &Ctx<'js>, source: &str, filename: &str) -> Result<(), rquickjs::CaughtError<'js>> {
    let mut options = rquickjs::context::EvalOptions::default();
    options.filename = Some(filename.to_string());
    ctx.eval_with_options::<(), _>(source, options).catch(ctx)
}

/// Turns a caught engine error into message text, with ` (line N)` when the stack names the script.
fn describe(err: rquickjs::CaughtError<'_>, timed_out: &AtomicBool, limit: Duration) -> String {
    if timed_out.load(Ordering::Relaxed) {
        return format!("script timed out after {}", format_limit(limit));
    }
    describe_caught(err.to_string())
}

fn describe_caught(raw: String) -> String {
    // `CaughtError` renders as "Error: message\n    at <fn> (script:LINE:COL)\n...".
    let line = raw.lines().find_map(|l| {
        let at = l.find("script:")?;
        let digits: String = l[at + "script:".len()..].chars().take_while(char::is_ascii_digit).collect();
        digits.parse::<u32>().ok()
    });
    let mut message = raw.lines().next().unwrap_or("").trim().to_string();
    if let Some(stripped) = message.strip_prefix("Error: ") {
        message = stripped.to_string();
    }
    match line {
        Some(n) => format!("{message} (line {n})"),
        None => message,
    }
}

fn format_limit(d: Duration) -> String {
    let ms = d.as_millis();
    if ms % 1000 == 0 { format!("{}s", ms / 1000) } else { format!("{ms}ms") }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st() -> ScriptState {
        ScriptState {
            request: ScriptRequest { method: "GET".into(), url: "http://x/".into(), ..Default::default() },
            ..Default::default()
        }
    }

    fn run(src: &str, state: &mut ScriptState) -> Result<(), ScriptError> {
        run_script(src, "Test", state, &ShellSettings::default(), &mut Vec::new())
    }

    fn with_response(body: &str) -> ScriptState {
        ScriptState {
            response: Some(ScriptResponse {
                code: 200,
                status: "OK".into(),
                headers: vec![("Content-Type".into(), "application/json".into()), ("Set-Cookie".into(), "a=1".into()), ("set-cookie".into(), "b=2".into())],
                body: body.into(),
                time_ms: 42,
            }),
            ..st()
        }
    }

    #[test]
    fn env_set_persists_in_state() {
        let mut s = st();
        run("pm.environment.set('token', 'abc'); pm.globals.set('g', 1);", &mut s).unwrap();
        assert_eq!(s.env["token"], "abc");
        assert_eq!(s.globals["g"], "1");
        assert!(s.runtime.is_empty());
    }

    #[test]
    fn set_coerces_to_string() {
        let mut s = st();
        run("pm.environment.set('o', {a: [1, 2]}); pm.environment.set('n', 5); pm.environment.set('b', true);", &mut s).unwrap();
        assert_eq!(s.env["o"], r#"{"a":[1,2]}"#);
        assert_eq!(s.env["n"], "5");
        assert_eq!(s.env["b"], "true");
    }

    #[test]
    fn unset_removes() {
        let mut s = st();
        s.env.insert("a".into(), "1".into());
        s.globals.insert("g".into(), "1".into());
        run("pm.environment.unset('a'); pm.globals.unset('g'); pm.environment.unset('missing');", &mut s).unwrap();
        assert!(s.env.is_empty() && s.globals.is_empty());
    }

    #[test]
    fn get_has_and_to_object() {
        let mut s = st();
        s.env.insert("a".into(), "1".into());
        run(
            "if (pm.environment.get('nope') !== undefined) throw new Error('get');
             if (!pm.environment.has('a') || pm.environment.has('b') || pm.globals.has('a')) throw new Error('has');
             pm.environment.set('json', JSON.stringify(pm.environment.toObject()));",
            &mut s,
        )
        .unwrap();
        assert_eq!(s.env["json"], r#"{"a":"1"}"#);
    }

    #[test]
    fn variables_get_priority() {
        let mut s = st();
        s.globals.insert("k".into(), "global".into());
        s.globals.insert("only_g".into(), "g".into());
        s.env.insert("k".into(), "env".into());
        s.env.insert("only_e".into(), "e".into());
        s.runtime.insert("only_r".into(), "r".into());
        run(
            "const v = pm.variables;
             pm.environment.set('first', v.get('k'));
             pm.variables.set('k', 'runtime');
             pm.environment.set('second', v.get('k'));
             pm.environment.set('all', [v.get('only_g'), v.get('only_e'), v.get('only_r'), String(v.get('zzz'))].join(','));",
            &mut s,
        )
        .unwrap();
        assert_eq!(s.env["first"], "env");
        assert_eq!(s.env["second"], "runtime");
        assert_eq!(s.env["all"], "g,e,r,undefined");
        assert_eq!(s.runtime["k"], "runtime");
        assert_eq!(s.env["k"], "env");
    }

    #[test]
    fn response_json_parse() {
        let mut s = with_response(r#"{"access_token":"t0k","n":{"x":[1,2]}}"#);
        run(
            "pm.environment.set('token', pm.response.json().access_token);
             pm.environment.set('x', pm.response.json().n.x[1]);
             pm.environment.set('code', pm.response.code);
             pm.environment.set('status', pm.response.status);
             pm.environment.set('rt', pm.response.responseTime);
             pm.environment.set('text', pm.response.text().length);",
            &mut s,
        )
        .unwrap();
        assert_eq!(s.env["token"], "t0k");
        assert_eq!(s.env["x"], "2");
        assert_eq!(s.env["code"], "200");
        assert_eq!(s.env["status"], "OK");
        assert_eq!(s.env["rt"], "42");
        assert_eq!(s.env["text"], "38");
        assert!(s.response.is_some());
    }

    #[test]
    fn response_json_invalid_throws() {
        let mut s = with_response("not json");
        let e = run("pm.response.json()", &mut s).unwrap_err();
        assert!(e.message.starts_with("Test: "), "{}", e.message);
    }

    #[test]
    fn response_headers_get_is_case_insensitive() {
        let mut s = with_response("{}");
        run(
            "pm.environment.set('ct', pm.response.headers.get('CONTENT-type'));
             pm.environment.set('sc', pm.response.headers.get('Set-Cookie'));
             pm.environment.set('none', String(pm.response.headers.get('x-missing')));",
            &mut s,
        )
        .unwrap();
        assert_eq!(s.env["ct"], "application/json");
        assert_eq!(s.env["sc"], "a=1, b=2");
        assert_eq!(s.env["none"], "undefined");
    }

    #[test]
    fn response_is_undefined_in_pre_request() {
        let mut s = st();
        run("pm.environment.set('t', typeof pm.response); if (pm.response) throw new Error('set');", &mut s).unwrap();
        assert_eq!(s.env["t"], "undefined");
    }

    #[test]
    fn request_header_upsert() {
        let mut s = st();
        s.request.headers = vec![("Accept".into(), "*/*".into()), ("X-Id".into(), "1".into()), ("x-id".into(), "dup".into())];
        run(
            "pm.request.headers.upsert({key: 'x-ID', value: '2'});
             pm.request.headers.upsert({key: 'New', value: 'n'});
             pm.request.headers.add({key: 'Extra', value: 'e'});
             pm.request.headers.add({key: 'Extra', value: 'e2'});
             pm.request.headers.remove('ACCEPT');
             pm.environment.set('got', pm.request.headers.get('NEW'));
             pm.environment.set('missing', String(pm.request.headers.get('nope')));",
            &mut s,
        )
        .unwrap();
        assert_eq!(
            s.request.headers,
            vec![
                ("x-ID".to_string(), "2".to_string()),
                ("New".to_string(), "n".to_string()),
                ("Extra".to_string(), "e".to_string()),
                ("Extra".to_string(), "e2".to_string()),
            ]
        );
        assert_eq!(s.env["got"], "n");
        assert_eq!(s.env["missing"], "undefined");
    }

    #[test]
    fn request_fields_are_assignable() {
        let mut s = st();
        s.request.body = Some("old".into());
        run("pm.request.method = 'POST'; pm.request.url = pm.request.url + 'a?b=1'; pm.request.body = pm.request.body + '!';", &mut s).unwrap();
        assert_eq!(s.request.method, "POST");
        assert_eq!(s.request.url, "http://x/a?b=1");
        assert_eq!(s.request.body.as_deref(), Some("old!"));
    }

    #[test]
    fn request_body_undefined_and_settable() {
        let mut s = st();
        run("if (pm.request.body !== undefined) throw new Error('not undefined'); pm.request.body = JSON.stringify({a: 1});", &mut s).unwrap();
        assert_eq!(s.request.body.as_deref(), Some(r#"{"a":1}"#));
        run("pm.request.body = undefined", &mut s).unwrap();
        assert_eq!(s.request.body, None);
    }

    #[test]
    fn console_log_collected() {
        let mut s = st();
        let mut logs = Vec::new();
        run_script(
            "console.log('a', {b: 1}, [2], 3, null, undefined);
             console.info('i'); console.warn('w'); console.error(new Error('boom'));",
            "Test",
            &mut s,
            &ShellSettings::default(),
            &mut logs,
        )
        .unwrap();
        let got: Vec<_> = logs.iter().map(|l| (l.level, l.message.as_str())).collect();
        assert_eq!(
            got,
            vec![
                (LogLevel::Log, r#"a {"b":1} [2] 3 null undefined"#),
                (LogLevel::Info, "i"),
                (LogLevel::Warn, "w"),
                (LogLevel::Error, "Error: boom"),
            ]
        );
    }

    #[test]
    fn log_level_serializes_lowercase() {
        let l = LogLine { level: LogLevel::Warn, message: "m".into() };
        assert_eq!(serde_json::to_string(&l).unwrap(), r#"{"level":"warn","message":"m"}"#);
    }

    #[test]
    fn throw_returns_error() {
        let mut s = st();
        let mut logs = Vec::new();
        let e = run_script("console.log('before');\n\nthrow new Error('nope');", "Request pre-request", &mut s, &ShellSettings::default(), &mut logs).unwrap_err();
        assert_eq!(e.message, "Request pre-request: nope (line 3)");
        assert_eq!(e.to_string(), e.message);
        assert_eq!(logs.len(), 1, "logs before the error are kept");
    }

    #[test]
    fn throw_non_error_value() {
        let mut s = st();
        let e = run("throw 'plain'", &mut s).unwrap_err();
        assert!(e.message.starts_with("Test: "), "{}", e.message);
        assert!(e.message.contains("plain"), "{}", e.message);
    }

    #[test]
    fn syntax_error_returns_error() {
        let mut s = st();
        let e = run("let x = ;", &mut s).unwrap_err();
        assert!(e.message.starts_with("Test: "), "{}", e.message);
        assert!(e.message.contains("line 1"), "{}", e.message);
    }

    #[test]
    fn failed_script_leaves_state_untouched() {
        let mut s = st();
        let before = s.clone();
        let _ = run("pm.environment.set('a', '1'); throw new Error('x');", &mut s).unwrap_err();
        assert_eq!(s, before);
    }

    #[test]
    fn empty_source_is_ok_and_untouched() {
        let mut s = st();
        let before = s.clone();
        run("  \n\t ", &mut s).unwrap();
        run("", &mut s).unwrap();
        assert_eq!(s, before);
    }

    #[test]
    fn infinite_loop_times_out() {
        let mut s = st();
        let start = Instant::now();
        let e = run_script_with_limit("while (true) {}", "Test", &mut s, &ShellSettings::default(), &mut Vec::new(), Duration::from_millis(200), Duration::from_secs(5)).unwrap_err();
        assert!(e.message.contains("timed out"), "{}", e.message);
        assert!(start.elapsed() < Duration::from_secs(3));
    }

    #[test]
    fn timeout_cannot_be_caught_by_script() {
        let mut s = st();
        let e = run_script_with_limit(
            "try { while (true) {} } catch (e) { pm.environment.set('caught', '1'); }",
            "Test",
            &mut s,
            &ShellSettings::default(),
            &mut Vec::new(),
            Duration::from_millis(200),
            Duration::from_secs(5),
        );
        assert!(e.is_err());
        assert!(s.env.is_empty());
    }

    #[test]
    fn no_node_or_io_globals() {
        let mut s = st();
        run(
            "pm.environment.set('t', [typeof require, typeof process, typeof fetch, typeof XMLHttpRequest, typeof std, typeof os, typeof __shell, typeof __state].join(','));",
            &mut s,
        )
        .unwrap();
        assert_eq!(s.env["t"], "undefined,undefined,undefined,undefined,undefined,undefined,undefined,undefined");
    }

    #[cfg(unix)]
    mod shell {
        use super::*;

        #[test]
        fn shell_returns_trimmed_stdout() {
            let mut s = st();
            run("pm.environment.set('out', pm.shell('echo hi'));", &mut s).unwrap();
            assert_eq!(s.env["out"], "hi");
        }

        #[test]
        fn shell_nonzero_throws_with_stderr() {
            let mut s = st();
            let e = run("pm.shell('echo bad >&2; exit 2')", &mut s).unwrap_err();
            assert!(e.message.contains("exit code 2"), "{}", e.message);
            assert!(e.message.contains("bad"), "{}", e.message);
            assert!(e.message.starts_with("Test: "), "{}", e.message);
        }

        #[test]
        fn shell_error_can_be_caught() {
            let mut s = st();
            run("try { pm.shell('exit 1') } catch (e) { pm.environment.set('msg', e.message) }", &mut s).unwrap();
            assert!(s.env["msg"].starts_with("command failed (exit code 1)"), "{}", s.env["msg"]);
        }

        #[test]
        fn shell_stderr_ignored_on_success() {
            let mut s = st();
            run("pm.environment.set('out', pm.shell('echo warn >&2; echo ok'));", &mut s).unwrap();
            assert_eq!(s.env["out"], "ok");
        }

        #[test]
        fn shell_timeout_kills() {
            let mut s = st();
            let start = Instant::now();
            let e = run_script_with_limit("pm.shell('sleep 5')", "Test", &mut s, &ShellSettings::default(), &mut Vec::new(), Duration::from_secs(60), Duration::from_millis(200)).unwrap_err();
            assert!(e.message.contains("timed out"), "{}", e.message);
            assert!(start.elapsed() < Duration::from_secs(2), "took {:?}", start.elapsed());
        }

        #[test]
        fn shell_time_does_not_count_against_script_limit() {
            let mut s = st();
            run_script_with_limit("pm.shell('sleep 0.6'); pm.environment.set('done', '1');", "Test", &mut s, &ShellSettings::default(), &mut Vec::new(), Duration::from_millis(300), Duration::from_secs(5)).unwrap();
            assert_eq!(s.env["done"], "1");
        }

        #[test]
        fn script_time_after_shell_is_still_limited() {
            let mut s = st();
            let e = run_script_with_limit("pm.shell('sleep 0.3'); while (true) {}", "Test", &mut s, &ShellSettings::default(), &mut Vec::new(), Duration::from_millis(200), Duration::from_secs(5)).unwrap_err();
            assert!(e.message.contains("timed out"), "{}", e.message);
        }
    }
}
