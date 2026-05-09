//! Single shared reqwest::Client. Building a Client is expensive (TLS
//! provider init, connection pool, DNS resolver) so we hand out one process-
//! wide instance and let it pool connections across health probes,
//! scoopsearch queries, and any future HTTP callers.

use std::sync::OnceLock;
use std::time::Duration;

use reqwest::Client;

static CLIENT: OnceLock<Client> = OnceLock::new();

/// Default request timeout. Per-call code paths can wrap the future in their
/// own `tokio::time::timeout` for tighter bounds.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(8);

pub fn shared() -> &'static Client {
    CLIENT.get_or_init(|| {
        Client::builder()
            .timeout(DEFAULT_TIMEOUT)
            .pool_idle_timeout(Some(Duration::from_secs(30)))
            .user_agent(concat!("stackpilot/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("build shared reqwest client")
    })
}
