import { invoke } from "@tauri-apps/api/core";

export type PingResponse = {
  ok: boolean;
  message: string;
  scoop_root: string | null;
};

export const ipc = {
  ping: () => invoke<PingResponse>("ping"),
};
