//! Live health probes per service. We deliberately keep these cheap (TCP
//! connect with a 500 ms timeout, or HTTP GET with 800 ms) so the polling
//! call from the Services page stays snappy.

use std::time::Duration;

use serde::Serialize;
use tokio::net::TcpStream;
use tokio::time::timeout;

use crate::known_services::KnownService;

#[derive(Serialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ServiceHealth {
    /// No health probe defined for this service, or the service isn't
    /// running so health doesn't apply.
    Unknown,
    /// Recently started; port not yet bound. Within the grace window.
    Starting,
    /// Probe succeeded.
    Healthy,
    /// Process is alive and port is bound, but the probe didn't get the
    /// answer it expected (e.g. service still warming up).
    Degraded,
}

#[derive(Clone, Copy, Debug)]
enum Probe {
    /// Just establishing a TCP connection counts as healthy. Right for
    /// Redis, Postgres, MySQL, MariaDB, Mongo.
    Tcp(u16),
    /// HTTP GET — anything 1xx/2xx/3xx/4xx counts as healthy (server
    /// responded). 5xx or no response = degraded.
    Http(&'static str),
}

fn probe_for(svc: &KnownService) -> Option<Probe> {
    match svc.key {
        "caddy" => Some(Probe::Http("http://127.0.0.1:2019/config/")),
        "nginx" => Some(Probe::Http("http://127.0.0.1:80/")),
        "apache" => Some(Probe::Http("http://127.0.0.1:80/")),
        "meilisearch" => Some(Probe::Http("http://127.0.0.1:7700/health")),
        "minio" => Some(Probe::Http("http://127.0.0.1:9000/minio/health/live")),
        _ => svc.default_port.map(Probe::Tcp),
    }
}

pub async fn check(svc: &KnownService) -> ServiceHealth {
    let Some(probe) = probe_for(svc) else {
        return ServiceHealth::Unknown;
    };
    match probe {
        Probe::Tcp(port) => check_tcp(port).await,
        Probe::Http(url) => check_http(url).await,
    }
}

async fn check_tcp(port: u16) -> ServiceHealth {
    let addr = format!("127.0.0.1:{port}");
    match timeout(Duration::from_millis(500), TcpStream::connect(&addr)).await {
        Ok(Ok(_)) => ServiceHealth::Healthy,
        _ => ServiceHealth::Degraded,
    }
}

async fn check_http(url: &str) -> ServiceHealth {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_millis(800))
        .build();
    let Ok(client) = client else {
        return ServiceHealth::Degraded;
    };
    match timeout(Duration::from_millis(800), client.get(url).send()).await {
        Ok(Ok(resp)) if resp.status().as_u16() < 500 => ServiceHealth::Healthy,
        _ => ServiceHealth::Degraded,
    }
}
