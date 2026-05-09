# Changelog

All notable changes to Stackpilot are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

## [0.3.1] — 2026-05-09

Performance + correctness pass focused on long-term maintainability and
the start/stop user flow. No new features.

### Fixed

- **`service_logs::tail` partial-line drop**: when reading was truncated
  by the 64 KB window the wrong end was being trimmed, so the last line
  was dropped instead of the (possibly half-record) first line.
- **TOCTOU on `running_pid`** across scoop install/update/uninstall/
  bootstrap and presets_apply. Two concurrent invokes could both pass
  the busy check. Replaced with an `OpSlot` RAII guard that reserves
  the slot atomically and a single shared one across multi-step preset
  applies.
- **TOCTOU on `services_start`**: two concurrent same-key starts could
  both run `init_step` (Postgres `initdb` corrupting the data dir).
  Added `state.starting` set + `StartReservation` guard locking
  tracked, starting, and the port check together.
- **`services_stop` mid-start race**: refuses with a clear error when a
  start is in flight so it can't taskkill a child the start path
  hasn't yet registered.
- **`services_restart`** now surfaces both stop and start errors when
  the start step fails — previously only the start error was shown.
- **`scoop_cancel` slot release**: used to `take()` the slot, which let
  a concurrent op slip in before the in-flight op had unwound. Now
  peeks + bumps cancellation_gen + taskkills; the OpSlot drop on
  natural exit handles release.
- **`validate_config_path`** uses `dunce::canonicalize` and walks the
  deepest existing ancestor, so symlinks/junctions can no longer escape
  the Scoop root.
- **`PortMap::snapshot` failure** now logs + falls back to per-port
  probes instead of silently making every service look stopped.
- Redundant frontend race: home services + logs service-tail effects
  re-check their cancellation flag after the awaited IPC resolves.

### Performance

- Single shared `reqwest::Client` across health probes + scoopsearch
  (was building TLS state per call).
- `CatalogCache` hands out `Arc<Vec<AppEntry>>` instead of cloning the
  3,800-entry vector on every catalog_list keystroke.
- `services_list` takes one `GetExtendedTcpTable` snapshot and reuses
  it across sweep + per-row status + health staleness check (was
  hitting the kernel ~3× per service).
- `scoop_root()` cached for the process lifetime; invalidated on
  `scoop_bootstrap` and `catalog_refresh`.
- Operation log appends switch from O(n²) spread to in-place push +
  splice-trim. Long installs no longer stall the UI.
- Polling pauses while document is hidden (window minimised to tray):
  services list (2.5 s) and service-log tail (1.5 s).
- `services_tail_log` accepts `since_size`; returns `lines:[]` for an
  idle service so the frontend skips the re-render.
- 30-minute wall-clock watchdog on the scoop subprocess so a hung
  install can't latch `running_pid` forever.

### Memory leaks fixed

- `service_logs::reap_orphans` removes log files for uninstalled
  services on every services_list tail.
- `ssl::reap_orphans` removes cert + key files for hosts no longer
  referenced by any project on projects_update / projects_delete.

### Maintenance

- New `crate::winutil` (`hide_console_{tokio,std}`, PATHEXT-aware
  `which`) and `crate::http` (shared client) modules consolidate code
  duplicated across spawn sites and HTTP callers.
- scoop install/update/uninstall collapse into one `run_scoop_app_op`.
- `validate_app_ref` length cap at 128 chars.
- `eprintln!` → `log::warn!`; `env_logger` initialised at startup.
- Rotating config-write backups (`<path>.{1,2,3}.bak`) instead of
  clobbering a single `.bak` on every save; no-op writes are skipped
  entirely.
- Cargo.toml + package.json + tauri.conf.json metadata aligned with
  the actual project: 0.3.1, real description, MIT license.

### Tests

45 unit tests pass; svelte-check 0/0/0; release build green.

## [0.3.0] — 2026-05-07

Five Laragon-inspired UX wins. Stays-in-tray when closed, right-click
context menus, project-aware terminal launcher, pretty-URL vhosts, and
auto-SSL via OpenSSL. Multi-version services moved to v0.4 backlog.

### Added

- **System tray + minimize-to-tray** (Phase A): tray icon with Show /
  Hide / Quit menu, left-click toggles window. Closing the X hides to
  tray (toggle in Settings menu, persisted in localStorage + mirrored
  to a Rust AtomicBool so the close handler reads it sync).
- **Right-click context menus** (Phase B): generic ContextMenu component
  wired to ServiceCard (Start/Stop/Restart/Folder/Configs/Logs) and
  Project cards (Activate/Deactivate/Open terminal/Edit/Delete).
  Auto-clamps to viewport, closes on Escape / outside click / scroll.
- **Open terminal at project root** (Phase C): per-project Terminal
  button + context-menu item. Launches Windows Terminal if installed,
  falls back to PowerShell with `Set-Location`. Project env vars
  propagate through cmd → start → shell so DB_URL etc are pre-set.
- **Pretty URLs / vhosts** (Phase D1): Project gains `vhosts: VHost[]`.
  On activate, Stackpilot emits one nginx server block per vhost into
  `<scoop_persist>/nginx/conf/stackpilot/auto.<project>.<host>.conf`,
  patches `nginx.conf` once with an `include` directive (with marker
  comment + .bak backup), and edits the hosts file via elevated
  PowerShell — only prompts UAC when the file actually changes.
  Auto-restarts nginx if it's tracked-running.
- **Auto-SSL via OpenSSL** (Phase D2): per-vhost SSL toggle. When on,
  mints a 2048-bit RSA self-signed cert (825-day, with
  `subjectAltName=DNS:host,DNS:*.host`) into
  `%APPDATA%\Stackpilot\certs\<host>.{crt,key}` and emits the SSL
  variant of the nginx template. Falls back to HTTP-only with a warning
  if OpenSSL isn't on PATH.

### Changed

- ServiceCard's inline log panel removed in v0.2 polish; Logs button
  now navigates to `/logs?service=<key>` where the new tabs picker
  hosts both Operations and per-service tails.
- "Open data folder" → "Folder", with smart fallback: opens persist
  dir if it exists and has contents, else install dir. Fixes the
  Redis-has-no-persist case.
- Sidebar order: Services first (home page `/`), Packages second.
  Visible Ctrl-N hint chips removed; keyboard shortcuts still work.
- Stop / Restart buttons get danger / warning color treatment.
- Refresh buttons across pages share the same SVG + spin animation.

### Backend modules added

- `tray.rs`, `commands/settings.rs` — system tray + close-to-tray
- `vhosts.rs`, `hosts_file.rs`, `ssl.rs` — vhost emit + hosts edit + cert mint
- `commands/projects_ops.rs::projects_open_terminal` — terminal launcher

### Tests

42 unit tests (4 new vhosts + 5 new hosts_file + 1 new ssl) all green.

### Deferred to v0.4

- Multi-version services (postgres 15 vs 16 per project)
- Apache + Caddy vhost emitters
- Cert-trust install (one UAC for the local CA)
- Procfile-style user-defined services
- Quick-share via cloudflared / ngrok
- Stack templates (1-click WordPress, Laravel)

## [0.2.0] — 2026-05-07

Crash-resilient state, per-service logs, smart init, health checks,
config editor, projects.

### Added

- **Persisted service state** (Phase A): tracked PIDs survive
  Stackpilot restarts. State file at `%APPDATA%\Stackpilot\state.json`,
  re-attach via port-binding verification on launch.
- **Per-service log files** (Phase B): each spawned service's
  stdout/stderr captured to `%APPDATA%\Stackpilot\logs\<key>.log` with
  5 MB rotation. Inline tail panel + `services_tail_log` IPC + later
  moved to a unified Logs page with source picker.
- **Smart service init** (Phase C): Postgres `initdb`, MySQL
  `--initialize-insecure`, MongoDB data dir creation auto-run when
  persist dir is empty. Init output captured to the same log file.
- **Real health checks** (Phase D): TCP / HTTP probes per service,
  five-state UI dot (Stopped / Starting / Healthy / Degraded /
  External). Probes run in parallel via `futures::join_all`.
- **Config editor** (Phase E): "Configs" button per service opens a
  modal with file list, monospace textarea, Ctrl+S to save, automatic
  `.bak` backup, optional restart-after-save. Knows which services
  have which configs (postgresql.conf, redis.conf, nginx.conf, etc).
- **Projects / workspaces** (Phase F): new `/projects` page. Project =
  name + root_dir + services[] + env_vars{}. Activate to swap the
  running stack atomically — stop other-project services, start this
  project's with its env vars.

### Tests

32 unit tests passing.

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

[Unreleased]: https://github.com/evil1morty/stackpilot/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/evil1morty/stackpilot/releases/tag/v0.3.1
[0.3.0]: https://github.com/evil1morty/stackpilot/releases/tag/v0.3.0
[0.2.0]: https://github.com/evil1morty/stackpilot/releases/tag/v0.2.0
[0.1.0]: https://github.com/evil1morty/stackpilot/releases/tag/v0.1.0
