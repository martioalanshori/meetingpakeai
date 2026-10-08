<script lang="ts">
  // Snippet FTS5: kata yang cocok diapit "[" "]" → <mark>. Tanpa {@html} agar teks transkrip tidak dirender sebagai HTML.
  let { text }: { text: string } = $props();

  const parts = $derived(
    text.split(/(\[[^\]]*\])/).map((p) =>
      p.startsWith("[") && p.endsWith("]") ? { mark: true, text: p.slice(1, -1) } : { mark: false, text: p },
    ),
  );
</script>

{#each parts as p, i (i)}{#if p.mark}<mark class="rounded bg-yellow-200 px-0.5">{p.text}</mark>{:else}{p.text}{/if}{/each}
