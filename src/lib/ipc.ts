import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  AppEntry,
  CatalogStats,
  PresetInfo,
  ScoopEvent,
  ScoopStatus,
  ServiceInfo,
} from "./types";

export type PingResponse = {
  ok: boolean;
  message: string;
  scoopRoot: string | null;
};

export { Channel };

export const ipc = {
  ping: () => invoke<PingResponse>("ping"),
  scoopCheck: () => invoke<ScoopStatus>("scoop_check"),
  catalogList: (query?: string, bucket?: string, installedOnly?: boolean) =>
    invoke<AppEntry[]>("catalog_list", {
      query: query?.trim() ? query.trim() : null,
      bucket: bucket ?? null,
      installedOnly: installedOnly ?? false,
    }),
  catalogStats: () => invoke<CatalogStats>("catalog_stats"),
  catalogRefresh: () => invoke<CatalogStats>("catalog_refresh"),

  scoopInstall: (app: string, onEvent: Channel<ScoopEvent>) =>
    invoke<void>("scoop_install", { app, onEvent }),
  scoopUninstall: (app: string, onEvent: Channel<ScoopEvent>) =>
    invoke<void>("scoop_uninstall", { app, onEvent }),
  scoopUpdate: (app: string, onEvent: Channel<ScoopEvent>) =>
    invoke<void>("scoop_update", { app, onEvent }),
  scoopBootstrap: (onEvent: Channel<ScoopEvent>) =>
    invoke<void>("scoop_bootstrap", { onEvent }),
  scoopCancel: () => invoke<boolean>("scoop_cancel"),

  servicesList: () => invoke<ServiceInfo[]>("services_list"),
  servicesStart: (key: string) => invoke<ServiceInfo>("services_start", { key }),
  servicesStop: (key: string) => invoke<ServiceInfo>("services_stop", { key }),
  servicesRestart: (key: string) =>
    invoke<ServiceInfo>("services_restart", { key }),
  servicesOpenData: (key: string) =>
    invoke<void>("services_open_data", { key }),

  presetsList: () => invoke<PresetInfo[]>("presets_list"),
  presetsApply: (key: string, onEvent: Channel<ScoopEvent>) =>
    invoke<void>("presets_apply", { key, onEvent }),
};
