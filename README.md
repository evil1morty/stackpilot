# Stackpilot

A modern Windows GUI for browsing, installing, and running dev services
(Redis, Postgres, MySQL, Nginx, Mongo, …) with [Scoop](https://scoop.sh)
under the hood.

Think XAMPP, but for everything Scoop has — and built on Tauri 2 so it
weighs ~10 MB instead of 100+.

---

## Features

- **Catalog** — browse every Scoop manifest (3,800+ across `main` and
  `extras`), filter by bucket, search with sub-200 ms debounce, install
  with one click.
- **Services** — start / stop / restart Redis, Postgres, MySQL, MariaDB,
  MongoDB, Nginx, Caddy. Detects port conflicts before launching, and
  picks up services started outside Stackpilot via port-PID lookup.
- **Presets** — five curated stacks (LEMP, Postgres Stack, MERN Lite,
  Caddy Lab, Cache Only) — one click installs everything missing and
  starts every service.
- **Live logs** — terminal-style streaming output for every install,
  uninstall, and bootstrap. Cancel mid-run with `taskkill /T /F`.
- **First-run bootstrap** — if Scoop isn't installed, Stackpilot runs
  the official `irm get.scoop.sh | iex` for you.

## Stack

- **Tauri 2.11** — native window with WebView2 + Rust backend
- **SvelteKit** + Svelte 5 runes (SPA mode via `adapter-static`)
- **Bun** for the JS toolchain
- **rayon** for parallel manifest parsing — 3,800 JSON files in ~0.4 s
- **listeners** crate for `GetExtendedTcpTable`-based port → PID lookup
- **tokio::process** for spawning + streaming child output

## Build from source

Prerequisites:

- Windows 10 1803+ (for WebView2)
- [Bun](https://bun.sh) ≥ 1.3
- Rust stable (MSVC toolchain)
- Microsoft C++ Build Tools (Desktop development with C++)

```sh
git clone <this repo>
cd stackpilot
bun install
bun run tauri dev      # development with hot reload
bun run tauri build    # production MSI in src-tauri/target/release/bundle/msi
```

## Tests

```sh
cd src-tauri
cargo test --lib                    # 15 unit tests
cargo test --test catalog_smoke     # walks live ~/scoop catalog
cargo test --test services_smoke    # spawns redis-server, kills via tree-kill
```

## Project layout

```
stackpilot/
├── src-tauri/
│   ├── src/
│   │   ├── catalog.rs              # bucket walker, manifest parser
│   │   ├── known_services.rs       # 7-service curated table
│   │   ├── presets.rs              # 5 curated stack bundles
│   │   ├── scoop.rs                # SCOOP env / ~/scoop resolver
│   │   ├── state.rs                # AppState (cache + tracked PIDs)
│   │   └── commands/
│   │       ├── catalog.rs          # catalog_list / refresh / scoop_check
│   │       ├── scoop_ops.rs        # install / uninstall / bootstrap / cancel
│   │       ├── services.rs         # start / stop / restart / open data
│   │       └── presets_ops.rs      # presets_list / presets_apply
│   └── tests/
│       ├── catalog_smoke.rs
│       └── services_smoke.rs
└── src/
    ├── routes/                     # /, /services, /presets, /logs
    └── lib/
        ├── ipc.ts                  # typed invoke wrappers
        ├── types.ts                # mirrors Rust serde shapes
        ├── components/
        └── stores/
```

## Architecture notes

**Scoop is treated as a data layer, not a CLI.** Browsing reads JSON
manifests directly from disk — no `scoop search` shell-out per keypress.
Installs and uninstalls are the only operations that spawn `scoop.ps1`,
and they're streamed line-by-line via `tauri::ipc::Channel<ScoopEvent>`.

**Services are tracked in-process.** When you start Redis, the
`tokio::process::Child` handle goes into `AppState.tracked`. On stop we
`taskkill /T /F /PID` so child workers go too. Status detection layers:
tracked-and-alive → port-bound externally → stopped. Children survive
app close (drop is a no-op); next launch picks them up as
`runningExternal`.

**Cancellation crosses op boundaries.** A monotonic
`AppState.cancellation_gen` is bumped by `scoop_cancel`. Multi-step
orchestrators like `presets_apply` snapshot it at start and check
between every install / start step, so Cancel actually aborts the
remaining work in a preset.

## Roadmap

### v0.1 (current)

- ✅ Catalog browse / install / uninstall
- ✅ Service control (7 services)
- ✅ Presets (5 stacks)
- ✅ Live logs with cancel
- ✅ Theme (dark / light / system)

### v1.1

- [ ] Tauri updater (signed releases via GitHub Releases)
- [ ] Config file editing (Monaco — postgresql.conf, redis.conf, nginx.conf)
- [ ] Auto-start services on app launch (opt-in)
- [ ] Custom bucket management
- [ ] Per-service log file tailing

## License

MIT
