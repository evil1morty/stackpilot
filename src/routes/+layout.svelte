<script lang="ts">
  import "../app.css";
  import Sidebar from "$lib/components/Sidebar.svelte";

  let { children } = $props();
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
