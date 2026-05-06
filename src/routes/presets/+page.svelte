<script lang="ts">
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ipc } from "$lib/ipc";
  import { operation } from "$lib/stores/operation.svelte";
  import type { PresetInfo } from "$lib/types";

  let presets = $state<PresetInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  onMount(async () => {
    await refresh();
  });

  async function refresh() {
    try {
      presets = await ipc.presetsList();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function apply(p: PresetInfo) {
    if (operation.busy) return;
    goto("/logs");
    try {
      await operation.runPreset(p.key, p.name);
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<section class="page">
  <header class="head">
    <div>
      <h1>Presets</h1>
      <p class="lede">
        Curated stacks. One click installs everything missing and starts the services.
      </p>
    </div>
  </header>

  {#if error}
    <div class="error-banner">
      <strong>Error:</strong>
      {error}
    </div>
  {/if}

  {#if loading}
    <p class="loading">Loading presets…</p>
  {/if}

  <div class="grid">
    {#each presets as p (p.key)}
      <article class="preset-card">
        <header>
          <h2>{p.name}</h2>
          <span class="ratio" class:ready={p.appsInstalled === p.appsTotal}>
            {p.appsInstalled}/{p.appsTotal}
          </span>
        </header>

        <p class="description">{p.description}</p>

        <div class="apps">
          {#each p.apps as a (a.scoopApp)}
            <span class="app-chip" class:installed={a.installed}>
              {#if a.installed}
                <svg viewBox="0 0 16 16" width="11" height="11" fill="none"
                  stroke="currentColor" stroke-width="2.5" stroke-linecap="round"
                  stroke-linejoin="round" aria-hidden="true">
                  <polyline points="3 8.5 6.5 12 13 4.5" />
                </svg>
              {:else}
                <svg viewBox="0 0 16 16" width="11" height="11" fill="none"
                  stroke="currentColor" stroke-width="2" stroke-linecap="round"
                  stroke-linejoin="round" aria-hidden="true">
                  <line x1="8" y1="3" x2="8" y2="13" />
                  <line x1="3" y1="8" x2="13" y2="8" />
                </svg>
              {/if}
              {a.scoopApp}
            </span>
          {/each}
        </div>

        <footer>
          <span class="meta">
            Starts: {p.autoStart.map((s) => s.display).join(", ")}
          </span>
          <button
            class="apply-btn"
            class:full={p.appsInstalled === p.appsTotal}
            disabled={operation.busy}
            onclick={() => apply(p)}
          >
            {#if p.appsInstalled === p.appsTotal}
              Start stack
            {:else}
              Install &amp; start
            {/if}
          </button>
        </footer>
      </article>
    {/each}
  </div>
</section>

<style>
  .page {
    padding: 28px 36px 60px 36px;
    max-width: 1400px;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 28px;
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
    max-width: 520px;
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

  .loading {
    color: var(--text-dim);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(340px, 1fr));
    gap: 14px;
  }

  .preset-card {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition: border-color 120ms ease;
  }

  .preset-card:hover {
    border-color: var(--border-strong);
  }

  .preset-card header {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
  }

  h2 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
    letter-spacing: -0.01em;
  }

  .ratio {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 11.5px;
    color: var(--text-muted);
    background: var(--bg-2);
    padding: 2px 8px;
    border-radius: 999px;
  }

  .ratio.ready {
    color: var(--success);
    background: var(--success-soft);
  }

  .description {
    margin: 0;
    font-size: 13px;
    color: var(--text-dim);
    line-height: 1.5;
  }

  .apps {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }

  .app-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 500;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }

  .app-chip.installed {
    background: var(--success-soft);
    border-color: transparent;
    color: var(--success);
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: auto;
    padding-top: 8px;
  }

  .meta {
    font-size: 11px;
    color: var(--text-muted);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .apply-btn {
    background: var(--accent);
    border: 1px solid var(--accent);
    color: var(--bg-0);
    border-radius: var(--radius-sm);
    padding: 6px 14px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    transition: all 120ms ease;
    flex-shrink: 0;
  }

  .apply-btn:hover:not(:disabled) {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }

  .apply-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .apply-btn.full {
    background: var(--success);
    border-color: var(--success);
  }

  .apply-btn.full:hover:not(:disabled) {
    background: #2bc888;
    border-color: #2bc888;
  }
</style>
