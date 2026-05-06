<script lang="ts">
  import type { AppEntry } from "$lib/types";

  let { app }: { app: AppEntry } = $props();

  const installed = $derived(app.installed);
  const updateAvailable = $derived(
    installed != null && installed.version !== app.version && app.version !== "?",
  );
</script>

<article class="card">
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

    <div class="status">
      {#if installed}
        {#if updateAvailable}
          <span class="badge badge-warning">update · {installed.version}</span>
        {:else}
          <span class="badge badge-success">installed</span>
        {/if}
        {#if installed.hold}
          <span class="badge">held</span>
        {/if}
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
    min-height: 22px;
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

  .status {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }
</style>
