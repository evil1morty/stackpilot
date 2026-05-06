# Changelog

All notable changes to Stackpilot are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.1.0] — 2026-05-06

The first cut. Five working pages, seven services, five stack presets,
parallel manifest parsing, streaming installs, port-conflict guard,
tree-kill cancellation, theme toggle, GitHub Actions release pipeline.

### Added

- **Catalog**: walks `~/scoop/buckets/<name>/bucket/*.json` with rayon —
  3,800+ manifests parsed in ~0.4 s. Search with 180 ms debounce,
  bucket filter chips, installed-only toggle, install detection via
  `~/scoop/apps/<name>/current/install.json`.
- **Install / uninstall pipeline**: `tauri::ipc::Channel<ScoopEvent>`
  streams stdout + stderr line-by-line. Hidden console window
  (`CREATE_NO_WINDOW`). Cancellation via `taskkill /T /F /PID` for
  process-tree kill. Single-op guard via `running_pid` mutex.
- **Service control** for Redis, PostgreSQL, MySQL, MariaDB, MongoDB,
  Nginx, Caddy. Tracks spawned children; detects external services via
  `listeners` crate (`GetExtendedTcpTable`). Port pre-check rejects
  start when default port is bound. `Open data folder` opens the persist
  directory in Explorer.
- **Presets**: LEMP, Postgres Stack, MERN Lite, Caddy Lab, Cache Only.
  Sequential install of missing apps + best-effort service start.
  Cancellation generation counter aborts remaining steps when user
  presses Cancel.
- **Bootstrap**: detects missing Scoop on first launch; one-click
  `irm get.scoop.sh | iex` with streamed output.
- **Logs**: terminal-style live viewer with stdout / stderr / system
  colour coding, elapsed counter, Cancel + Clear actions, "Jump to live"
  pill when scrolled up.
- **Theme**: dark / light / system, persisted in localStorage.
- **GitHub Actions release workflow** (`.github/workflows/release.yml`):
  builds MSI on tag push, creates draft release.

### Tests

- 15 Rust unit tests (catalog parsing, app-ref validation, services
  table integrity, presets table integrity)
- `catalog_smoke` integration test against the live `~/scoop` install
- `services_smoke` integration test that actually spawns redis-server,
  confirms port 6379 binds with the spawned PID, kills via
  `taskkill /T /F /PID`, and verifies the port released

[Unreleased]: https://github.com/USER/stackpilot/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/USER/stackpilot/releases/tag/v0.1.0
