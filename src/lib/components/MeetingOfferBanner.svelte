<script lang="ts">
  import { id } from "$lib/i18n/id";
  import { rec, startRecording } from "$lib/recording.svelte";

  // Tawaran dari deteksi meeting: satu klik untuk mulai rekam, tidak merekam otomatis.
  const app = $derived(rec.offer ? (id.offer.apps[rec.offer as keyof typeof id.offer.apps] ?? rec.offer) : "");
</script>

{#if rec.offer && rec.state.status === "idle"}
  <div
    role="alert"
    class="on-dark fixed top-4 left-1/2 z-40 flex w-[min(34rem,calc(100%-2rem))] -translate-x-1/2 items-center gap-3 rounded-xl bg-ink py-2.5 pr-2.5 pl-4 text-white shadow-[0_16px_40px_-12px_rgb(30_36_51/0.55)]"
  >
    <span class="h-2.5 w-2.5 shrink-0 rounded-full bg-rec" aria-hidden="true"></span>
    <span class="flex-1 text-sm">{id.offer.text(app)}</span>
    <button type="button" class="btn btn-rec" disabled={rec.busy} onclick={() => startRecording(rec.offer)}>
      {id.offer.start}
    </button>
    <button type="button" class="btn text-white/70 hover:text-white" onclick={() => (rec.offer = null)}>
      {id.offer.dismiss}
    </button>
  </div>
{/if}
