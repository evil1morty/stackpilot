<script lang="ts">
  import { onMount } from "svelte";
  import { ipc } from "$lib/ipc";
  import type { ProjectInfo, ServiceInfo } from "$lib/types";

  import type { VHost } from "$lib/types";

  let {
    initial,
    onClose,
    onSaved,
  }: {
    /** Editing an existing project — pass it in. Creating new — pass null. */
    initial: ProjectInfo | null;
    onClose: () => void;
    onSaved: (next: ProjectInfo) => void;
  } = $props();

  // The parent re-mounts ProjectEditor on every open, so capturing the
  // initial value of `initial` exactly once is correct here.
  /* svelte-ignore state_referenced_locally */
  let name = $state(initial?.name ?? "");
  /* svelte-ignore state_referenced_locally */
  let rootDir = $state(initial?.rootDir ?? "");
  /* svelte-ignore state_referenced_locally */
  let notes = $state(initial?.notes ?? "");
  /* svelte-ignore state_referenced_locally */
  let selectedServices = $state<string[]>(initial?.services ?? []);
  /* svelte-ignore state_referenced_locally */
  let envPairs = $state<Array<{ k: string; v: string }>>(
    Object.entries(initial?.envVars ?? {}).map(([k, v]) => ({ k, v })),
  );
  /* svelte-ignore state_referenced_locally */
  let vhosts = $state<VHost[]>(
    (initial?.vhosts ?? []).map((v) => ({ ...v })),
  );

  function addVhost() {
    const suggested = name.trim().toLowerCase().replace(/\s+/g, "-") || "myapp";
    vhosts = [
      ...vhosts,
      {
        host: `${suggested}.test`,
        port: 80,
        documentRoot: "",
        server: "nginx",
        ssl: false,
      },
    ];
  }

  function removeVhost(i: number) {
    vhosts = vhosts.filter((_, idx) => idx !== i);
  }

  let availableServices = $state<ServiceInfo[]>([]);
  let saving = $state(false);
  let error = $state<string | null>(null);

  onMount(async () => {
    try {
      availableServices = await ipc.servicesList();
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    }
  });

  function toggleService(key: string) {
    selectedServices = selectedServices.includes(key)
      ? selectedServices.filter((k) => k !== key)
      : [...selectedServices, key];
  }

  function addEnvPair() {
    envPairs = [...envPairs, { k: "", v: "" }];
  }

  function removeEnvPair(i: number) {
    envPairs = envPairs.filter((_, idx) => idx !== i);
  }

  async function save() {
    saving = true;
    error = null;
    try {
      const envVars: Record<string, string> = {};
      for (const { k, v } of envPairs) {
        if (k.trim()) envVars[k.trim()] = v;
      }
      const input = {
        name: name.trim(),
        rootDir: rootDir.trim(),
        services: selectedServices,
        envVars,
        notes: notes.trim(),
        vhosts: vhosts
          .filter((v) => v.host.trim() !== "")
          .map((v) => ({
            host: v.host.trim(),
            port: Number(v.port) || 80,
            documentRoot: v.documentRoot.trim(),
            server: v.server || "nginx",
            ssl: !!v.ssl,
          })),
      };
      const next = initial
        ? await ipc.projectsUpdate(initial.key, input)
        : await ipc.projectsCreate(input);
      onSaved(next);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally {
      saving = false;
    }
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
    aria-label={initial ? "Edit project" : "New project"}
    tabindex="-1"
    onclick={(e) => e.stopPropagation()}
    onkeydown={(e) => e.stopPropagation()}
  >
    <header class="modal-head">
      <h2>{initial ? "Edit project" : "New project"}</h2>
      <button class="close" onclick={onClose} aria-label="Close">×</button>
    </header>

    <div class="body">
      <label class="row">
        <span class="label">Name</span>
        <input type="text" bind:value={name} placeholder="My App" />
      </label>

      <label class="row">
        <span class="label">Root directory <span class="muted">(optional)</span></span>
        <input
          type="text"
          bind:value={rootDir}
          placeholder="C:\code\myapp"
          spellcheck="false"
        />
      </label>

      <div class="row">
        <span class="label">Services</span>
        <div class="services">
          {#each availableServices as svc (svc.key)}
            <label class="svc-chip" class:selected={selectedServices.includes(svc.key)}>
              <input
                type="checkbox"
                checked={selectedServices.includes(svc.key)}
                onchange={() => toggleService(svc.key)}
              />
              {svc.display}
              {#if !svc.installed}
                <span class="missing">not installed</span>
              {/if}
            </label>
          {/each}
        </div>
      </div>

      <div class="row">
        <span class="label">Env vars <span class="muted">(applied when starting services)</span></span>
        <div class="env-list">
          {#each envPairs as pair, i (i)}
            <div class="env-row">
              <input
                type="text"
                bind:value={pair.k}
                placeholder="DB_HOST"
                spellcheck="false"
              />
              <span class="eq">=</span>
              <input
                type="text"
                bind:value={pair.v}
                placeholder="localhost"
                spellcheck="false"
              />
              <button class="env-rm" onclick={() => removeEnvPair(i)} aria-label="Remove">×</button>
            </div>
          {/each}
          <button class="env-add" onclick={addEnvPair}>+ Add variable</button>
        </div>
      </div>

      <div class="row">
        <span class="label">
          Pretty URLs <span class="muted">(needs nginx — applied on Activate)</span>
        </span>
        <div class="vhost-list">
          {#each vhosts as vh, i (i)}
            <div class="vhost-row">
              <input
                type="text"
                bind:value={vh.host}
                placeholder="myapp.test"
                spellcheck="false"
              />
              <span class="eq">:</span>
              <input
                type="number"
                class="port"
                bind:value={vh.port}
                min="1"
                max="65535"
              />
              <input
                type="text"
                class="docroot"
                bind:value={vh.documentRoot}
                placeholder={rootDir || "(use root_dir)"}
                spellcheck="false"
                title="Document root — leave blank to inherit project root"
              />
              <button class="vh-rm" onclick={() => removeVhost(i)} aria-label="Remove">×</button>
            </div>
          {/each}
          <button class="vh-add" onclick={addVhost}>+ Add pretty URL</button>
        </div>
      </div>

      <label class="row">
        <span class="label">Notes <span class="muted">(optional)</span></span>
        <textarea
          bind:value={notes}
          rows="2"
          placeholder="What is this project for?"
        ></textarea>
      </label>

      {#if error}
        <p class="err">{error}</p>
      {/if}
    </div>

    <footer class="modal-foot">
      <button class="btn" onclick={onClose}>Cancel</button>
      <button class="btn btn-primary" onclick={save} disabled={saving || !name.trim()}>
        {saving ? "Saving…" : initial ? "Save" : "Create project"}
      </button>
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
    max-width: 600px;
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    box-shadow: var(--shadow-lg);
  }

  .modal-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 22px 14px 22px;
    border-bottom: 1px solid var(--border);
  }

  .modal-head h2 {
    font-size: 16px;
    font-weight: 600;
    margin: 0;
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
    padding: 18px 22px;
    overflow-y: auto;
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .label {
    font-size: 11.5px;
    color: var(--text-dim);
    font-weight: 500;
  }

  .muted {
    color: var(--text-muted);
    font-weight: 400;
  }

  input[type="text"],
  textarea {
    background: var(--bg-2);
    border: 1px solid var(--border);
    border-radius: var(--radius-sm);
    padding: 7px 10px;
    color: var(--text);
    font-size: 13px;
    font-family: inherit;
    transition: border-color 120ms ease;
  }

  input[type="text"]:focus,
  textarea:focus {
    outline: none;
    border-color: var(--accent);
    background: var(--bg-3);
  }

  textarea {
    resize: vertical;
    min-height: 50px;
    font-family: inherit;
  }

  .services {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .svc-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 5px 11px;
    border-radius: 999px;
    background: var(--bg-2);
    border: 1px solid var(--border);
    color: var(--text-dim);
    font-size: 12px;
    font-weight: 500;
    cursor: pointer;
    transition: all 120ms ease;
  }

  .svc-chip:hover {
    background: var(--bg-3);
    color: var(--text);
  }

  .svc-chip.selected {
    background: var(--accent-soft);
    color: var(--accent);
    border-color: transparent;
  }

  .svc-chip input[type="checkbox"] {
    margin: 0;
    accent-color: var(--accent);
    width: 12px;
    height: 12px;
  }

  .missing {
    font-size: 10px;
    color: var(--warning);
    margin-left: 4px;
  }

  .env-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .env-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .env-row input {
    flex: 1;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
  }

  .eq {
    color: var(--text-muted);
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }

  .env-rm {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 18px;
    line-height: 1;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .env-rm:hover {
    background: var(--bg-2);
    color: var(--danger);
  }

  .env-add,
  .vh-add {
    align-self: flex-start;
    background: transparent;
    border: 1px dashed var(--border-strong);
    color: var(--text-dim);
    padding: 5px 12px;
    border-radius: var(--radius-sm);
    font-size: 11.5px;
    cursor: pointer;
  }

  .env-add:hover,
  .vh-add:hover {
    background: var(--bg-2);
    color: var(--text);
  }

  .vhost-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .vhost-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .vhost-row input[type="text"] {
    flex: 1;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
  }

  .vhost-row .port {
    width: 72px;
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
    font-size: 12px;
    text-align: center;
  }

  .vhost-row .docroot {
    flex: 1.4;
  }

  .vh-rm {
    background: transparent;
    border: none;
    color: var(--text-muted);
    font-size: 18px;
    line-height: 1;
    padding: 4px 8px;
    border-radius: var(--radius-sm);
    cursor: pointer;
  }

  .vh-rm:hover {
    background: var(--bg-2);
    color: var(--danger);
  }

  .err {
    margin: 0;
    padding: 8px 12px;
    background: var(--danger-soft);
    color: var(--danger);
    border-radius: var(--radius-sm);
    font-size: 12px;
  }

  .modal-foot {
    display: flex;
    gap: 8px;
    justify-content: flex-end;
    padding: 12px 22px;
    border-top: 1px solid var(--border);
  }
</style>
