<script lang="ts">
  import { tick } from "svelte";
  import { goto } from "$app/navigation";
  import { ipc } from "$lib/ipc";
  import type { ServiceInfo, ServiceLog } from "$lib/types";

  let {
    service,
    onChanged,
  }: { service: ServiceInfo; onChanged: (next: ServiceInfo) => void } = $props();

  let busy = $state<"start" | "stop" | "restart" | null>(null);
  let actionError = $state<string | null>(null);

  let logsOpen = $state(false);
  let logs = $state<ServiceLog | null>(null);
  let logsErr = $state<string | null>(null);
  let logEl: HTMLDivElement | undefined = $state();

  const isRunning = $derived(service.status.kind !== "stopped");
  const isOurs = $derived(service.status.kind === "runningTracked");
  const pid = $derived(
    service.status.kind === "runningTracked" || service.status.kind === "runningExternal"
      ? service.status.pid
      : null,
  );

  const categoryLabel: Record<string, string> = {
    database: "Database",
    cache: "Cache",
    webserver: "Web server",
    queue: "Queue",
    search: "Search",
    storage: "Storage",
  };

  $effect(() => {
    if (!logsOpen) return;
    let cancelled = false;

    async function refresh() {
      try {
        logs = await ipc.servicesTailLog(service.key, 200);
        logsErr = null;
        await tick();
        if (logEl) logEl.scrollTop = logEl.scrollHeight;
      } catch (e) {
        logsErr = e instanceof Error ? e.message : String(e);
      }
    }
    refresh();

    const interval = setInterval(() => {
      if (!cancelled) refresh();
    }, 1500);

    return () => {
      cancelled = true;
      clearInterval(interval);
    };
  });

  async function call(action: "start" | "stop" | "restart") {
    busy = action;
    actionError = null;
    try {
      const next =
        action === "start"
          ? await ipc.servicesStart(service.key)
          : action === "stop"
            ? await ipc.servicesStop(service.key)
            : await ipc.servicesRestart(service.key);
      onChanged(next);
    } catch (e) {
      actionError = e instanceof Error ? e.message : String(e);
    } finally {
      busy = null;
    }
  }

  async function openData() {
    try {
      await ipc.servicesOpenData(service.key);
    } catch (e) {
      actionError = e instanceof Error ? e.message : String(e);
    }
  }

  function gotoCatalog() {
    goto(`/?q=${encodeURIComponent(service.scoopApp)}`);
  }

  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<article class="card" class:running={isRunning}>
  <header>
    <div class="title-block">
      <h3>{service.display}</h3>
      <span class="cat">{categoryLabel[service.category] ?? service.category}</span>
    </div>

    <div class="status-block">
      {#if !service.installed}
        <span class="badge">not installed</span>
      {:else if service.status.kind === "stopped"}
        <span class="status-dot off"></span>
        <span class="status-text">Stopped</span>
      {:else if service.health === "starting"}
        <span class="status-dot starting"></span>
        <span class="status-text">Starting…</span>
      {:else if service.health === "healthy" && service.status.kind === "runningTracked"}
        <span class="status-dot live"></span>
        <span class="status-text">Healthy</span>
      {:else if service.health === "healthy"}
        <span class="status-dot ok"></span>
        <span class="status-text">External · healthy</span>
      {:else if service.health === "degraded"}
        <span class="status-dot warn"></span>
        <span class="status-text">Degraded</span>
      {:else if service.status.kind === "runningTracked"}
        <span class="status-dot live"></span>
        <span class="status-text">Running</span>
      {:else}
        <span class="status-dot warn"></span>
        <span class="status-text">External</span>
      {/if}
    </div>
  </header>

  <div class="meta">
    {#if service.defaultPort != null}
      <span class="meta-item">
        <span class="meta-label">port</span>
        <span class="meta-value">{service.defaultPort}</span>
      </span>
    {/if}
    {#if pid != null}
      <span class="meta-item">
        <span class="meta-label">pid</span>
        <span class="meta-value">{pid}</span>
      </span>
    {/if}
    {#if !isOurs && service.status.kind === "runningExternal"}
      <span class="meta-note">Not started by Stackpilot — Stop will still work.</span>
    {/if}
  </div>

  {#if actionError}
    <p class="err">{actionError}</p>
  {/if}

  <footer>
    {#if !service.installed}
      <button class="btn-mini primary" onclick={gotoCatalog}>Install</button>
    {:else if service.status.kind === "stopped"}
      <button class="btn-mini primary" onclick={() => call("start")} disabled={busy != null}>
        {busy === "start" ? "Starting…" : "Start"}
      </button>
      {#if service.persistDir}
        <button class="btn-mini" onclick={openData}>Open data folder</button>
      {/if}
      <button class="btn-mini ghost" onclick={() => (logsOpen = !logsOpen)}>
        {logsOpen ? "Hide logs" : "Logs"}
      </button>
    {:else}
      <button class="btn-mini" onclick={() => call("stop")} disabled={busy != null}>
        {busy === "stop" ? "Stopping…" : "Stop"}
      </button>
      <button class="btn-mini" onclick={() => call("restart")} disabled={busy != null || !isOurs}>
        {busy === "restart" ? "Restarting…" : "Restart"}
      </button>
      {#if service.persistDir}
        <button class="btn-mini ghost" onclick={openData}>Data folder</button>
      {/if}
      <button class="btn-mini ghost" onclick={() => (logsOpen = !logsOpen)}>
        {logsOpen ? "Hide logs" : "Logs"}
      </button>
    {/if}
  </footer>

  {#if logsOpen}
    <section class="log-panel">
      <div class="log-head">
        <span class="log-title">Logs</span>
        <span class="log-stat">
          {#if logs}
            {fmtSize(logs.sizeBytes)} · {logs.lines.length} lines
          {:else}
            …
          {/if}
        </span>
      </div>
      <div class="log-body" bind:this={logEl}>
        {#if logsErr}
          <div class="log-err">{logsErr}</div>
        {:else if logs && logs.lines.length === 0}
          <div class="log-empty">
            {service.status.kind === "stopped"
              ? "No logs yet. Start the service to capture output."
              : "Service is running but hasn't written anything yet."}
          </div>
        {:else if logs}
          {#each logs.lines as line, i (i)}
            <div class="log-line">{line || " "}</div>
          {/each}
        {/if}
      </div>
    </section>
  {/if}
</article>

<style>
  .card {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 16px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition: border-color 120ms ease;
  }

  .card.running {
    border-color: rgba(52, 211, 153, 0.25);
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }

  .title-block {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  h3 {
    font-size: 15px;
    font-weight: 600;
    margin: 0;
    letter-spacing: -0.01em;
  }

  .cat {
    font-size: 11px;
    color: var(--text-muted);
    letter-spacing: 0.02em;
  }

  .status-block {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    flex-shrink: 0;
  }

  .status-dot.live {
    background: var(--success);
    box-shadow: 0 0 0 3px var(--success-soft);
  }

  .status-dot.ok {
    background: var(--success);
  }

  .status-dot.warn {
    background: var(--warning);
    box-shadow: 0 0 0 3px var(--warning-soft);
  }

  .status-dot.off {
    background: var(--text-muted);
  }

  .status-dot.starting {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    animation: pulse-starting 1.2s ease-in-out infinite;
  }

  @keyframes pulse-starting {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.55; }
  }

  .status-text {
    font-size: 12px;
    color: var(--text-dim);
    font-weight: 500;
  }

  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 14px;
    align-items: center;
    font-size: 11.5px;
  }

  .meta-item {
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }

  .meta-label {
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    font-size: 10px;
  }

  .meta-value {
    color: var(--text);
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }

  .meta-note {
    color: var(--text-muted);
    font-size: 11px;
    font-style: italic;
  }

  .err {
    margin: 0;
    padding: 6px 10px;
    background: var(--danger-soft);
    color: var(--danger);
    border-radius: var(--radius-sm);
    font-size: 11.5px;
  }

  footer {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .btn-mini {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
    cursor: pointer;
    transition: all 120ms ease;
  }

  .btn-mini:hover:not(:disabled) {
    background: var(--bg-3);
    border-color: var(--border-strong);
  }

  .btn-mini:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .btn-mini.primary {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--bg-0);
    font-weight: 600;
  }

  .btn-mini.primary:hover:not(:disabled) {
    background: var(--accent-hover);
    border-color: var(--accent-hover);
  }

  .btn-mini.ghost {
    background: transparent;
    border-color: transparent;
    color: var(--text-dim);
  }

  .btn-mini.ghost:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .log-panel {
    background: #07080b;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    margin-top: 4px;
    overflow: hidden;
  }

  .log-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 10px;
    border-bottom: 1px solid var(--border);
    background: var(--bg-2);
  }

  .log-title {
    font-size: 11px;
    color: var(--text-dim);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .log-stat {
    font-size: 10.5px;
    color: var(--text-muted);
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }

  .log-body {
    max-height: 240px;
    overflow-y: auto;
    padding: 8px 12px;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 11.5px;
    line-height: 1.55;
    color: var(--text-dim);
  }

  .log-line {
    white-space: pre-wrap;
    word-break: break-word;
  }

  .log-empty,
  .log-err {
    color: var(--text-muted);
    font-style: italic;
    font-family: inherit;
  }

  .log-err {
    color: var(--danger);
  }
</style>
