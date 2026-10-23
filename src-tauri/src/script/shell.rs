//! Runs a terminal command for `pm.shell(...)`.

use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crate::model::ShellSettings;

/// How long to wait for the output pipes to close after the shell has exited.
/// A daemon started by the command may keep them open; we then use what we have.
const PIPE_GRACE: Duration = Duration::from_millis(500);

/// Runs `cmd` in the user's shell and returns its trimmed stdout.
///
/// Fails when the exit code is not zero (the message has the code and stderr),
/// when `timeout` passes (the process tree is killed), or when the shell cannot
/// start. Stderr is ignored when the command succeeds.
pub fn run_shell(cmd: &str, settings: &ShellSettings, timeout: Duration) -> Result<String, String> {
    let mut command = build_command(cmd, settings);
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // Own process group, so a timeout can kill the whole tree.
        command.process_group(0);
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = command.spawn().map_err(|e| format!("could not start shell: {e}"))?;
    let out = capture(child.stdout.take());
    let err = capture(child.stderr.take());

    let deadline = Instant::now() + timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {}
            Err(e) => {
                kill_tree(&mut child);
                return Err(format!("could not wait for command: {e}"));
            }
        }
        if Instant::now() >= deadline {
            kill_tree(&mut child);
            return Err(format!("command timed out after {}", format_duration(timeout)));
        }
        thread::sleep(Duration::from_millis(10));
    };

    let stdout = out.collect(PIPE_GRACE);
    if status.success() {
        return Ok(after_marker(&stdout).trim().to_string());
    }
    let stderr = err.collect(PIPE_GRACE);
    let reason = match status.code() {
        Some(code) => format!("exit code {code}"),
        None => "terminated by signal".to_string(),
    };
    Err(format!("command failed ({reason}): {}", clean_stderr(&stderr)))
}

/// Printed right before the user's command in interactive shells, so anything the shell's
/// startup files print (banners, motd) can be cut off.
const OUTPUT_MARK: &str = "__POSTCARD_OUTPUT_START__";

fn with_marker(cmd: &str) -> String {
    format!("printf '%s' '{OUTPUT_MARK}'\n{cmd}")
}

fn after_marker(stdout: &str) -> &str {
    match stdout.find(OUTPUT_MARK) {
        Some(i) => &stdout[i + OUTPUT_MARK.len()..],
        None => stdout,
    }
}

/// Drops the noise an interactive shell without a terminal prints (job control warnings, "logout").
fn clean_stderr(stderr: &str) -> String {
    stderr
        .lines()
        .filter(|l| {
            let l = l.trim();
            !(l.contains("cannot set terminal process group")
                || l.contains("no job control in this shell")
                || l == "logout")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

/// Output of one pipe, filled by a background reader thread.
struct Captured {
    buf: Arc<Mutex<Vec<u8>>>,
    done: Receiver<()>,
}

impl Captured {
    /// Waits up to `grace` for EOF, then returns whatever was read.
    fn collect(self, grace: Duration) -> String {
        let _ = self.done.recv_timeout(grace);
        let bytes = self.buf.lock().map(|b| b.clone()).unwrap_or_default();
        String::from_utf8_lossy(&bytes).into_owned()
    }
}

/// Reads `pipe` on its own thread so a full pipe never blocks the child.
fn capture<R: Read + Send + 'static>(pipe: Option<R>) -> Captured {
    let buf = Arc::new(Mutex::new(Vec::new()));
    let (tx, done) = mpsc::channel();
    if let Some(mut pipe) = pipe {
        let shared = Arc::clone(&buf);
        thread::spawn(move || {
            let mut chunk = [0u8; 8192];
            loop {
                match pipe.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if let Ok(mut b) = shared.lock() {
                            b.extend_from_slice(&chunk[..n]);
                        }
                    }
                }
            }
            let _ = tx.send(());
        });
    } else {
        let _ = tx.send(());
    }
    Captured { buf, done }
}

fn format_duration(d: Duration) -> String {
    let ms = d.as_millis();
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms % 1000 == 0 {
        format!("{}s", ms / 1000)
    } else {
        format!("{:.1}s", d.as_secs_f64())
    }
}

/// Kills the child and everything it started, then reaps it.
fn kill_tree(child: &mut Child) {
    #[cfg(unix)]
    {
        // The child leads its own process group (see `process_group(0)`).
        let _ = Command::new("kill")
            .args(["-9", "--", &format!("-{}", child.id())])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let _ = Command::new("taskkill")
            .args(["/PID", &child.id().to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .status();
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[cfg(unix)]
fn build_command(cmd: &str, settings: &ShellSettings) -> Command {
    let custom = settings.unix_custom.as_deref().map(str::trim).filter(|s| !s.is_empty());
    let login = std::env::var("SHELL").ok().filter(|s| !s.trim().is_empty());
    let (program, flag, script) = match (custom, login) {
        (Some(custom), _) => (custom.to_string(), "-c", cmd.to_string()),
        // Interactive login shell, so PATH set in ~/.bashrc / ~/.zshrc (e.g. by gcloud's installer) applies.
        (None, Some(shell)) => (shell, "-ilc", with_marker(cmd)),
        (None, None) => ("/bin/sh".to_string(), "-c", cmd.to_string()),
    };
    let mut command = Command::new(program);
    command.arg(flag).arg(script);
    command
}

#[cfg(windows)]
fn build_command(cmd: &str, settings: &ShellSettings) -> Command {
    use crate::model::WindowsShell;
    use std::os::windows::process::CommandExt;

    let mut kind = settings.windows.clone();
    if kind == WindowsShell::Wsl && which::which("wsl.exe").is_err() {
        kind = WindowsShell::Cmd;
    }
    match kind {
        WindowsShell::Wsl => {
            let mut command = Command::new("wsl.exe");
            if let Some(distro) = settings.wsl_distro.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
                command.args(["-d", distro]);
            }
            // Interactive login shell, so PATH set in ~/.bashrc applies.
            command.args(["-e", "bash", "-ilc"]).arg(with_marker(cmd));
            command
        }
        WindowsShell::Cmd => {
            let mut command = Command::new("cmd");
            // cmd has its own quoting rules; pass the text through untouched.
            // /S keeps cmd from stripping quotes unpredictably: the outer quotes are removed, the rest is kept.
            command.raw_arg(format!("/S /C \"{cmd}\""));
            command
        }
        WindowsShell::PowerShell => {
            let mut command = Command::new("powershell.exe");
            command.args(["-NoProfile", "-NonInteractive", "-Command", cmd]);
            command
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    fn run(cmd: &str, secs: u64) -> Result<String, String> {
        run_shell(cmd, &ShellSettings::default(), Duration::from_secs(secs))
    }

    #[test]
    fn startup_banner_is_cut_off() {
        assert_eq!(after_marker(&format!("Welcome!\n{OUTPUT_MARK}tok\n")), "tok\n");
        assert_eq!(after_marker("plain"), "plain");
        // The default shell wraps the command with the marker itself.
        assert_eq!(run("echo tok", 5).unwrap(), "tok");
    }

    #[test]
    fn echo_is_trimmed() {
        assert_eq!(run("echo hi", 5).unwrap(), "hi");
        assert_eq!(run("printf '  a b \\n\\n'", 5).unwrap(), "a b");
    }

    #[test]
    fn nonzero_reports_code_and_stderr() {
        let e = run("echo boom >&2; exit 3", 5).unwrap_err();
        assert_eq!(e, "command failed (exit code 3): boom");
    }

    #[test]
    fn stderr_ignored_on_success() {
        assert_eq!(run("echo warn >&2; echo ok", 5).unwrap(), "ok");
    }

    #[test]
    fn timeout_kills_process_tree() {
        let start = Instant::now();
        let e = run_shell("sleep 5", &ShellSettings::default(), Duration::from_millis(200)).unwrap_err();
        assert_eq!(e, "command timed out after 200ms");
        assert!(start.elapsed() < Duration::from_secs(2), "took {:?}", start.elapsed());
    }

    #[test]
    fn timeout_kills_grandchildren_holding_pipes() {
        let start = Instant::now();
        let e = run_shell("sleep 5 & sleep 5; echo done", &ShellSettings::default(), Duration::from_millis(200)).unwrap_err();
        assert!(e.contains("timed out"), "{e}");
        assert!(start.elapsed() < Duration::from_secs(2), "took {:?}", start.elapsed());
    }

    #[test]
    fn background_daemon_holding_pipe_does_not_hang() {
        let start = Instant::now();
        let out = run("sleep 5 & echo started", 10).unwrap();
        assert_eq!(out, "started");
        assert!(start.elapsed() < Duration::from_secs(3), "took {:?}", start.elapsed());
    }

    #[test]
    fn large_output_does_not_deadlock() {
        let out = run("head -c 300000 /dev/zero | tr '\\0' x; echo y >&2", 10).unwrap();
        assert_eq!(out.len(), 300_000);
    }

    #[test]
    fn custom_shell_is_used() {
        let s = ShellSettings { unix_custom: Some("/bin/sh".into()), ..Default::default() };
        assert_eq!(run_shell("echo $0", &s, Duration::from_secs(5)).unwrap(), "/bin/sh");
    }

    #[test]
    fn missing_custom_shell_is_start_error() {
        let s = ShellSettings { unix_custom: Some("/definitely/not/a/shell".into()), ..Default::default() };
        let e = run_shell("true", &s, Duration::from_secs(5)).unwrap_err();
        assert!(e.starts_with("could not start shell:"), "{e}");
    }
}
