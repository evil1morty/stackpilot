export type InstalledInfo = {
  version: string;
  bucket: string | null;
  architecture: string | null;
  hold: boolean;
};

export type AppEntry = {
  name: string;
  bucket: string;
  version: string;
  description: string | null;
  homepage: string | null;
  license: string | null;
  depends: string[];
  suggest: string[];
  bins: string[];
  supportsArch: string[];
  installed: InstalledInfo | null;
};

export type BucketSummary = {
  name: string;
  count: number;
};

export type CatalogStats = {
  total: number;
  installed: number;
  buckets: BucketSummary[];
};

export type ScoopStatus = {
  installed: boolean;
  root: string | null;
  buckets: string[];
};

export type ScoopEvent =
  | { type: "started"; payload: { command: string } }
  | { type: "stdout"; payload: { line: string } }
  | { type: "stderr"; payload: { line: string } }
  | { type: "finished"; payload: { exitCode: number } }
  | { type: "error"; payload: { message: string } };

export type ServiceStatus =
  | { kind: "stopped" }
  | { kind: "runningTracked"; pid: number; startedAt: number }
  | { kind: "runningExternal"; pid: number };

export type ServiceCategory =
  | "database"
  | "cache"
  | "webserver"
  | "queue"
  | "search"
  | "storage";

export type ServiceInfo = {
  key: string;
  scoopApp: string;
  display: string;
  category: ServiceCategory | string;
  installed: boolean;
  status: ServiceStatus;
  defaultPort: number | null;
  persistDir: string | null;
  binPath: string | null;
};

