<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import type { ConfigFileInfo, ServiceInfo } from "$lib/types";

  let {
    service,
    onClose,
    onSaved,
  }: {
    service: ServiceInfo;
    onClose: () => void;
    onSaved: (info: ServiceInfo) => void;
  } = $props();

  let files = $state<ConfigFileInfo[]>([]);
  let activePath = $state<string | null>(null);
  let content = $state("");
  let original = $state("");
  let loadError = $state<string | null>(null);
  let saving = $state(false);
  let restartAfterSave = $state(true);
  let saveResult = $state<string | null>(null);

  const dirty = $derived(content !== original);
  const activeFile = $derived(files.find((f) => f.path === activePath) ?? null);

  onMount(async () => {
    try {
      files = await ipc.servicesConfigFiles(service.key);
      const first = files.find((f) => f.exists) ?? files[0];
      if (first) await openFile(first.path);
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
    }
  });

  async function openFile(path: string) {
    if (dirty) {
      const ok = confirm("Discard unsaved changes?");
      if (!ok) return;
    }
    activePath = path;
    saveResult = null;
    try {
      const text = await ipc.servicesConfigRead(path);
      content = text;
      original = text;
      loadError = null;
    } catch (e) {
      loadError = e instanceof Error ? e.message : String(e);
      content = "";
      original = "";
    }
  }

  async function save() {
    if (!activePath || !dirty) return;
    saving = true;
    saveResult = null;
    try {
      await ipc.servicesConfigWrite(activePath, content);
      original = content;

      let next = service;
      if (restartAfterSave && service.status.kind === "runningTracked") {
        next = await ipc.servicesRestart(service.key);
        saveResult = "Saved + restarted.";
      } else {
        saveResult = "Saved. Restart the service to apply.";
      }
      onSaved(next);

      // Refresh file metadata.
      files = await ipc.servicesConfigFiles(service.key);
    } catch (e) {
      saveResult = `Error: ${e instanceof Error ? e.message : String(e)}`;
    } finally {
      saving = false;
    }
  }

  function fmtSize(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  }
</script>

<div
  class="overlay"
  role="presentation"
  onclick={onClose}
  onkeydown={(e) => {
    if (e.key === "Escape") onClose();
  }}
>
  <div
    class="modal"
    role="dialog"
    aria-modal="true"
    aria-label="{service.display} configuration"
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => {
      if ((e.ctrlKey || e.metaKey) && e.key === "s") {
        e.preventDefault();
        save();
      }
      e.stopPropagation();
    }}
  >
    <header class="modal-head">
      <div>
        <h2>{service.display} · configs</h2>
        <p class="hint">Ctrl+S saves. Backups kept as <code>.bak</code> next to each file.</p>
      </div>
      <button class="close" onclick={onClose} aria-label="Close">×</button>
    </header>

    <div class="body">
      <aside class="files">
        {#if files.length === 0}
          <p class="files-empty">No configurable files for this service.</p>
        {/if}
        {#each files as f (f.path)}
          <button
            class="file-item"
            class:active={f.path === activePath}
            class:missing={!f.exists}
            onclick={() => openFile(f.path)}
            title={f.path}
          >
            <span class="file-label">{f.label}</span>
            <span class="file-meta">
              {#if f.exists}
                {fmtSize(f.sizeBytes)}
              {:else}
                missing
              {/if}
            </span>
          </button>
        {/each}
      </aside>

      <main class="editor">
        {#if loadError}
          <div class="err-block">{loadError}</div>
        {:else if !activePath}
          <div class="empty-block">Pick a file on the left.</div>
        {:else if activeFile && !activeFile.exists}
          <div class="empty-block">
            File doesn't exist yet — start the service once to populate it.
          </div>
        {:else}
          <div class="editor-head">
            <code class="path">{activePath}</code>
            {#if dirty}
              <span class="dirty-badge">unsaved</span>
            {/if}
          </div>
          <textarea
            bind:value={content}
            spellcheck="false"
            wrap="off"
            placeholder="Loading…"
          ></textarea>
        {/if}
      </main>
    </div>

    <footer class="modal-foot">
      <label class="restart-toggle">
        <input
          type="checkbox"
          bind:checked={restartAfterSave}
          disabled={service.status.kind !== "runningTracked"}
        />
        Restart service after save
        {#if service.status.kind !== "runningTracked"}
          <span class="muted">(not running)</span>
        {/if}
      </label>

      {#if saveResult}
        <span class="save-result" class:err={saveResult.startsWith("Error")}>{saveResult}</span>
      {/if}

      <div class="foot-actions">
        <button class="btn" onclick={onClose}>Close</button>
        <button
          class="btn btn-primary"
          onclick={save}
          disabled={!dirty || !activeFile?.exists || saving}
        >
          {saving ? "Saving…" : "Save"}
        </button>
      </div>
    </footer>
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
    width: 92%;
    max-width: 1000px;
    height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow-lg);
  }

  .modal-head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 18px 22px 14px 22px;
    border-bottom: 1px solid var(--border);
  }

  .modal-head h2 {
    font-size: 16px;
    font-weight: 600;
    margin: 0 0 2px 0;
    letter-spacing: -0.01em;
  }

  .hint {
    margin: 0;
    color: var(--text-muted);
    font-size: 11.5px;
  }

  code {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 11.5px;
    color: var(--text-dim);
  }

  .close {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 22px;
    line-height: 1;
    width: 28px;
    height: 28px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .close:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .body {
    display: grid;
    grid-template-columns: 220px 1fr;
    flex: 1;
    min-height: 0;
  }

  .files {
    border-right: 1px solid var(--border);
    overflow-y: auto;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .files-empty {
    color: var(--text-muted);
    font-size: 12px;
    padding: 12px;
  }

  .file-item {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 2px;
    padding: 8px 10px;
    border: none;
    background: transparent;
    color: var(--text-dim);
    font-size: 12.5px;
    text-align: left;
    border-radius: var(--radius-sm);
    cursor: pointer;
    width: 100%;
    transition: background 120ms ease, color 120ms ease;
  }

  .file-item:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .file-item.active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .file-item.missing {
    opacity: 0.5;
  }

  .file-label {
    font-weight: 500;
  }

  .file-meta {
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 10.5px;
    color: var(--text-muted);
  }

  .editor {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .editor-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 6px 14px;
    border-bottom: 1px solid var(--border);
    gap: 10px;
  }

  .path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dirty-badge {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--warning);
    background: var(--warning-soft);
    padding: 2px 8px;
    border-radius: 999px;
  }

  textarea {
    flex: 1;
    background: #07080b;
    color: var(--text);
    border: none;
    padding: 12px 16px;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12.5px;
    line-height: 1.55;
    resize: none;
    outline: none;
    white-space: pre;
    overflow: auto;
    border-radius: 0;
    tab-size: 2;
  }

  textarea:focus {
    border: none;
    background: #07080b;
  }

  .err-block,
  .empty-block {
    padding: 24px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .err-block {
    color: var(--danger);
  }

  .modal-foot {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 22px;
    border-top: 1px solid var(--border);
    flex-wrap: wrap;
  }

  .restart-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 12.5px;
    color: var(--text-dim);
    cursor: pointer;
  }

  .restart-toggle input[type="checkbox"] {
    accent-color: var(--accent);
    margin: 0;
  }

  .muted {
    color: var(--text-muted);
    font-size: 11px;
  }

  .save-result {
    font-size: 12px;
    color: var(--success);
    flex: 1;
    text-align: center;
  }

  .save-result.err {
    color: var(--danger);
  }

  .foot-actions {
    margin-left: auto;
    display: flex;
    gap: 8px;
  }
</style>
