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
  /** ISO 8601 date of the last manifest commit (online results only). */
  committed?: string;
  /** Bucket repo URL — `https://github.com/ScoopInstaller/Main` etc. */
  repository?: string;
  /** GitHub stars on the bucket repo (online results only). */
  repositoryStars?: number;
  /** Search highlight fragments wrapped in `<mark>`, keyed by field. */
  highlights?: Record<string, string[]>;
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

export type SortBy = "bestMatch" | "popular" | "recent" | "name";

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

export type ServiceHealth = "unknown" | "starting" | "healthy" | "degraded";

export type ServiceInfo = {
  key: string;
  scoopApp: string;
  display: string;
  category: ServiceCategory | string;
  installed: boolean;
  status: ServiceStatus;
  health: ServiceHealth;
  defaultPort: number | null;
  persistDir: string | null;
  binPath: string | null;
};

export type ServiceLog = {
  key: string;
  path: string;
  sizeBytes: number;
  lines: string[];
};

export type ConfigFileInfo = {
  path: string;
  label: string;
  language: string;
  exists: boolean;
  sizeBytes: number;
  /** File lives in the install dir and gets clobbered by `scoop update`. */
  volatile: boolean;
};

export type Project = {
  key: string;
  name: string;
  rootDir: string;
  services: string[];
  envVars: Record<string, string>;
  notes: string;
  createdAt: number;
  lastActiveAt: number | null;
};

export type ProjectInfo = Project & { isActive: boolean };

export type ProjectInput = {
  name: string;
  rootDir?: string;
  services?: string[];
  envVars?: Record<string, string>;
  notes?: string;
};

export type ServiceFailure = {
  key: string;
  error: string;
};

export type ActivationReport = {
  stopped: string[];
  started: string[];
  failed: ServiceFailure[];
  project: ProjectInfo;
};

export type PresetApp = {
  scoopApp: string;
  installed: boolean;
};

export type PresetService = {
  key: string;
  display: string;
};

export type PresetInfo = {
  key: string;
  name: string;
  description: string;
  apps: PresetApp[];
  autoStart: PresetService[];
  appsInstalled: number;
  appsTotal: number;
};

