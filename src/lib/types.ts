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
