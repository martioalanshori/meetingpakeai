<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { id } from "$lib/i18n/id";

  // Tawarkan API key Groq yang baru disalin dari console.groq.com (langkah 24).
  // Key tidak diisi diam-diam: pengguna harus menekan tombol.
  let { onuse }: { onuse: (key: string) => void } = $props();

  let found = $state<string | null>(null);
  let dismissed = $state<string | null>(null);

  async function check() {
    found = await api.detectApiKeyInClipboard().catch(() => null);
  }

  onMount(check);

  const masked = $derived(found ? `${found.slice(0, 4)}…${found.slice(-4)}` : "");
</script>

<!-- Pengguna kembali dari browser setelah menyalin key → cek lagi. -->
<svelte:window onfocus={check} />

{#if found && found !== dismissed}
  <div class="flex flex-wrap items-center gap-3 rounded-lg border border-indigo-200 bg-indigo-50 p-3 text-sm text-indigo-950" role="status">
    <span class="flex-1">{id.settings.apiKey.clipboardFound(masked)}</span>
    <button
      type="button"
      class="rounded-lg bg-indigo-600 px-3 py-1.5 font-medium text-white hover:bg-indigo-700"
      onclick={() => {
        if (found) onuse(found);
        dismissed = found;
      }}
    >
      {id.settings.apiKey.clipboardUse}
    </button>
    <button type="button" class="rounded-lg px-3 py-1.5 hover:bg-indigo-100" onclick={() => (dismissed = found)}>
      {id.common.close}
    </button>
  </div>
{/if}
