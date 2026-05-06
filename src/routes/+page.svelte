<script lang="ts">
  import { onMount } from "svelte";
  import { ipc, type PingResponse } from "$lib/ipc";

  let pong = $state<PingResponse | null>(null);
  let err = $state<string | null>(null);

  onMount(async () => {
    try {
      pong = await ipc.ping();
    } catch (e) {
      err = e instanceof Error ? e.message : String(e);
    }
  });
</script>

<section class="page">
  <header>
    <h1>Catalog</h1>
    <p class="lede">Browse Scoop manifests, install services, manage your stack.</p>
  </header>

  <div class="placeholder">
    <div class="placeholder-card">
      <h2>Scoop status</h2>
      {#if err}
        <span class="badge badge-danger">IPC error</span>
        <pre>{err}</pre>
      {:else if pong}
        {#if pong.scoop_root}
          <span class="badge badge-success">Detected</span>
          <p class="path">{pong.scoop_root}</p>
        {:else}
          <span class="badge badge-warning">Not installed</span>
          <p class="hint">{pong.message}</p>
        {/if}
      {:else}
        <span class="badge">Checking…</span>
      {/if}
    </div>

    <p class="phase-note">Phase 0 scaffold — catalog UI lands in Phase 1.</p>
  </div>
</section>

<style>
  .page {
    padding: 32px 40px;
    max-width: 1200px;
  }

  header {
    margin-bottom: 32px;
  }

  h1 {
    font-size: 24px;
    font-weight: 600;
    letter-spacing: -0.01em;
    margin: 0 0 4px 0;
  }

  h2 {
    font-size: 13px;
    font-weight: 500;
    margin: 0 0 12px 0;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .lede {
    color: var(--text-dim);
    margin: 0;
    font-size: 14px;
  }

  .placeholder {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .placeholder-card {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 20px 24px;
    max-width: 520px;
  }

  .path {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    color: var(--text-dim);
    margin: 12px 0 0 0;
  }

  .hint {
    color: var(--text-dim);
    font-size: 13px;
    margin: 12px 0 0 0;
  }

  pre {
    margin: 12px 0 0 0;
    font-size: 12px;
    color: var(--danger);
    white-space: pre-wrap;
  }

  .phase-note {
    color: var(--text-muted);
    font-size: 12px;
    margin: 0;
  }
</style>
