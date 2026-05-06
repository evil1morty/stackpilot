# Stackpilot

A modern Windows GUI for browsing, installing, and running dev services
(Redis, Postgres, MySQL, Nginx, Mongo, …) with [Scoop](https://scoop.sh) under the hood.

> Status: Phase 0 — scaffold.

## Stack

- **Tauri 2** — native window, Rust backend, ~10 MB binary
- **SvelteKit** with `adapter-static` (SPA mode) + Svelte 5 runes
- **Bun** for the JS toolchain
- Frontend talks to Rust via typed `invoke()` wrappers in `src/lib/ipc.ts`

## Phases

| Phase | Scope                                                         |
| ----- | ------------------------------------------------------------- |
| 0     | Scaffold, sidebar nav, IPC sanity check                       |
| 1     | Catalog: read Scoop manifests, search/filter, install detect  |
| 2     | Install / uninstall pipeline with streaming logs              |
| 3     | Service control (start/stop/restart) with port-conflict guard |
| 4     | Curated stack presets                                         |
| 5     | Theme, persisted state, updater, GitHub Actions, polish       |

## Develop

```sh
bun install
bun run tauri dev
```

## Build

```sh
bun run tauri build
```
