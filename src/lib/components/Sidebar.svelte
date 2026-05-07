<script lang="ts">
  import { page } from "$app/state";
  import SettingsMenu from "./SettingsMenu.svelte";
  import { theme } from "$lib/stores/theme.svelte";

  type IconName = "grid" | "activity" | "layers" | "folder" | "terminal";

  function cycleTheme() {
    // Quick toggle between dark and light. "system" stays available
    // through the Settings menu segment for users who want it.
    theme.set(theme.resolved === "dark" ? "light" : "dark");
  }

  type NavItem = {
    href: string;
    label: string;
    icon: IconName;
  };

  const items: NavItem[] = [
    { href: "/", label: "Services", icon: "activity" },
    { href: "/packages", label: "Packages", icon: "grid" },
    { href: "/projects", label: "Projects", icon: "folder" },
    { href: "/presets", label: "Presets", icon: "layers" },
    { href: "/logs", label: "Logs", icon: "terminal" },
  ];

  function isActive(href: string): boolean {
    if (href === "/") return page.url.pathname === "/";
    return page.url.pathname === href || page.url.pathname.startsWith(href + "/");
  }
</script>

<aside>
  <div class="brand">
    <div class="logo-mark"></div>
    <div class="brand-text">
      <span class="brand-name">Stackpilot</span>
      <span class="brand-sub">v0.1</span>
    </div>
  </div>

  <nav>
    {#each items as item (item.href)}
      <a href={item.href} class="nav-item" class:active={isActive(item.href)}>
        <span class="icon" aria-hidden="true">
          {#if item.icon === "grid"}
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="3" width="7" height="7" rx="1.5" />
              <rect x="14" y="3" width="7" height="7" rx="1.5" />
              <rect x="3" y="14" width="7" height="7" rx="1.5" />
              <rect x="14" y="14" width="7" height="7" rx="1.5" />
            </svg>
          {:else if item.icon === "activity"}
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="22 12 18 12 15 21 9 3 6 12 2 12" />
            </svg>
          {:else if item.icon === "folder"}
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z" />
            </svg>
          {:else if item.icon === "layers"}
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polygon points="12 2 2 7 12 12 22 7 12 2" />
              <polyline points="2 17 12 22 22 17" />
              <polyline points="2 12 12 17 22 12" />
            </svg>
          {:else if item.icon === "terminal"}
            <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
              stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <polyline points="4 17 10 11 4 5" />
              <line x1="12" y1="19" x2="20" y2="19" />
            </svg>
          {/if}
        </span>
        <span class="label">{item.label}</span>
      </a>
    {/each}
  </nav>

  <div class="foot">
    <button
      class="theme-toggle"
      onclick={cycleTheme}
      aria-label="Toggle theme"
      title={theme.resolved === "dark" ? "Switch to light" : "Switch to dark"}
    >
      {#if theme.resolved === "dark"}
        <!-- sun icon: shown while in dark mode (click → go light) -->
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <circle cx="12" cy="12" r="4" />
          <path d="M12 2v2M12 20v2M4.93 4.93l1.41 1.41M17.66 17.66l1.41 1.41M2 12h2M20 12h2M4.93 19.07l1.41-1.41M17.66 6.34l1.41-1.41" />
        </svg>
      {:else}
        <!-- moon icon: shown while in light mode (click → go dark) -->
        <svg viewBox="0 0 24 24" width="14" height="14" fill="none" stroke="currentColor"
          stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
          <path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z" />
        </svg>
      {/if}
    </button>
    <SettingsMenu />
  </div>
</aside>

<style>
  aside {
    display: flex;
    flex-direction: column;
    background: var(--bg-1);
    border-right: 1px solid var(--border);
    padding: 16px 12px;
    height: 100vh;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 16px 8px;
    border-bottom: 1px solid var(--border);
    margin-bottom: 12px;
  }

  .logo-mark {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    background: linear-gradient(135deg, var(--accent), #8a64ff);
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.05) inset, 0 6px 16px rgba(107, 140, 255, 0.3);
    position: relative;
  }

  .logo-mark::after {
    content: "";
    position: absolute;
    inset: 6px;
    border: 1.5px solid rgba(255, 255, 255, 0.85);
    border-radius: 4px;
    border-bottom: none;
    border-right: none;
    transform: rotate(-45deg);
  }

  .brand-text {
    display: flex;
    flex-direction: column;
    line-height: 1.2;
  }

  .brand-name {
    font-weight: 600;
    font-size: 14px;
    letter-spacing: -0.01em;
  }

  .brand-sub {
    font-size: 11px;
    color: var(--text-muted);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex: 1;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    border-radius: var(--radius-sm);
    color: var(--text-dim);
    text-decoration: none;
    font-size: 13px;
    font-weight: 500;
    transition: background 120ms ease, color 120ms ease;
    position: relative;
  }

  .nav-item:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .nav-item.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .icon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 16px;
    height: 16px;
    color: var(--text-muted);
    transition: color 120ms ease;
    flex-shrink: 0;
  }

  .nav-item:hover .icon {
    color: var(--text-dim);
  }

  .nav-item.active .icon {
    color: var(--accent);
  }

  .label {
    flex: 1;
  }

  .foot {
    padding: 8px 6px 0 6px;
    margin-top: 8px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
  }

  .theme-toggle {
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

  .theme-toggle:hover {
    background: var(--bg-2);
    color: var(--text);
  }
</style>
