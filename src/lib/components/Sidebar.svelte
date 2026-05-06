<script lang="ts">
  import { page } from "$app/state";
  import SettingsMenu from "./SettingsMenu.svelte";

  type NavItem = { href: string; label: string; icon: string };

  const items: NavItem[] = [
    { href: "/", label: "Catalog", icon: "grid" },
    { href: "/services", label: "Services", icon: "activity" },
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
        <span class="dot" aria-hidden="true"></span>
        <span class="label">{item.label}</span>
      </a>
    {/each}
  </nav>

  <div class="foot">
    <span class="foot-text">Scoop · Windows</span>
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

  .dot {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: var(--text-muted);
    transition: background 120ms ease;
    flex-shrink: 0;
  }

  .nav-item:hover .dot {
    background: var(--text-dim);
  }

  .nav-item.active .dot {
    background: var(--accent);
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .foot {
    padding: 12px 10px 4px 10px;
    border-top: 1px solid var(--border);
    margin-top: 12px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .foot-text {
    font-size: 11px;
    color: var(--text-muted);
    letter-spacing: 0.02em;
  }
</style>
