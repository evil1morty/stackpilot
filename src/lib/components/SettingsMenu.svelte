<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { ipc } from "$lib/ipc";
  import { theme, type ThemePref } from "$lib/stores/theme.svelte";

  const CLOSE_TO_TRAY_KEY = "stackpilot.closeToTray";

  let open = $state(false);
  let aboutOpen = $state(false);
  let appVersion = $state<string>("");
  let closeToTray = $state(true);

  onMount(async () => {
    const stored = localStorage.getItem(CLOSE_TO_TRAY_KEY);
    if (stored != null) {
      closeToTray = stored === "true";
    }
    // Mirror to Rust so the close handler reads the same value.
    try {
      await ipc.setCloseToTray(closeToTray);
    } catch {
      // pass — IPC might not be ready yet during first render
    }
  });

  $effect(() => {
    if (aboutOpen && !appVersion) {
      getVersion().then((v) => (appVersion = v));
    }
  });

  function close(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest(".settings-root")) open = false;
  }

  $effect(() => {
    if (open) {
      window.addEventListener("click", close);
      return () => window.removeEventListener("click", close);
    }
  });

  function setTheme(p: ThemePref) {
    theme.set(p);
  }

  async function toggleCloseToTray() {
    closeToTray = !closeToTray;
    localStorage.setItem(CLOSE_TO_TRAY_KEY, String(closeToTray));
    try {
      await ipc.setCloseToTray(closeToTray);
    } catch {
      // ignore — best-effort
    }
  }

  async function quit() {
    try {
      await ipc.quitApp();
    } catch {
      window.close();
    }
  }
</script>

<div class="settings-root">
  <button
    class="trigger"
    class:open
    aria-label="Settings"
    onclick={(e) => {
      e.stopPropagation();
      open = !open;
    }}
  >
    <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
      stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <circle cx="12" cy="12" r="3" />
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-2 2 2 2 0 0 1-2-2v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83 0 2 2 0 0 1 0-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1-2-2 2 2 0 0 1 2-2h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 0-2.83 2 2 0 0 1 2.83 0l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 2-2 2 2 0 0 1 2 2v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 0 2 2 0 0 1 0 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 2 2 2 2 0 0 1-2 2h-.09a1.65 1.65 0 0 0-1.51 1z" />
    </svg>
  </button>

  {#if open}
    <div class="popover">
      <div class="section">
        <span class="section-label">Theme</span>
        <div class="seg">
          {#each ["dark", "light", "system"] as p (p)}
            <button
              class="seg-btn"
              class:active={theme.pref === p}
              onclick={() => setTheme(p as ThemePref)}
            >
              {p}
            </button>
          {/each}
        </div>
      </div>

      <div class="divider"></div>

      <button class="row toggle" onclick={toggleCloseToTray}>
        <span>Close to tray</span>
        <span class="check" class:on={closeToTray} aria-hidden="true">
          {#if closeToTray}✓{/if}
        </span>
      </button>

      <div class="divider"></div>

      <button class="row" onclick={() => { aboutOpen = true; open = false; }}>
        About Stackpilot
      </button>
      <button class="row" onclick={() => openUrl("https://scoop.sh")}>
        Scoop docs
      </button>

      <div class="divider"></div>

      <button class="row danger" onclick={quit}>Quit Stackpilot</button>
    </div>
  {/if}
</div>

{#if aboutOpen}
  <div
    class="overlay"
    role="presentation"
    onclick={() => (aboutOpen = false)}
    onkeydown={(e) => {
      if (e.key === "Escape") aboutOpen = false;
    }}
  >
    <div
      class="about"
      role="dialog"
      aria-modal="true"
      aria-label="About Stackpilot"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.stopPropagation()}
    >
      <div class="logo-mark"></div>
      <h2>Stackpilot</h2>
      <p class="ver">v{appVersion || "?"}</p>
      <p class="tag">A modern Windows GUI for managing dev services with Scoop.</p>

      <div class="links">
        <button class="btn" onclick={() => openUrl("https://github.com")}>Source</button>
        <button class="btn btn-primary" onclick={() => (aboutOpen = false)}>Close</button>
      </div>
    </div>
  </div>
{/if}

<style>
  .settings-root {
    position: relative;
  }

  .trigger {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    border: 1px solid transparent;
    background: transparent;
    color: var(--text-dim);
    cursor: pointer;
    transition: all 120ms ease;
  }

  .trigger:hover,
  .trigger.open {
    background: var(--bg-2);
    color: var(--text);
  }

  .popover {
    position: absolute;
    bottom: calc(100% + 6px);
    left: 0;
    right: 0;
    background: var(--bg-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 6px;
    box-shadow: var(--shadow-lg);
    z-index: 50;
    min-width: 180px;
  }

  .section {
    padding: 6px 8px;
  }

  .section-label {
    font-size: 10.5px;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
    display: block;
    margin-bottom: 6px;
  }

  .seg {
    display: flex;
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 2px;
    gap: 2px;
  }

  .seg-btn {
    flex: 1;
    background: transparent;
    border: none;
    color: var(--text-dim);
    font-size: 11px;
    padding: 4px 6px;
    border-radius: 4px;
    cursor: pointer;
    text-transform: capitalize;
    transition: all 120ms ease;
  }

  .seg-btn:hover {
    color: var(--text);
  }

  .seg-btn.active {
    background: var(--accent);
    color: var(--bg-0);
    font-weight: 600;
  }

  .divider {
    height: 1px;
    background: var(--border);
    margin: 4px 0;
  }

  .row {
    width: 100%;
    text-align: left;
    background: transparent;
    border: none;
    color: var(--text-dim);
    padding: 7px 10px;
    border-radius: var(--radius-sm);
    font-size: 12.5px;
    cursor: pointer;
    transition: background 120ms ease, color 120ms ease;
  }

  .row:hover {
    background: var(--bg-3);
    color: var(--text);
  }

  .row.toggle {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .check {
    width: 16px;
    height: 16px;
    border-radius: 4px;
    border: 1px solid var(--border-strong);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: var(--bg-0);
    font-size: 11px;
    font-weight: 700;
  }

  .check.on {
    background: var(--accent);
    border-color: var(--accent);
  }

  .row.danger {
    color: var(--danger);
  }

  .row.danger:hover {
    background: var(--danger-soft);
    color: var(--danger);
  }

  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(4px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 100;
  }

  .about {
    background: var(--bg-1);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius-lg);
    padding: 32px;
    max-width: 360px;
    width: 90%;
    text-align: center;
    box-shadow: var(--shadow-lg);
  }

  .logo-mark {
    width: 48px;
    height: 48px;
    margin: 0 auto 16px auto;
    border-radius: 12px;
    background: linear-gradient(135deg, var(--accent), #8a64ff);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.05) inset, 0 8px 24px rgba(107, 140, 255, 0.35);
    position: relative;
  }

  .logo-mark::after {
    content: "";
    position: absolute;
    inset: 11px;
    border: 2px solid rgba(255, 255, 255, 0.85);
    border-radius: 5px;
    border-bottom: none;
    border-right: none;
    transform: rotate(-45deg);
  }

  .about h2 {
    margin: 0 0 4px 0;
    font-size: 18px;
    font-weight: 600;
  }

  .ver {
    margin: 0 0 14px 0;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    color: var(--text-muted);
  }

  .tag {
    margin: 0 0 24px 0;
    color: var(--text-dim);
    font-size: 13px;
    line-height: 1.5;
  }

  .links {
    display: flex;
    gap: 8px;
    justify-content: center;
  }
</style>
