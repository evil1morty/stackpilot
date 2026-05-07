<script lang="ts">
  import { parseAnsiLine, styleToCss } from "$lib/util/ansi";

  let { text }: { text: string } = $props();

  const fragments = $derived(parseAnsiLine(text));
</script>

{#each fragments as frag, i (i)}
  {#if frag.style.fg || frag.style.bg || frag.style.bold || frag.style.dim || frag.style.italic || frag.style.underline}
    <span style={styleToCss(frag.style)}>{frag.text}</span>
  {:else}
    {frag.text || " "}
  {/if}
{/each}
