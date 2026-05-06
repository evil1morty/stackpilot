<script lang="ts">
  import { goto } from "$app/navigation";
  import type { AppEntry } from "$lib/types";
  import { operation } from "$lib/stores/operation.svelte";

  let { app, onChanged }: { app: AppEntry; onChanged?: () => void } = $props();

  const installed = $derived(app.installed);
  const updateAvailable = $derived(
    installed != null && installed.version !== app.version && app.version !== "?",
  );
  const ref = $derived(`${app.bucket}/${app.name}`);

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
    <h3 class="name" title={app.name}>{app.name}</h3>
    <span class="bucket-badge" title="bucket">{app.bucket}</span>
  </header>

  <p class="desc" title={app.description ?? ""}>
    {app.description ?? "—"}
  </p>

  <footer>
    <div class="meta">
      <span class="version">v{app.version}</span>
      {#if app.license}
        <span class="dot-sep">·</span>
        <span class="license">{app.license}</span>
      {/if}
    </div>

    <div class="actions">
      {#if installed && updateAvailable}
        <span class="badge badge-warning" title="installed v{installed.version}">update</span>
      {:else if installed}
        <span class="badge badge-success">installed</span>
      {/if}
      {#if installed?.hold}
        <span class="badge" title="held — scoop unhold to update">held</span>
      {/if}
      {#if installed}
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
    min-height: 132px;
    gap: 8px;
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

  .card:hover .bucket-badge {
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

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin-top: 4px;
    min-height: 26px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .version {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }

  .dot-sep {
    color: var(--text-muted);
  }

  .license {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
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
</style>
