<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { ipc } from "$lib/ipc";
  import type { ServiceInfo } from "$lib/types";
  import ServiceCard from "$lib/components/ServiceCard.svelte";
  import { startPolling } from "$lib/util/poll";

  let services = $state<ServiceInfo[]>([]);
  let loading = $state(true);
  let manualRefreshing = $state(false);
  let error = $state<string | null>(null);

  let stopPolling: (() => void) | null = null;
  let unmounted = false;

  onMount(async () => {
    await refresh();
    if (unmounted) return;
    stopPolling = startPolling(refresh, 2500);
  });

  onDestroy(() => {
    unmounted = true;
    stopPolling?.();
  });

  async function refresh() {
    try {
      const next = await ipc.servicesList();
      if (unmounted) return;
      services = next;
      error = null;
    } catch (e) {
      if (unmounted) return;
      error = e instanceof Error ? e.message : String(e);
    } finally {
      if (!unmounted) loading = false;
    }
  }

  async function manualRefresh() {
    manualRefreshing = true;
    try {
      await refresh();
    } finally {
      manualRefreshing = false;
    }
  }

  function patch(next: ServiceInfo) {
    services = services.map((s) => (s.key === next.key ? next : s));
  }

  const installed = $derived(services.filter((s) => s.installed));
  const running = $derived(
    installed.filter((s) => s.status.kind !== "stopped").length,
  );
</script>

<section class="page">
  <header class="head">
    <div>
      <h1>Services</h1>
      <p class="lede">
        {#if loading && services.length === 0}
          Loading…
        {:else if installed.length === 0}
          No supported services installed yet.
        {:else}
          {installed.length} installed
          <span class="dot-sep">·</span>
          {running} running
        {/if}
      </p>
    </div>
    {#if installed.length > 0}
      <button
        class="btn"
        class:spinning={manualRefreshing}
        onclick={manualRefresh}
        disabled={manualRefreshing}
        title="Refresh service status"
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
        {manualRefreshing ? "Refreshing…" : "Refresh"}
      </button>
    {/if}
  </header>

  {#if error}
    <div class="error-banner">
      <strong>Error:</strong>
      {error}
    </div>
  {/if}

  {#if installed.length > 0}
    <div class="grid">
      {#each installed as svc (svc.key)}
        <ServiceCard service={svc} onChanged={patch} />
      {/each}
    </div>
  {:else if !loading}
    <div class="empty">
      <p class="empty-title">Nothing to start yet.</p>
      <p class="empty-body">
        Stackpilot can manage Redis, PostgreSQL, MySQL, MariaDB, MongoDB, Nginx,
        Apache, Caddy, Memcached, Meilisearch, MinIO and Mosquitto. Install any of
        them from Packages and they'll show up here.
      </p>
      <button class="btn btn-primary" onclick={() => goto("/packages")}>
        Browse packages →
      </button>
    </div>
  {/if}
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

  .error-banner {
    background: var(--danger-soft);
    border: 1px solid var(--danger);
    color: var(--danger);
    padding: 10px 14px;
    border-radius: var(--radius-sm);
    margin-bottom: 16px;
    font-size: 13px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 14px;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 14px;
    padding: 80px 24px;
    text-align: center;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-1);
  }

  .empty-title {
    margin: 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
  }

  .empty-body {
    margin: 0;
    font-size: 13px;
    color: var(--text-dim);
    max-width: 480px;
    line-height: 1.5;
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
