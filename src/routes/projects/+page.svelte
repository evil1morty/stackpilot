<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import type { ProjectInfo } from "$lib/types";
  import ProjectEditor from "$lib/components/ProjectEditor.svelte";
  import ContextMenu, { type ContextMenuItem } from "$lib/components/ContextMenu.svelte";

  let projects = $state<ProjectInfo[]>([]);
  let loading = $state(true);
  let error = $state<string | null>(null);
  let editing = $state<ProjectInfo | null | undefined>(undefined);
  let activating = $state<string | null>(null);
  let activationToast = $state<string | null>(null);
  let menu = $state<{ x: number; y: number; project: ProjectInfo } | null>(null);

  function openMenu(e: MouseEvent, p: ProjectInfo) {
    e.preventDefault();
    menu = { x: e.clientX, y: e.clientY, project: p };
  }

  function buildMenuItems(p: ProjectInfo): ContextMenuItem[] {
    return [
      p.isActive
        ? {
            kind: "item",
            label: "Deactivate",
            action: () => deactivate(p),
            disabled: activating !== null,
          }
        : {
            kind: "item",
            label: "Activate",
            action: () => activate(p),
            disabled: activating !== null || p.services.length === 0,
          },
      {
        kind: "item",
        label: "Open terminal",
        action: () => openTerminal(p),
        disabled: !p.rootDir,
      },
      { kind: "divider" },
      { kind: "item", label: "Edit project", action: () => (editing = p) },
      { kind: "divider" },
      { kind: "item", label: "Delete", danger: true, action: () => remove(p) },
    ];
  }

  async function openTerminal(p: ProjectInfo) {
    try {
      await ipc.projectsOpenTerminal(p.key);
    } catch (e) {
      activationToast = `Error: ${e instanceof Error ? e.message : String(e)}`;
      setTimeout(() => (activationToast = null), 4000);
    }
  }

  onMount(async () => {
    await refresh();
  });

  async function refresh() {
    try {
      projects = await ipc.projectsList();
      error = null;
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      loading = false;
    }
  }

  async function activate(p: ProjectInfo) {
    activating = p.key;
    activationToast = null;
    try {
      const report = await ipc.projectsActivate(p.key);
      const parts: string[] = [];
      if (report.stopped.length)
        parts.push(`stopped ${report.stopped.length}`);
      if (report.started.length)
        parts.push(`started ${report.started.length}`);
      if (report.failed.length)
        parts.push(`${report.failed.length} failed`);
      if (report.vhostsWritten != null && report.vhostsWritten > 0)
        parts.push(
          `${report.vhostsWritten} vhost${report.vhostsWritten === 1 ? "" : "s"} emitted`,
        );
      if (report.hostsFileUpdated) parts.push("hosts file updated");
      let toast =
        parts.length > 0
          ? `${p.name}: ${parts.join(", ")}`
          : `${p.name} is active.`;
      if (report.vhostWarnings.length) {
        toast += ` · ${report.vhostWarnings.length} warning${report.vhostWarnings.length === 1 ? "" : "s"}`;
        console.warn("vhost warnings", report.vhostWarnings);
      }
      activationToast = toast;
      await refresh();
      setTimeout(() => (activationToast = null), 6000);
    } catch (e) {
      activationToast = `Error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      activating = null;
    }
  }

  async function deactivate(p: ProjectInfo) {
    activating = p.key;
    try {
      const stopped = await ipc.projectsDeactivate();
      activationToast = `Deactivated. Stopped ${stopped.length} service${stopped.length === 1 ? "" : "s"}.`;
      await refresh();
      setTimeout(() => (activationToast = null), 4000);
    } catch (e) {
      activationToast = `Error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      activating = null;
    }
  }

  async function remove(p: ProjectInfo) {
    if (!confirm(`Delete project "${p.name}"? Services won't be stopped.`)) return;
    try {
      await ipc.projectsDelete(p.key);
      await refresh();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  }

  function fmtDate(ts: number | null): string {
    if (!ts) return "never";
    return new Date(ts * 1000).toLocaleDateString(undefined, {
      month: "short",
      day: "numeric",
      year: "numeric",
    });
  }
</script>

<section class="page">
  <header class="head">
    <div>
      <h1>Projects</h1>
      <p class="lede">
        Save the stack you actually use. Activating a project stops everything else
        and brings up only its services with its env vars.
      </p>
    </div>
    <button class="btn btn-primary" onclick={() => (editing = null)}>
      + New project
    </button>
  </header>

  {#if error}
    <div class="error-banner"><strong>Error:</strong> {error}</div>
  {/if}
  {#if activationToast}
    <div class="toast">{activationToast}</div>
  {/if}

  {#if loading}
    <p class="loading">Loading…</p>
  {:else if projects.length === 0}
    <div class="empty">
      <p class="empty-title">No projects yet.</p>
      <p class="empty-body">
        Create one to bind a folder + a stack of services + env vars under a name
        you can switch back to in one click.
      </p>
      <button class="btn btn-primary" onclick={() => (editing = null)}>
        + Create your first project
      </button>
    </div>
  {:else}
    <div class="grid">
      {#each projects as p (p.key)}
        <article
          class="card"
          class:active={p.isActive}
          oncontextmenu={(e) => openMenu(e, p)}
        >
          <header>
            <div class="title-block">
              <h3>{p.name}</h3>
              {#if p.rootDir}
                <code class="path">{p.rootDir}</code>
              {/if}
            </div>
            {#if p.isActive}
              <span class="active-badge">Active</span>
            {/if}
          </header>

          {#if p.notes}
            <p class="notes">{p.notes}</p>
          {/if}

          <div class="services">
            {#each p.services as s (s)}
              <span class="svc">{s}</span>
            {/each}
            {#if p.services.length === 0}
              <span class="empty-svc">No services bound.</span>
            {/if}
          </div>

          <footer>
            <span class="meta">
              created {fmtDate(p.createdAt)}
              {#if p.lastActiveAt}
                · last active {fmtDate(p.lastActiveAt)}
              {/if}
            </span>
            <div class="actions">
              {#if p.isActive}
                <button
                  class="btn-mini"
                  onclick={() => deactivate(p)}
                  disabled={activating != null}
                >
                  {activating === p.key ? "Stopping…" : "Deactivate"}
                </button>
              {:else}
                <button
                  class="btn-mini primary"
                  onclick={() => activate(p)}
                  disabled={activating != null || p.services.length === 0}
                >
                  {activating === p.key ? "Activating…" : "Activate"}
                </button>
              {/if}
              {#if p.rootDir}
                <button class="btn-mini ghost" onclick={() => openTerminal(p)}>Terminal</button>
              {/if}
              <button class="btn-mini ghost" onclick={() => (editing = p)}>Edit</button>
              <button class="btn-mini ghost danger" onclick={() => remove(p)}>Delete</button>
            </div>
          </footer>
        </article>
      {/each}
    </div>
  {/if}
</section>

{#if editing !== undefined}
  <ProjectEditor
    initial={editing}
    onClose={() => (editing = undefined)}
    onSaved={(next) => {
      editing = undefined;
      projects = [next, ...projects.filter((p) => p.key !== next.key)];
    }}
  />
{/if}

{#if menu}
  <ContextMenu
    x={menu.x}
    y={menu.y}
    items={buildMenuItems(menu.project)}
    onClose={() => (menu = null)}
  />
{/if}

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
    gap: 16px;
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
    max-width: 600px;
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

  .toast {
    background: var(--accent-soft);
    border: 1px solid transparent;
    color: var(--accent);
    padding: 8px 14px;
    border-radius: var(--radius-sm);
    margin-bottom: 16px;
    font-size: 13px;
    font-weight: 500;
  }

  .loading {
    color: var(--text-dim);
  }

  .empty {
    text-align: center;
    padding: 80px 24px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-lg);
    background: var(--bg-1);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .empty-title {
    margin: 0;
    font-weight: 600;
    font-size: 14px;
  }

  .empty-body {
    margin: 0;
    color: var(--text-dim);
    max-width: 480px;
    font-size: 13px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(360px, 1fr));
    gap: 14px;
  }

  .card {
    background: var(--bg-1);
    border: 1px solid var(--border);
    border-radius: var(--radius-lg);
    padding: 18px 20px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    transition: border-color 120ms ease;
  }

  .card.active {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent-soft);
  }

  .card header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
  }

  .title-block {
    display: flex;
    flex-direction: column;
    gap: 3px;
    min-width: 0;
  }

  h3 {
    font-size: 15px;
    font-weight: 600;
    margin: 0;
    letter-spacing: -0.01em;
  }

  .path {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 11px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    background: transparent;
    padding: 0;
  }

  .active-badge {
    background: var(--accent);
    color: var(--bg-0);
    font-size: 10px;
    font-weight: 600;
    padding: 2px 8px;
    border-radius: 999px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .notes {
    margin: 0;
    color: var(--text-dim);
    font-size: 12.5px;
    line-height: 1.5;
  }

  .services {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
  }

  .svc {
    background: var(--bg-2);
    color: var(--text-dim);
    font-size: 11px;
    font-weight: 500;
    padding: 2px 9px;
    border-radius: 999px;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }

  .empty-svc {
    color: var(--text-muted);
    font-style: italic;
    font-size: 12px;
  }

  footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin-top: auto;
    padding-top: 6px;
    flex-wrap: wrap;
  }

  .meta {
    font-size: 11px;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    gap: 4px;
    flex-shrink: 0;
  }

  .btn-mini {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 5px 11px;
    font-size: 11.5px;
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

  .btn-mini.ghost.danger:hover {
    color: var(--danger);
  }
</style>
