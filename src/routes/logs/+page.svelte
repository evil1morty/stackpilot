<script lang="ts">
  import { tick } from "svelte";
  import { operation } from "$lib/stores/operation.svelte";

  let logEl: HTMLDivElement | undefined = $state();
  let pinToBottom = $state(true);

  $effect(() => {
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

  const elapsed = $derived.by(() => {
    const c = operation.current;
    if (!c) return null;
    const end = c.endedAt ?? Date.now();
    return fmtElapsed(end - c.startedAt);
  });
</script>

<section class="page">
  <header class="head">
    <div>
      <h1>Logs</h1>
      <p class="lede">
        {#if operation.current}
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
        {:else}
          Streaming output from installs and services.
        {/if}
      </p>
    </div>

    {#if operation.current}
      <div class="head-actions">
        {#if operation.busy}
          <button class="btn danger" onclick={() => operation.cancel()}>Cancel</button>
        {:else}
          <button class="btn" onclick={() => operation.clear()}>Clear</button>
        {/if}
      </div>
    {/if}
  </header>

  {#if operation.current}
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
          <span class="text">{line.text || " "}</span>
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
      <p class="hint">Install or uninstall an app from the Catalog to see output here.</p>
    </div>
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
    margin-bottom: 16px;
    flex-shrink: 0;
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
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .dot-sep {
    color: var(--text-muted);
    margin: 0 2px;
  }

  .elapsed {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
  }

  .status-dot {
    width: 8px;
    height: 8px;
    border-radius: 999px;
    display: inline-block;
    margin-right: 4px;
  }

  .status-dot.live {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
    animation: pulse 1.4s ease-in-out infinite;
  }

  .status-dot.ok {
    background: var(--success);
  }
  .status-dot.err {
    background: var(--danger);
  }
  .status-dot.warn {
    background: var(--warning);
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.55; }
  }

  .head-actions {
    display: flex;
    gap: 8px;
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
    border-radius: 0 0 var(--radius-sm) var(--radius-sm);
    padding: 12px 16px;
    overflow-y: auto;
    flex: 1;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.6;
    color: var(--text);
  }

  .line {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 0;
  }

  .prefix {
    width: 12px;
    color: var(--text-muted);
    flex-shrink: 0;
    user-select: none;
  }

  .line.stderr .prefix {
    color: var(--warning);
  }

  .line.stderr .text {
    color: #fbd38d;
  }

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
    padding: 80px 0;
    color: var(--text-muted);
  }

  .empty p {
    margin: 0 0 4px 0;
  }

  .empty .hint {
    font-size: 12px;
    color: var(--text-muted);
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
