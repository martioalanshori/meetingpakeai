<script lang="ts">
  import { id } from "$lib/i18n/id";
  import { rec, startRecording } from "$lib/recording.svelte";

  // Tawaran dari deteksi meeting: satu klik untuk mulai rekam, tidak merekam otomatis.
  const app = $derived(rec.offer ? (id.offer.apps[rec.offer as keyof typeof id.offer.apps] ?? rec.offer) : "");
</script>

{#if rec.offer && rec.state.status === "idle"}
  <div
    role="alert"
    class="fixed top-4 left-1/2 z-40 flex w-[min(36rem,calc(100%-2rem))] -translate-x-1/2 items-center gap-3 rounded-xl border border-indigo-200 bg-white p-3 shadow-lg"
  >
    <span class="flex-1 text-gray-900">{id.offer.text(app)}</span>
    <button
      type="button"
      class="rounded-lg bg-red-600 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-700 disabled:opacity-60"
      disabled={rec.busy}
      onclick={() => startRecording(rec.offer)}
    >
      {id.offer.start}
    </button>
    <button type="button" class="rounded-lg px-3 py-1.5 text-sm hover:bg-gray-100" onclick={() => (rec.offer = null)}>
      {id.offer.dismiss}
    </button>
  </div>
{/if}
