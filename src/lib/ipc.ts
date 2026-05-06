import { Channel, invoke } from "@tauri-apps/api/core";
import type {
  ActivationReport,
  AppEntry,
  CatalogStats,
  ConfigFileInfo,
  PresetInfo,
  ProjectInfo,
  ProjectInput,
  ScoopEvent,
  ScoopStatus,
  ServiceInfo,
  ServiceLog,
  SortBy,
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
  catalogList: (
    query?: string,
    bucket?: string,
    installedOnly?: boolean,
    sort?: SortBy,
  ) =>
    invoke<AppEntry[]>("catalog_list", {
      query: query?.trim() ? query.trim() : null,
      bucket: bucket ?? null,
      installedOnly: installedOnly ?? false,
      sort: sort ?? "bestMatch",
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
  servicesTailLog: (key: string, maxLines?: number) =>
    invoke<ServiceLog>("services_tail_log", {
      key,
      maxLines: maxLines ?? 200,
    }),

  servicesConfigFiles: (key: string) =>
    invoke<ConfigFileInfo[]>("services_config_files", { key }),
  servicesConfigRead: (path: string) =>
    invoke<string>("services_config_read", { path }),
  servicesConfigWrite: (path: string, content: string) =>
    invoke<void>("services_config_write", { path, content }),
  servicesOpenPath: (path: string) =>
    invoke<void>("services_open_path", { path }),

  presetsList: () => invoke<PresetInfo[]>("presets_list"),
  presetsApply: (key: string, onEvent: Channel<ScoopEvent>) =>
    invoke<void>("presets_apply", { key, onEvent }),

  projectsList: () => invoke<ProjectInfo[]>("projects_list"),
  projectsCreate: (input: ProjectInput) =>
    invoke<ProjectInfo>("projects_create", { input }),
  projectsUpdate: (key: string, input: ProjectInput) =>
    invoke<ProjectInfo>("projects_update", { key, input }),
  projectsDelete: (key: string) =>
    invoke<void>("projects_delete", { key }),
  projectsActivate: (key: string) =>
    invoke<ActivationReport>("projects_activate", { key }),
  projectsDeactivate: () =>
    invoke<string[]>("projects_deactivate"),
};
