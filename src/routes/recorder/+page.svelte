<script lang="ts">
  import { formatTimestamp } from "$lib/format";

  // Langkah 6: state diisi dari event recording://state dan recording://level.
  let elapsedMs = $state(0);
</script>

<!-- Seluruh widget bisa digeser (jendela tanpa dekorasi). -->
<div
  data-tauri-drag-region
  class="flex h-screen select-none items-center gap-3 bg-gray-900 px-3 text-white"
>
  <span class="h-3 w-3 animate-pulse rounded-full bg-red-500"></span>
  <span class="font-mono text-sm tabular-nums">{formatTimestamp(elapsedMs)}</span>
</div>
