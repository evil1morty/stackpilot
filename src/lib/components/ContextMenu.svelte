<script lang="ts">
  import { tick } from "svelte";

  export type ContextMenuItem =
    | { kind: "item"; label: string; action: () => void; disabled?: boolean; danger?: boolean; hint?: string }
    | { kind: "divider" };

  let {
    x,
    y,
    items,
    onClose,
  }: {
    x: number;
    y: number;
    items: ContextMenuItem[];
    onClose: () => void;
  } = $props();

  let el: HTMLDivElement | undefined = $state();
  // The parent unmounts + remounts the menu on each right-click so the
  // captured-once-at-init pattern is correct here.
  /* svelte-ignore state_referenced_locally */
  let pos = $state({ x, y });

  // Re-clamp inside viewport once we know our size.
  $effect(() => {
    void items.length;
    void x;
    void y;
    tick().then(() => {
      if (!el) return;
      const r = el.getBoundingClientRect();
      const margin = 4;
      let nx = x;
      let ny = y;
      if (nx + r.width > window.innerWidth - margin) {
        nx = Math.max(margin, window.innerWidth - r.width - margin);
      }
      if (ny + r.height > window.innerHeight - margin) {
        ny = Math.max(margin, window.innerHeight - r.height - margin);
      }
      pos = { x: nx, y: ny };
    });
  });

  // Outside-click + Escape dismissal.
  $effect(() => {
    function handleDocClick(e: MouseEvent) {
      if (!el) return;
      if (!el.contains(e.target as Node)) onClose();
    }
    function handleKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    function handleScroll() {
      onClose();
    }
    // mousedown so we close before the next click bubble can re-trigger.
    document.addEventListener("mousedown", handleDocClick, true);
    document.addEventListener("keydown", handleKey);
    window.addEventListener("scroll", handleScroll, true);
    return () => {
      document.removeEventListener("mousedown", handleDocClick, true);
      document.removeEventListener("keydown", handleKey);
      window.removeEventListener("scroll", handleScroll, true);
    };
  });

  function activate(item: ContextMenuItem) {
    if (item.kind !== "item" || item.disabled) return;
    item.action();
    onClose();
  }
</script>

<div
  class="ctx"
  bind:this={el}
  style="left: {pos.x}px; top: {pos.y}px;"
  role="menu"
  tabindex="-1"
>
  {#each items as item, i (i)}
    {#if item.kind === "divider"}
      <div class="ctx-divider" role="separator"></div>
    {:else}
      <button
        type="button"
        class="ctx-item"
        class:disabled={item.disabled}
        class:danger={item.danger}
        disabled={item.disabled}
        onclick={() => activate(item)}
        role="menuitem"
      >
        <span class="ctx-label">{item.label}</span>
        {#if item.hint}<span class="ctx-hint">{item.hint}</span>{/if}
      </button>
    {/if}
  {/each}
</div>

<style>
  .ctx {
    position: fixed;
    background: var(--bg-2);
    border: 1px solid var(--border-strong);
    border-radius: var(--radius);
    padding: 4px;
    min-width: 180px;
    box-shadow: var(--shadow-lg);
    z-index: 200;
    user-select: none;
  }

  .ctx-divider {
    height: 1px;
    background: var(--border);
    margin: 4px 2px;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    width: 100%;
    background: transparent;
    border: none;
    color: var(--text);
    font-size: 12.5px;
    padding: 6px 10px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    text-align: left;
    transition: background 80ms ease, color 80ms ease;
  }

  .ctx-item:hover:not(.disabled) {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .ctx-item.danger {
    color: var(--danger);
  }

  .ctx-item.danger:hover:not(.disabled) {
    background: var(--danger-soft);
    color: var(--danger);
  }

  .ctx-item.disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .ctx-label {
    flex: 1;
  }

  .ctx-hint {
    font-size: 10.5px;
    color: var(--text-muted);
    font-family: ui-monospace, "Cascadia Code", "JetBrains Mono", Menlo, Consolas, monospace;
  }
</style>
