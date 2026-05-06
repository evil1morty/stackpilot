<script lang="ts">
  import { goto } from "$app/navigation";
  import type { AppEntry } from "$lib/types";
  import { operation } from "$lib/stores/operation.svelte";
  import { timeAgo } from "$lib/util/timeAgo";

  let { app, onChanged }: { app: AppEntry; onChanged?: () => void } = $props();

  const installed = $derived(app.installed);
  const updateAvailable = $derived(
    installed != null && installed.version !== app.version && app.version !== "?",
  );
  const ref = $derived(`${app.bucket}/${app.name}`);
  // Scoop license fields sometimes look like "MIT|https://..."; only keep the
  // identifier half for display.
  const licenseLabel = $derived(app.license?.split("|")[0]?.trim() || null);
  const committedAgo = $derived(timeAgo(app.committed));
  const isMain = $derived(app.bucket === "main");

  // Match highlight rendering: server returns text with <mark> tags around
  // matched substrings. Escape everything else, then re-allow mark tags.
  function renderHighlight(s: string): string {
    const escaped = s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;")
      .replace(/'/g, "&#39;");
    return escaped
      .replace(/&lt;mark&gt;/g, "<mark>")
      .replace(/&lt;\/mark&gt;/g, "</mark>");
  }

  const nameHtml = $derived.by(() => {
    const hi = app.highlights?.NamePartial?.[0] ?? app.highlights?.Name?.[0];
    return hi ? renderHighlight(hi) : escapeText(app.name);
  });

  const descHtml = $derived.by(() => {
    const text = app.description ?? "";
    if (!text) return "—";
    const hi = app.highlights?.Description?.[0];
    return hi ? renderHighlight(hi) : escapeText(text);
  });

  function escapeText(s: string): string {
    return s
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;");
  }

  let acting = $state(false);

  async function install() {
    acting = true;
    try {
      goto("/logs");
      await operation.runInstall(ref);
      onChanged?.();
    } finally {
      acting = false;
    }
  }

  async function update() {
    acting = true;
    try {
      goto("/logs");
      await operation.runUpdate(app.name);
      onChanged?.();
    } finally {
      acting = false;
    }
  }

  async function uninstall() {
    acting = true;
    try {
      goto("/logs");
      await operation.runUninstall(app.name);
      onChanged?.();
    } finally {
      acting = false;
    }
  }
</script>

<article class="card" class:is-installed={installed != null}>
  <header>
    <h3 class="name" title={app.name}>{@html nameHtml}</h3>
    <span class="bucket-badge" class:main={isMain} title="bucket">{app.bucket}</span>
  </header>

  <p class="desc" title={app.description ?? ""}>{@html descHtml}</p>

  <div class="meta">
    <span class="version">v{app.version}</span>
    {#if licenseLabel}
      <span class="dot-sep">·</span>
      <span class="license">{licenseLabel}</span>
    {/if}
    {#if committedAgo}
      <span class="dot-sep">·</span>
      <span class="updated" title={app.committed}>{committedAgo}</span>
    {/if}
  </div>

  <footer>
    <div class="badges">
      {#if installed && updateAvailable}
        <span class="badge badge-warning" title="installed v{installed.version}">update</span>
      {:else if installed}
        <span class="badge badge-success">installed</span>
      {/if}
      {#if installed?.hold}
        <span class="badge" title="held — scoop unhold to update">held</span>
      {/if}
    </div>

    <div class="actions">
      {#if installed && updateAvailable && !installed.hold}
        <button
          class="card-btn primary"
          disabled={operation.busy || acting}
          onclick={update}
          title="scoop update {app.name}"
        >
          Update
        </button>
        <button class="card-btn ghost" disabled={operation.busy || acting} onclick={uninstall}>
          Remove
        </button>
      {:else if installed}
        <button class="card-btn" disabled={operation.busy || acting} onclick={uninstall}>
          Uninstall
        </button>
      {:else}
        <button
          class="card-btn primary"
          disabled={operation.busy || acting}
          onclick={install}
        >
          Install
        </button>
      {/if}
    </div>
  </footer>
</article>

<style>
  .card {
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 14px 16px;
    transition: border-color 120ms ease, transform 120ms ease, background 120ms ease;
    min-height: 148px;
    gap: 6px;
  }

  .card:hover {
    border-color: var(--border-strong);
    background: var(--bg-2);
  }

  .card.is-installed {
    border-color: rgba(52, 211, 153, 0.18);
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 10px;
  }

  .name {
    font-size: 14px;
    font-weight: 600;
    margin: 0;
    color: var(--text);
    letter-spacing: -0.01em;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .name :global(mark) {
    background: var(--accent-soft);
    color: var(--accent);
    border-radius: 2px;
    padding: 0 1px;
  }

  .bucket-badge {
    font-size: 10px;
    font-weight: 500;
    color: var(--text-muted);
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 2px 8px;
    flex-shrink: 0;
    text-transform: lowercase;
    letter-spacing: 0.02em;
  }

  .bucket-badge.main {
    background: var(--accent-soft);
    color: var(--accent);
    border-color: transparent;
    font-weight: 600;
  }

  .card:hover .bucket-badge:not(.main) {
    background: var(--bg-3);
  }

  .desc {
    font-size: 12.5px;
    color: var(--text-dim);
    margin: 0;
    line-height: 1.45;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    flex: 1;
  }

  .desc :global(mark) {
    background: var(--accent-soft);
    color: var(--accent);
    border-radius: 2px;
    padding: 0 1px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
    margin-top: 2px;
  }

  .version {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    color: var(--text-dim);
  }

  .dot-sep {
    color: var(--text-muted);
  }

  .license,
  .updated {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 4px;
    min-height: 26px;
  }

  .badges {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .actions {
    display: flex;
    gap: 6px;
    flex-shrink: 0;
    align-items: center;
  }

  .card-btn {
    background: var(--bg-2);
    border: 1px solid var(--border);
    color: var(--text);
    border-radius: var(--radius-sm);
    padding: 4px 12px;
    font-size: 11.5px;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .card-btn:hover:not(:disabled) {
    background: var(--bg-3);
    border-color: var(--border-strong);
  }

  .card-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .card-btn.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg-0);
    font-weight: 600;
  }

  .card-btn.primary:hover:not(:disabled) {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }

  .card-btn.ghost {
    background: transparent;
    border-color: transparent;
    color: var(--text-dim);
  }

  .card-btn.ghost:hover:not(:disabled) {
    background: var(--bg-2);
    color: var(--text);
  }
</style>
