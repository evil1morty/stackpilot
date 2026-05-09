<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { page } from "$app/state";
  import { ipc } from "$lib/ipc";
  import { operation } from "$lib/stores/operation.svelte";
  import type { ServiceInfo, ServiceLog } from "$lib/types";
  import AnsiText from "$lib/components/AnsiText.svelte";

  type Source = { kind: "operation" } | { kind: "service"; key: string };

  let source = $state<Source>({ kind: "operation" });
  let services = $state<ServiceInfo[]>([]);
  let serviceLog = $state<ServiceLog | null>(null);
  let serviceErr = $state<string | null>(null);

  let logEl: HTMLDivElement | undefined = $state();
  let pinToBottom = $state(true);
  let pollHandle: ReturnType<typeof setInterval> | null = null;
  let visibilityHandler: (() => void) | null = null;

  // Initial: respect ?service=<key> query, else stay on Operations.
  onMount(async () => {
    const q = page.url.searchParams.get("service");
    try {
      services = await ipc.servicesList();
    } catch {
      // pass — leave list empty, picker still shows Operations
    }
    if (q && services.some((s) => s.key === q)) {
      source = { kind: "service", key: q };
    }
  });

  onDestroy(() => {
    if (pollHandle) clearInterval(pollHandle);
    if (visibilityHandler) {
      document.removeEventListener("visibilitychange", visibilityHandler);
      visibilityHandler = null;
    }
  });

  // Poll when a service source is selected; clear poll on switch. Pauses
  // when the document is hidden (window minimised to tray) to spare the
  // CPU + IPC + filesystem from work no one's looking at.
  $effect(() => {
    if (pollHandle) {
      clearInterval(pollHandle);
      pollHandle = null;
    }
    if (visibilityHandler) {
      document.removeEventListener("visibilitychange", visibilityHandler);
      visibilityHandler = null;
    }
    if (source.kind !== "service") {
      serviceLog = null;
      return;
    }
    const key = source.key;
    let cancelled = false;

    async function refresh() {
      if (cancelled) return;
      try {
        const sinceSize = serviceLog?.sizeBytes;
        const next = await ipc.servicesTailLog(key, 500, sinceSize);
        // The user may have switched sources while we were awaiting; if the
        // effect was torn down, drop the response on the floor instead of
        // clobbering the new source's state.
        if (cancelled) return;
        // Backend returns lines:[] + matching size when nothing changed.
        if (sinceSize != null && next.sizeBytes === sinceSize && next.lines.length === 0) {
          return;
        }
        serviceLog = next;
        serviceErr = null;
        await tick();
        if (cancelled) return;
        if (logEl && pinToBottom) logEl.scrollTop = logEl.scrollHeight;
      } catch (e) {
        if (cancelled) return;
        serviceErr = e instanceof Error ? e.message : String(e);
      }
    }

    function startTimer() {
      if (pollHandle != null) return;
      pollHandle = setInterval(refresh, 1500);
    }
    function stopTimer() {
      if (pollHandle != null) {
        clearInterval(pollHandle);
        pollHandle = null;
      }
    }

    visibilityHandler = () => {
      if (document.visibilityState === "visible") {
        refresh();
        startTimer();
      } else {
        stopTimer();
      }
    };
    document.addEventListener("visibilitychange", visibilityHandler);

    refresh();
    if (document.visibilityState === "visible") startTimer();

    return () => {
      cancelled = true;
      stopTimer();
      if (visibilityHandler) {
        document.removeEventListener("visibilitychange", visibilityHandler);
        visibilityHandler = null;
      }
    };
  });

  // Re-pin to bottom when the operation log appends.
  $effect(() => {
    if (source.kind !== "operation") return;
    if (!operation.current) return;
    void operation.current.lines.length;
    if (pinToBottom) tick().then(() => logEl?.scrollTo({ top: logEl.scrollHeight }));
  });

  function onScroll() {
    if (!logEl) return;
    const dist = logEl.scrollHeight - logEl.scrollTop - logEl.clientHeight;
    pinToBottom = dist < 32;
  }

  function fmtElapsed(ms: number): string {
    const s = Math.round(ms / 1000);
    if (s < 60) return `${s}s`;
    const m = Math.floor(s / 60);
    const r = s % 60;
    return `${m}m ${r}s`;
  }

  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }

  const elapsed = $derived.by(() => {
    const c = operation.current;
    if (!c) return null;
    const end = c.endedAt ?? Date.now();
    return fmtElapsed(end - c.startedAt);
  });

  const installedServices = $derived(services.filter((s) => s.installed));
  const activeServiceKey = $derived(source.kind === "service" ? source.key : null);
  const activeService = $derived(
    activeServiceKey != null
      ? services.find((s) => s.key === activeServiceKey) ?? null
      : null,
  );

  async function openServiceLogFile() {
    if (!serviceLog?.path) return;
    try {
      await ipc.servicesOpenPath(serviceLog.path);
    } catch (e) {
      serviceErr = e instanceof Error ? e.message : String(e);
    }
  }
</script>

<section class="page">
  <header class="head">
    <div class="head-left">
      <h1>Logs</h1>
      <div class="picker">
        <button
          class="src"
          class:active={source.kind === "operation"}
          onclick={() => (source = { kind: "operation" })}
        >
          Operations
          {#if operation.busy}<span class="src-dot live"></span>{/if}
        </button>
        {#each installedServices as svc (svc.key)}
          <button
            class="src"
            class:active={activeServiceKey === svc.key}
            onclick={() => (source = { kind: "service", key: svc.key })}
          >
            {svc.display}
            {#if svc.status.kind === "runningTracked"}
              <span class="src-dot ok"></span>
            {:else if svc.status.kind === "runningExternal"}
              <span class="src-dot warn"></span>
            {/if}
          </button>
        {/each}
      </div>
    </div>

    <div class="head-actions">
      {#if source.kind === "operation" && operation.current}
        {#if operation.busy}
          <button class="btn danger" onclick={() => operation.cancel()}>Cancel</button>
        {:else}
          <button class="btn" onclick={() => operation.clear()}>Clear</button>
        {/if}
      {:else if source.kind === "service" && serviceLog && serviceLog.sizeBytes > 0}
        <button class="btn" onclick={openServiceLogFile} title="Open in default editor">
          ↗ Open in editor
        </button>
      {/if}
    </div>
  </header>

  {#if source.kind === "operation"}
    {#if operation.current}
      <p class="status-line">
        {#if operation.current.state === "running"}
          <span class="status-dot live"></span> Running
        {:else if operation.current.state === "finished"}
          <span class="status-dot ok"></span> Finished · exit {operation.current.exitCode}
        {:else if operation.current.state === "errored"}
          <span class="status-dot err"></span> Errored · exit {operation.current.exitCode ?? "?"}
        {:else if operation.current.state === "cancelled"}
          <span class="status-dot warn"></span> Cancelled
        {/if}
        {#if elapsed}
          <span class="dot-sep">·</span>
          <span class="elapsed">{elapsed}</span>
        {/if}
        <span class="dot-sep">·</span>
        {operation.current.lines.length} lines
      </p>

      <div class="cmdline">
        <span class="prompt">PS&gt;</span>
        <code>{operation.current.command || `scoop ${operation.current.kind} ${operation.current.target}`}</code>
      </div>

      <div class="terminal" bind:this={logEl} onscroll={onScroll}>
        {#if operation.current.lines.length === 0}
          <div class="placeholder">Awaiting first line of output…</div>
        {/if}
        {#each operation.current.lines as line, i (i)}
          <div class="line {line.kind}">
            <span class="prefix">
              {#if line.kind === "stderr"}!{:else if line.kind === "system"}#{:else}&gt;{/if}
            </span>
            <span class="text"><AnsiText text={line.text} /></span>
          </div>
        {/each}
      </div>

      {#if !pinToBottom && operation.busy}
        <button
          class="jump-bottom"
          onclick={() => {
            pinToBottom = true;
            logEl?.scrollTo({ top: logEl.scrollHeight });
          }}
        >
          ↓ Jump to live
        </button>
      {/if}
    {:else}
      <div class="empty">
        <p>No operation running.</p>
        <p class="hint">
          Install a package or apply a preset to see streaming output here.
          Pick a service tab above to view its captured log.
        </p>
      </div>
    {/if}
  {:else if source.kind === "service"}
    <p class="status-line">
      {#if activeService?.status.kind === "runningTracked"}
        <span class="status-dot live"></span> Running
      {:else if activeService?.status.kind === "runningExternal"}
        <span class="status-dot warn"></span> External
      {:else}
        <span class="status-dot off"></span> Stopped
      {/if}
      {#if serviceLog}
        <span class="dot-sep">·</span>
        {fmtSize(serviceLog.sizeBytes)}
        <span class="dot-sep">·</span>
        {serviceLog.lines.length} lines
        <span class="dot-sep">·</span>
        <code class="path-inline">{serviceLog.path}</code>
      {/if}
    </p>

    {#if serviceErr}
      <div class="terminal err-block">{serviceErr}</div>
    {:else if serviceLog && serviceLog.lines.length === 0}
      <div class="terminal" bind:this={logEl}>
        <div class="placeholder">
          {activeService?.status.kind === "stopped"
            ? "No logs yet. Start the service to capture output."
            : "Service is running but hasn't written anything yet."}
        </div>
      </div>
    {:else if serviceLog}
      <div class="terminal" bind:this={logEl} onscroll={onScroll}>
        {#each serviceLog.lines as line, i (i)}
          <div class="line stdout">
            <span class="prefix">&gt;</span>
            <span class="text"><AnsiText text={line} /></span>
          </div>
        {/each}
      </div>
      {#if !pinToBottom}
        <button
          class="jump-bottom"
          onclick={() => {
            pinToBottom = true;
            logEl?.scrollTo({ top: logEl.scrollHeight });
          }}
        >
          ↓ Jump to live
        </button>
      {/if}
    {:else}
      <div class="empty">
        <p>Loading…</p>
      </div>
    {/if}
  {/if}
</section>

<style>
  .page {
    padding: 28px 36px 60px 36px;
    max-width: 1400px;
    height: 100vh;
    display: flex;
    flex-direction: column;
  }

  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    margin-bottom: 12px;
    flex-shrink: 0;
    gap: 16px;
  }

  .head-left {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }

  h1 {
    font-size: 24px;
    font-weight: 600;
    margin: 0;
    letter-spacing: -0.01em;
  }

  .picker {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .src {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    background: transparent;
    border: 1px solid var(--border);
    border-radius: 999px;
    padding: 4px 12px;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-dim);
    cursor: pointer;
    transition: all 120ms ease;
  }

  .src:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .src.active {
    background: var(--accent-soft);
    color: var(--accent);
    border-color: transparent;
  }

  .src-dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
  }

  .src-dot.live {
    background: var(--accent);
    animation: pulse 1.4s ease-in-out infinite;
  }
  .src-dot.ok { background: var(--success); }
  .src-dot.warn { background: var(--warning); }

  .status-line {
    margin: 0 0 10px 0;
    color: var(--text-dim);
    font-size: 12px;
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .dot-sep {
    color: var(--text-muted);
    margin: 0 2px;
  }

  .elapsed,
  .path-inline {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 11.5px;
    color: var(--text-muted);
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    display: inline-block;
  }

  .status-dot.live {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    animation: pulse 1.4s ease-in-out infinite;
  }
  .status-dot.ok { background: var(--success); }
  .status-dot.err { background: var(--danger); }
  .status-dot.warn { background: var(--warning); }
  .status-dot.off { background: var(--text-muted); }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.55; }
  }

  .head-actions {
    display: flex;
    gap: 8px;
    flex-shrink: 0;
  }

  .danger {
    background: var(--danger-soft);
    border-color: transparent;
    color: var(--danger);
  }

  .danger:hover {
    background: var(--danger);
    color: var(--bg-0);
  }

  .cmdline {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm) var(--radius-sm) 0 0;
    border-bottom: none;
    padding: 8px 14px;
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .prompt {
    color: var(--accent);
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    font-weight: 600;
  }

  .cmdline code {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12.5px;
    color: var(--text);
    background: transparent;
    padding: 0;
  }

  .terminal {
    background: #07080b;
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 12px 16px;
    overflow-y: auto;
    flex: 1;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.6;
    color: var(--text);
  }

  .cmdline + .terminal {
    border-top-left-radius: 0;
    border-top-right-radius: 0;
  }

  .err-block {
    color: var(--danger);
    font-style: italic;
  }

  .line {
    display: flex;
    align-items: flex-start;
    gap: 8px;
  }

  .prefix {
    width: 12px;
    color: var(--text-muted);
    flex-shrink: 0;
    user-select: none;
  }

  .line.stderr .prefix { color: var(--warning); }
  .line.stderr .text { color: #fbd38d; }
  .line.system .prefix,
  .line.system .text {
    color: var(--accent);
    font-style: italic;
  }

  .text {
    white-space: pre-wrap;
    word-break: break-word;
    flex: 1;
    min-width: 0;
  }

  .placeholder {
    color: var(--text-muted);
    font-style: italic;
  }

  .empty {
    text-align: center;
    padding: 80px 24px;
    color: var(--text-muted);
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-1);
  }

  .empty p {
    margin: 0 0 4px 0;
  }

  .empty .hint {
    font-size: 12px;
    max-width: 480px;
    margin: 8px auto 0 auto;
    line-height: 1.5;
  }

  .jump-bottom {
    position: fixed;
    bottom: 24px;
    right: 36px;
    background: var(--accent);
    color: var(--bg-0);
    border: none;
    border-radius: 999px;
    padding: 8px 16px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: var(--shadow-lg);
  }
</style>
