<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import type { ServiceInfo } from "$lib/types";
  import ServiceCard from "$lib/components/ServiceCard.svelte";

  let services = $state<ServiceInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let pollHandle: ReturnType<typeof setInterval> | null = null;

  onMount(async () => {
    await refresh();
    pollHandle = setInterval(refresh, 2500);
  });

  onDestroy(() => {
    if (pollHandle) clearInterval(pollHandle);
  });

  async function refresh() {
    try {
      services = await ipc.servicesList();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  function patch(next: ServiceInfo) {
    services = services.map((s) => (s.key === next.key ? next : s));
  }

  const installed = $derived(services.filter((s) => s.installed));
  const notInstalled = $derived(services.filter((s) => !s.installed));
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
    <button class="btn" onclick={refresh}>Refresh</button>
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
  {/if}

  {#if notInstalled.length > 0 && !loading}
    <section class="more">
      <h2>More services</h2>
      <p class="more-lede">
        Install any of these from the Catalog to manage them here.
      </p>
      <div class="more-grid">
        {#each notInstalled as svc (svc.key)}
          <ServiceCard service={svc} onChanged={patch} />
        {/each}
      </div>
    </section>
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

  .more {
    margin-top: 40px;
    padding-top: 28px;
    border-top: 1px solid var(--border);
  }

  .more h2 {
    font-size: 13px;
    font-weight: 500;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    margin: 0 0 4px 0;
  }

  .more-lede {
    font-size: 12.5px;
    color: var(--text-muted);
    margin: 0 0 16px 0;
  }

  .more-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 12px;
    opacity: 0.65;
  }
</style>
