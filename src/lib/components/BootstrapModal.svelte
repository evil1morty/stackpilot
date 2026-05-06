<script lang="ts">
  import { goto } from "$app/navigation";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { ipc } from "$lib/ipc";
  import { operation } from "$lib/stores/operation.svelte";
  import type { ScoopStatus } from "$lib/types";

  let { onResolved }: { onResolved: (s: ScoopStatus) => void } = $props();

  let checking = $state(false);
  let bootstrapping = $state(false);

  async function openScoopSite() {
    await openUrl("https://scoop.sh");
  }

  async function recheck() {
    checking = true;
    try {
      const status = await ipc.scoopCheck();
      onResolved(status);
    } finally {
      checking = false;
    }
  }

  async function bootstrap() {
    if (bootstrapping) return;
    bootstrapping = true;
    goto("/logs");
    try {
      await operation.runBootstrap();
      const status = await ipc.scoopCheck();
      onResolved(status);
    } finally {
      bootstrapping = false;
    }
  }
</script>

<div class="overlay">
  <div class="modal">
    <div class="icon">
      <svg viewBox="0 0 24 24" width="32" height="32" fill="none" stroke="currentColor"
        stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
        <path d="M21 15a2 2 0 0 1-2 2H7l-4 4V5a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2z" />
        <line x1="12" y1="8" x2="12" y2="12" />
        <line x1="12" y1="16" x2="12.01" y2="16" />
      </svg>
    </div>

    <h2>Scoop is required</h2>
    <p>
      Stackpilot uses <strong>Scoop</strong> to install and manage Windows
      services. We didn't find a Scoop install at <code>%USERPROFILE%\scoop</code>.
    </p>

    <div class="install-block">
      <p class="caption">One-click installer (runs the official script):</p>
      <button class="btn btn-primary big" onclick={bootstrap} disabled={bootstrapping}>
        {bootstrapping ? "Installing Scoop…" : "Install Scoop now"}
      </button>
      <p class="hint subtle">
        Runs <code>irm get.scoop.sh | iex</code> in PowerShell. Output streams to the
        Logs view.
      </p>
    </div>

    <details>
      <summary>Or install manually</summary>
      <pre>Set-ExecutionPolicy -ExecutionPolicy RemoteSigned -Scope CurrentUser
irm get.scoop.sh | iex</pre>
      <div class="actions">
        <button class="btn" onclick={openScoopSite}>Open scoop.sh</button>
        <button class="btn" onclick={recheck} disabled={checking}>
          {checking ? "Checking…" : "Recheck"}
        </button>
      </div>
    </details>
  </div>
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.55);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .modal {
    background: var(--bg-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    padding: 28px 32px;
    max-width: 480px;
    width: 90%;
    box-shadow: var(--shadow-lg);
  }

  .icon {
    width: 56px;
    height: 56px;
    border-radius: 14px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--accent-soft);
    color: var(--accent);
    margin-bottom: 16px;
  }

  h2 {
    font-size: 18px;
    font-weight: 600;
    margin: 0 0 8px 0;
    letter-spacing: -0.01em;
  }

  p {
    font-size: 13px;
    color: var(--text-dim);
    line-height: 1.5;
    margin: 0 0 16px 0;
  }

  code {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    background: var(--bg-2);
    padding: 1px 6px;
    border-radius: 4px;
    color: var(--text);
  }

  .install-block {
    margin: 16px 0 12px 0;
  }

  .caption {
    font-size: 12px;
    margin-bottom: 10px;
    color: var(--text-muted);
  }

  .big {
    width: 100%;
    padding: 10px 16px;
    font-size: 13px;
    justify-content: center;
  }

  .hint.subtle {
    margin: 10px 0 0 0;
    font-size: 11.5px;
    color: var(--text-muted);
    text-align: center;
  }

  details {
    margin-top: 8px;
    border-top: 1px solid var(--border);
    padding-top: 12px;
  }

  summary {
    font-size: 12px;
    color: var(--text-dim);
    cursor: pointer;
    user-select: none;
    padding: 4px 0;
  }

  summary:hover {
    color: var(--text);
  }

  pre {
    background: var(--bg-0);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 10px 12px;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 11.5px;
    color: var(--text);
    overflow-x: auto;
    margin: 10px 0;
    line-height: 1.6;
  }

  .actions {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    margin-top: 8px;
  }
</style>
