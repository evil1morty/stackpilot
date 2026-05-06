<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import Sidebar from "$lib/components/Sidebar.svelte";
  import { theme } from "$lib/stores/theme.svelte";

  let { children } = $props();

  const navByAccel: Record<string, string> = {
    "1": "/",
    "2": "/services",
    "3": "/presets",
    "4": "/logs",
  };

  onMount(() => {
    theme.init();

    function onKey(e: KeyboardEvent) {
      if (!(e.ctrlKey || e.metaKey)) return;
      const target = e.target as HTMLElement | null;
      const isTyping =
        target && (target.tagName === "INPUT" || target.tagName === "TEXTAREA");
      if (isTyping) return;
      const dest = navByAccel[e.key];
      if (dest) {
        e.preventDefault();
        goto(dest);
      }
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="app">
  <Sidebar />
  <main>
    {@render children()}
  </main>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 220px 1fr;
    height: 100vh;
    width: 100vw;
    background: var(--bg-0);
  }

  main {
    overflow: auto;
    background: var(--bg-0);
  }
</style>
