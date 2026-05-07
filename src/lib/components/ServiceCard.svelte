<script lang="ts">
  import { goto } from "$app/navigation";
  import { ipc } from "$lib/ipc";
  import type { ServiceInfo } from "$lib/types";
  import ConfigEditor from "./ConfigEditor.svelte";
  import ContextMenu, { type ContextMenuItem } from "./ContextMenu.svelte";

  let {
    service,
    onChanged,
  }: { service: ServiceInfo; onChanged: (next: ServiceInfo) => void } = $props();

  let busy = $state<"start" | "stop" | "restart" | null>(null);
  let actionError = $state<string | null>(null);

  let configOpen = $state(false);
  let menu = $state<{ x: number; y: number } | null>(null);

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
    goto(`/packages?q=${encodeURIComponent(service.scoopApp)}`);
  }

  function gotoLogs() {
    goto(`/logs?service=${encodeURIComponent(service.key)}`);
  }

  function openMenu(e: MouseEvent) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY };
  }

  const menuItems: ContextMenuItem[] = $derived.by(() => {
    if (!service.installed) {
      return [
        { kind: "item", label: "Install via Scoop", action: gotoCatalog },
      ] satisfies ContextMenuItem[];
    }
    const stopped = service.status.kind === "stopped";
    return [
      stopped
        ? {
            kind: "item",
            label: "Start",
            action: () => call("start"),
            disabled: busy !== null,
          }
        : {
            kind: "item",
            label: "Stop",
            action: () => call("stop"),
            danger: true,
            disabled: busy !== null,
          },
      {
        kind: "item",
        label: "Restart",
        action: () => call("restart"),
        disabled: busy !== null || stopped || !isOurs,
      },
      { kind: "divider" },
      { kind: "item", label: "Open folder", action: openData },
      { kind: "item", label: "Edit configs…", action: () => (configOpen = true) },
      { kind: "item", label: "View logs", action: gotoLogs },
    ] satisfies ContextMenuItem[];
  });
</script>

<article class="card" class:running={isRunning} oncontextmenu={openMenu}>
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
        <span class="status-text">Running</span>
      {:else if service.health === "healthy"}
        <span class="status-dot ok"></span>
        <span class="status-text">External</span>
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
    <div class="err-block">
      <div class="err-head">
        <strong>Couldn't start.</strong>
        <button
          class="err-close"
          onclick={() => (actionError = null)}
          aria-label="Dismiss"
        >×</button>
      </div>
      <pre>{actionError}</pre>
      <div class="err-actions">
        <button class="err-link" onclick={gotoLogs}>Open full log →</button>
      </div>
    </div>
  {/if}

  <footer>
    {#if !service.installed}
      <button class="btn-mini primary" onclick={gotoCatalog}>Install</button>
    {:else if service.status.kind === "stopped"}
      <button class="btn-mini primary" onclick={() => call("start")} disabled={busy != null}>
        {busy === "start" ? "Starting…" : "Start"}
      </button>
      <button class="btn-mini ghost" onclick={openData}>Folder</button>
      <button class="btn-mini ghost" onclick={() => (configOpen = true)}>Configs</button>
      <button class="btn-mini ghost" onclick={gotoLogs}>Logs</button>
    {:else}
      <button class="btn-mini danger" onclick={() => call("stop")} disabled={busy != null}>
        {busy === "stop" ? "Stopping…" : "Stop"}
      </button>
      <button class="btn-mini warn" onclick={() => call("restart")} disabled={busy != null || !isOurs}>
        {busy === "restart" ? "Restarting…" : "Restart"}
      </button>
      <button class="btn-mini ghost" onclick={openData}>Folder</button>
      <button class="btn-mini ghost" onclick={() => (configOpen = true)}>Configs</button>
      <button class="btn-mini ghost" onclick={gotoLogs}>Logs</button>
    {/if}
  </footer>

  {#if configOpen}
    <ConfigEditor
      {service}
      onClose={() => (configOpen = false)}
      onSaved={(next) => onChanged(next)}
    />
  {/if}

  {#if menu}
    <ContextMenu
      x={menu.x}
      y={menu.y}
      items={menuItems}
      onClose={() => (menu = null)}
    />
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

  .err-block {
    background: var(--danger-soft);
    border: 1px solid rgba(248, 113, 113, 0.25);
    border-radius: var(--radius-sm);
    padding: 8px 10px 6px 10px;
    font-size: 11.5px;
    color: var(--danger);
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .err-head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 8px;
  }

  .err-close {
    background: transparent;
    border: none;
    color: var(--danger);
    font-size: 16px;
    line-height: 1;
    width: 18px;
    height: 18px;
    border-radius: 4px;
    padding: 0;
    cursor: pointer;
    opacity: 0.7;
  }

  .err-close:hover {
    opacity: 1;
    background: rgba(248, 113, 113, 0.15);
  }

  .err-block pre {
    margin: 0;
    padding: 6px 8px;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 4px;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 11px;
    line-height: 1.5;
    color: var(--text-dim);
    white-space: pre-wrap;
    word-break: break-word;
    max-height: 160px;
    overflow-y: auto;
  }

  .err-actions {
    display: flex;
    justify-content: flex-end;
  }

  .err-link {
    background: transparent;
    border: none;
    color: var(--danger);
    font-size: 11px;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
    text-underline-offset: 2px;
  }

  .err-link:hover {
    color: var(--text);
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

  .btn-mini.danger {
    background: var(--danger-soft);
    border-color: transparent;
    color: var(--danger);
  }

  .btn-mini.danger:hover:not(:disabled) {
    background: var(--danger);
    color: var(--bg-0);
    border-color: var(--danger);
  }

  .btn-mini.warn {
    background: var(--warning-soft);
    border-color: transparent;
    color: var(--warning);
  }

  .btn-mini.warn:hover:not(:disabled) {
    background: var(--warning);
    color: var(--bg-0);
    border-color: var(--warning);
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

</style>
