//! E2E smoke test: validates the spawn → port-bind → tree-kill cycle against
//! an actual Redis install. Skipped automatically if Redis isn't available
//! on the host. Cleans up on completion.

#![cfg(windows)]

use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use tokio::process::Command;

const CREATE_NO_WINDOW: u32 = 0x08000000;
const REDIS_PORT: u16 = 6379;

#[path = "../src/scoop.rs"]
mod scoop;

fn redis_bin() -> Option<PathBuf> {
    let root = scoop::scoop_root()?;
    let bin = root
        .join("apps")
        .join("redis")
        .join("current")
        .join("redis-server.exe");
    if bin.exists() { Some(bin) } else { None }
}

fn pid_on_port(port: u16) -> Option<u32> {
    listeners::get_process_by_port(port, listeners::Protocol::TCP)
        .ok()
        .map(|p| p.pid)
}

#[tokio::test(flavor = "current_thread")]
async fn spawn_redis_check_port_kill() {
    let Some(bin) = redis_bin() else {
        eprintln!("skipping: redis not installed via scoop");
        return;
    };

    if pid_on_port(REDIS_PORT).is_some() {
        eprintln!("skipping: port {REDIS_PORT} is already bound");
        return;
    }

    // Spawn redis-server in the background.
    let mut cmd = Command::new(&bin);
    cmd.stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null());
    cmd.creation_flags(CREATE_NO_WINDOW);

    let child = cmd.spawn().expect("spawn redis-server");
    let pid = child.id().expect("redis-server has PID");

    // Give Redis a moment to bind the port.
    let mut bound: Option<u32> = None;
    for _ in 0..30 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if let Some(p) = pid_on_port(REDIS_PORT) {
            bound = Some(p);
            break;
        }
    }
    let bound_pid = bound.unwrap_or_else(|| {
        // Cleanup before panicking.
        let _ = std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .creation_flags(CREATE_NO_WINDOW)
            .status();
        panic!("redis never bound to port {REDIS_PORT} within 3s")
    });

    eprintln!(
        "spawned redis pid={} port-pid={} (match={})",
        pid,
        bound_pid,
        pid == bound_pid
    );
    assert_eq!(bound_pid, pid, "port-listener PID should match spawned PID");

    // Kill via the same path scoop_ops uses.
    let kill_status = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(CREATE_NO_WINDOW)
        .status()
        .await
        .expect("taskkill status");
    assert!(kill_status.success(), "taskkill should succeed");

    // Confirm port is released.
    let mut released = false;
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(100)).await;
        if pid_on_port(REDIS_PORT).is_none() {
            released = true;
            break;
        }
    }
    assert!(released, "port should be released after taskkill");

    // Drop the leaked Child handle (process is already dead).
    drop(child);
}
