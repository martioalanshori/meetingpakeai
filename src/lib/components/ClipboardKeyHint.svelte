<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { id } from "$lib/i18n/id";

  // Tawarkan API key yang baru disalin dari halaman penyedia (langkah 24). Hanya key yang bentuknya cocok
  // dengan `provider` yang ditawarkan. Key tidak diisi diam-diam: pengguna harus menekan tombol.
  let { provider, onuse }: { provider: string; onuse: (key: string) => void } = $props();

  let found = $state<string | null>(null);
  let dismissed = $state<string | null>(null);

  async function check() {
    const d = await api.detectApiKeyInClipboard().catch(() => null);
    found = d && d.provider === provider ? d.key : null;
  }

  onMount(check);

  const masked = $derived(found ? `${found.slice(0, 4)}…${found.slice(-4)}` : "");
</script>

<!-- Pengguna kembali dari browser setelah menyalin key → cek lagi. -->
<svelte:window onfocus={check} />

{#if found && found !== dismissed}
  <div class="flex flex-wrap items-center gap-3 rounded-xl bg-mic-wash px-4 py-3 text-sm text-ink" role="status">
    <span class="flex-1">{id.settings.apiKey.clipboardFound(masked)}</span>
    <button
      type="button"
      class="btn btn-ink"
      onclick={() => {
        if (found) onuse(found);
        dismissed = found;
      }}
    >
      {id.settings.apiKey.clipboardUse}
    </button>
    <button type="button" class="btn btn-quiet" onclick={() => (dismissed = found)}>
      {id.common.close}
    </button>
  </div>
{/if}
