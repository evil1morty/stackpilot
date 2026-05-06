import { invoke } from "@tauri-apps/api/core";
import type { AppEntry, CatalogStats, ScoopStatus } from "./types";

export type PingResponse = {
  ok: boolean;
  message: string;
  scoopRoot: string | null;
};

export const ipc = {
  ping: () => invoke<PingResponse>("ping"),
  scoopCheck: () => invoke<ScoopStatus>("scoop_check"),
  catalogList: (
    query?: string,
    bucket?: string,
    installedOnly?: boolean,
  ) =>
    invoke<AppEntry[]>("catalog_list", {
      query: query?.trim() ? query.trim() : null,
      bucket: bucket ?? null,
      installedOnly: installedOnly ?? false,
    }),
  catalogStats: () => invoke<CatalogStats>("catalog_stats"),
  catalogRefresh: () => invoke<CatalogStats>("catalog_refresh"),
};
