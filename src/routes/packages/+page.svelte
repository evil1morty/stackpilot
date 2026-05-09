<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import type { AppEntry, CatalogStats, ScoopStatus, SortBy } from "$lib/types";
  import AppCard from "$lib/components/AppCard.svelte";
  import BootstrapModal from "$lib/components/BootstrapModal.svelte";

  const SORT_KEY = "stackpilot.packages.sort";
  const SORT_LABEL: Record<SortBy, string> = {
    bestMatch: "Best match",
    popular: "Popular",
    recent: "Recently updated",
    name: "Name (A-Z)",
  };

  let scoop = $state<ScoopStatus | null>(null);
  let stats = $state<CatalogStats | null>(null);
  let results = $state<AppEntry[]>([]);
  let query = $state("");
  let bucket = $state<string | null>(null);
  let installedOnly = $state(false);
  let sort = $state<SortBy>("bestMatch");
  let loading = $state(true);
  let refreshing = $state(false);
  let error = $state<string | null>(null);

  let searchTimer: ReturnType<typeof setTimeout> | null = null;
  let searchInput: HTMLInputElement | undefined = $state();

  onMount(async () => {
    const stored = localStorage.getItem(SORT_KEY) as SortBy | null;
    if (stored && stored in SORT_LABEL) sort = stored;
    await refreshAll();
  });

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      const target = e.target as HTMLElement | null;
      const isTyping =
        target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA");

      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        searchInput?.focus();
        searchInput?.select();
        return;
      }
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "r") {
        e.preventDefault();
        if (!refreshing && scoop?.installed) refreshCatalog();
        return;
      }
      if (e.key === "Escape" && isTyping && target === searchInput && query) {
        e.preventDefault();
        clearQuery();
        searchInput?.blur();
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  async function refreshAll() {
    loading = true;
    error = null;
    try {
      const status = await ipc.scoopCheck();
      scoop = status;
      if (status.installed) {
        stats = await ipc.catalogStats();
        await fetchResults();
      } else {
        stats = null;
        results = [];
      }
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function fetchResults() {
    try {
      results = await ipc.catalogList(query, bucket ?? undefined, installedOnly, sort);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function setSort(next: SortBy) {
    sort = next;
    localStorage.setItem(SORT_KEY, next);
    fetchResults();
  }

  function debouncedFetch() {
    if (searchTimer) clearTimeout(searchTimer);
    searchTimer = setTimeout(fetchResults, 180);
  }

  async function refreshCatalog() {
    refreshing = true;
    try {
      stats = await ipc.catalogRefresh();
      await fetchResults();
    } finally {
      refreshing = false;
    }
  }

  function selectBucket(b: string | null) {
    bucket = b;
    fetchResults();
  }

  function toggleInstalled() {
    installedOnly = !installedOnly;
    fetchResults();
  }

  function clearQuery() {
    query = "";
    fetchResults();
  }

  const placeholder = $derived.by(() => {
    if (!stats) return "Search packages…";
    return `Search ${stats.total.toLocaleString()} packages…`;
  });
</script>

<section class="page">
  <header class="head">
    <div>
      <h1>Packages</h1>
      <p class="lede">
        {#if stats}
          <button
            class="stat-link"
            class:active={installedOnly}
            onclick={toggleInstalled}
            title="Toggle installed-only filter"
          >
            {stats.installed} installed
          </button>
        {:else if loading}
          Loading…
        {:else}
          Browse Scoop manifests, install services, manage your stack.
        {/if}
      </p>
    </div>
    {#if scoop?.installed}
      <button
        class="btn"
        class:spinning={refreshing}
        onclick={refreshCatalog}
        disabled={refreshing}
        title="Re-scan buckets (Ctrl+R)"
      >
        <svg
          class="refresh-icon"
          viewBox="0 0 24 24"
          width="14"
          height="14"
          fill="none"
          stroke="currentColor"
          stroke-width="2"
          stroke-linecap="round"
          stroke-linejoin="round"
        >
          <polyline points="23 4 23 10 17 10" />
          <polyline points="1 20 1 14 7 14" />
          <path d="M3.51 9a9 9 0 0 1 14.85-3.36L23 10M1 14l4.64 4.36A9 9 0 0 0 20.49 15" />
        </svg>
        {refreshing ? "Refreshing…" : "Refresh"}
      </button>
    {/if}
  </header>

  {#if error}
    <div class="error-banner">
      <strong>Error:</strong>
      {error}
    </div>
  {/if}

  {#if scoop?.installed && stats}
    <div class="controls">
      <div class="search">
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="11" cy="11" r="8" />
          <line x1="21" y1="21" x2="16.65" y2="16.65" />
        </svg>
        <input
          type="text"
          {placeholder}
          bind:value={query}
          bind:this={searchInput}
          oninput={debouncedFetch}
        />
        {#if query}
          <button class="clear" onclick={clearQuery} aria-label="Clear search">×</button>
        {:else}
          <kbd class="hint" title="Focus search">Ctrl K</kbd>
        {/if}
      </div>

      <button
        class="filter-pill"
        class:active={installedOnly}
        onclick={toggleInstalled}
        title="Show only installed apps"
      >
        Installed only
      </button>

      <div class="sort">
        <label for="sort-select" class="sort-label">Sort</label>
        <select
          id="sort-select"
          value={sort}
          onchange={(e) => setSort((e.currentTarget as HTMLSelectElement).value as SortBy)}
        >
          {#each Object.entries(SORT_LABEL) as [value, label] (value)}
            <option {value}>{label}</option>
          {/each}
        </select>
      </div>
    </div>

    <div class="bucket-row">
      <button
        class="chip"
        class:active={bucket == null}
        onclick={() => selectBucket(null)}
      >
        All <span class="count">{stats.total}</span>
      </button>
      {#each stats.buckets as b (b.name)}
        <button
          class="chip"
          class:active={bucket === b.name}
          onclick={() => selectBucket(b.name)}
        >
          {b.name} <span class="count">{b.count}</span>
        </button>
      {/each}
    </div>

    <div class="grid">
      {#each results as app (app.bucket + "/" + app.name)}
        <AppCard {app} onChanged={refreshAll} />
      {/each}
    </div>

    {#if results.length === 0 && !loading}
      <div class="empty">
        <p>No packages match.</p>
        {#if query || bucket || installedOnly}
          <button
            class="btn"
            onclick={() => {
              query = "";
              bucket = null;
              installedOnly = false;
              fetchResults();
            }}
          >
            Clear filters
          </button>
        {/if}
      </div>
    {/if}

    {#if results.length === 300}
      <p class="cap-hint">
        Showing the first 300 results. Refine your search to narrow further.
      </p>
    {/if}
  {/if}

  {#if loading && !scoop}
    <div class="loading-state"><div class="spinner"></div></div>
  {/if}
</section>

{#if scoop && !scoop.installed}
  <BootstrapModal
    onResolved={(s) => {
      scoop = s;
      if (s.installed) refreshAll();
    }}
  />
{/if}

<style>
  .page {
    padding: 28px 36px 60px 36px;
    max-width: 1400px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 24px;
  }

  h1 {
    font-size: 24px;
    font-weight: 600;
    margin: 0 0 4px 0;
    letter-spacing: -0.01em;
  }

  .lede {
    color: var(--text-dim);
    margin: 0;
    font-size: 13px;
  }

  .dot-sep {
    color: var(--text-muted);
    margin: 0 4px;
  }

  .stat-link {
    background: transparent;
    border: none;
    padding: 0;
    font: inherit;
    color: var(--text-dim);
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 2px;
    text-decoration-color: var(--border-strong);
    text-decoration-thickness: 1px;
    transition: color 120ms ease, text-decoration-color 120ms ease;
  }

  .stat-link:hover {
    color: var(--text);
    text-decoration-color: var(--text-dim);
  }

  .stat-link.active {
    color: var(--accent);
    text-decoration-color: var(--accent);
    font-weight: 600;
  }

  .error-banner {
    background: var(--danger-soft);
    border: 1px solid var(--danger);
    color: var(--danger);
    padding: 10px 14px;
    border-radius: var(--radius-sm);
    margin-bottom: 16px;
    font-size: 13px;
  }

  .controls {
    display: flex;
    gap: 8px;
    margin-bottom: 12px;
  }

  .search {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 0 10px;
    transition: border-color 120ms ease, background 120ms ease;
  }

  .search:focus-within {
    border-color: var(--accent);
    background: var(--bg-2);
  }

  .search svg {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .search input {
    flex: 1;
    border: none;
    background: transparent;
    padding: 8px 0;
  }

  .search input:focus {
    border: none;
    background: transparent;
  }

  .clear {
    color: var(--text-muted);
    font-size: 18px;
    line-height: 1;
    padding: 0 4px;
    border: none;
    cursor: pointer;
  }

  .clear:hover {
    color: var(--text);
  }

  .hint {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 10px;
    color: var(--text-muted);
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    padding: 1px 6px;
    user-select: none;
    flex-shrink: 0;
  }

  .filter-pill {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 0 14px;
    color: var(--text-dim);
    font-size: 12.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .filter-pill:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .filter-pill.active {
    background: var(--accent-soft);
    color: var(--accent);
    border-color: var(--accent-soft);
  }

  .sort {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 0 4px 0 10px;
    transition: border-color 120ms ease, background 120ms ease;
  }

  .sort:focus-within {
    border-color: var(--accent);
    background: var(--bg-2);
  }

  .sort-label {
    font-size: 11px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    user-select: none;
  }

  .sort select {
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    border: none;
    color: var(--text);
    font-size: 12.5px;
    font-weight: 500;
    padding: 6px 22px 6px 4px;
    cursor: pointer;
    background-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='10' viewBox='0 0 24 24' fill='none' stroke='%238a92a3' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><polyline points='6 9 12 15 18 9'/></svg>");
    background-repeat: no-repeat;
    background-position: right 6px center;
  }

  .sort select:focus {
    outline: none;
    border: none;
  }

  .sort select option {
    background: var(--bg-1);
    color: var(--text);
  }

  .bucket-row {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: 20px;
  }

  .chip {
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 4px 12px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-dim);
    cursor: pointer;
    transition: all 120ms ease;
    display: inline-flex;
    align-items: center;
    gap: 6px;
  }

  .chip:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .chip.active {
    background: var(--accent);
    color: var(--bg-0);
    border-color: var(--accent);
  }

  .chip .count {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-muted);
    background: var(--bg-2);
    padding: 1px 6px;
    border-radius: 999px;
  }

  .chip.active .count {
    color: var(--text);
    background: rgba(255, 255, 255, 0.18);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 12px;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 80px 0;
    color: var(--text-muted);
    font-size: 13px;
  }

  .empty p {
    margin: 0;
  }

  .cap-hint {
    text-align: center;
    color: var(--text-muted);
    font-size: 12px;
    margin: 24px 0 0 0;
  }

  .loading-state {
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 50vh;
  }

  .spinner {
    width: 24px;
    height: 24px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  .btn.spinning .refresh-icon {
    animation: spin 0.9s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
