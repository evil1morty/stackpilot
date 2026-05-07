//! Auto-generate self-signed certificates for vhost hostnames using OpenSSL.
//! Same approach Laragon takes (an openssl-config template with subjectAltName)
//! — produces a cert + key per host, valid for 825 days, 2048-bit RSA, with
//! `subjectAltName=DNS:<host>,DNS:*.<host>` so it covers wildcards.
//!
//! Stackpilot does NOT auto-install the cert into the Windows root store.
//! Browsers will show a warning the first visit; users can either accept
//! it or import the .crt manually from the certs dir.

use std::fs;
use std::path::PathBuf;
use std::process::Stdio;

use crate::persistence;

const VALID_DAYS: u32 = 825; // matches modern browser cap (Apple's policy)

pub struct CertPaths {
    pub crt: PathBuf,
    pub key: PathBuf,
}

pub fn certs_dir() -> PathBuf {
    persistence::state_dir().join("certs")
}

/// Mint a self-signed cert + key for `host`. Idempotent — if both files
/// already exist for this host (and aren't expired), returns the existing
/// paths without re-running OpenSSL.
pub fn mint(host: &str) -> Result<CertPaths, String> {
    let safe_host: String = host
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '.' { c } else { '_' })
        .collect();
    if safe_host.is_empty() {
        return Err("invalid host".into());
    }

    let dir = certs_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("create certs dir: {e}"))?;

    let crt = dir.join(format!("{safe_host}.crt"));
    let key = dir.join(format!("{safe_host}.key"));

    if crt.exists() && key.exists() {
        return Ok(CertPaths { crt, key });
    }

    let openssl = locate_openssl().ok_or_else(|| {
        "OpenSSL not found on PATH. Install via `scoop install openssl`.".to_string()
    })?;

    let san = format!("subjectAltName=DNS:{host},DNS:*.{host}");
    let subj = format!("/CN={host}/O=Stackpilot/OU=Local Dev");

    let mut cmd = std::process::Command::new(&openssl);
    cmd.args([
        "req",
        "-x509",
        "-newkey",
        "rsa:2048",
        "-nodes",
        "-days",
        &VALID_DAYS.to_string(),
        "-subj",
        &subj,
        "-addext",
        &san,
        "-keyout",
    ])
    .arg(&key)
    .arg("-out")
    .arg(&crt)
    .stdout(Stdio::null())
    .stderr(Stdio::piped());

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("spawn openssl: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "openssl failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    Ok(CertPaths { crt, key })
}

/// Probe PATH for openssl[.exe], same simple `which` we use for wt.
pub fn locate_openssl() -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        for ext in &["", "exe"] {
            let candidate = if ext.is_empty() {
                dir.join("openssl")
            } else {
                dir.join(format!("openssl.{ext}"))
            };
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certs_dir_is_under_state_dir() {
        let dir = certs_dir();
        assert!(dir.ends_with("certs"));
    }
}
