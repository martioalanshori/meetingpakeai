<script lang="ts">
  import { onDestroy } from "svelte";
  import { formatTimestamp } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { rec, startRecording, stopRecording } from "$lib/recording.svelte";

  // Kontrol rekam di rel kiri: idle = tombol merah; merekam = lampu + timer + Stop.
  const recording = $derived(rec.state.status !== "idle");
  const paused = $derived(rec.state.status === "paused");

  let baseMs = 0;
  let baseAt = 0;
  let now = $state(0);
  let timer: ReturnType<typeof setInterval> | undefined;

  $effect(() => {
    baseMs = rec.state.elapsedMs;
    baseAt = performance.now();
    now = baseAt;
    clearInterval(timer);
    // Timer hanya berdetak saat merekam (CPU idle ≈ 0).
    if (rec.state.status === "recording") timer = setInterval(() => (now = performance.now()), 500);
  });
  onDestroy(() => clearInterval(timer));

  const elapsed = $derived(rec.state.status === "recording" ? baseMs + Math.max(0, now - baseAt) : baseMs);
</script>

{#if !recording}
  <button type="button" class="btn btn-rec w-full py-2.5 text-base" disabled={rec.busy} onclick={() => startRecording()}>
    <span class="h-2.5 w-2.5 rounded-full bg-white" aria-hidden="true"></span>
    {rec.busy ? id.home.starting : id.home.startRecording}
  </button>
{:else}
  <div class="flex flex-col gap-2.5 rounded-xl border border-line bg-sheet p-3" role="status">
    <div class="flex items-center gap-2">
      <span
        class={["h-2.5 w-2.5 rounded-full", paused ? "bg-warn" : "bg-rec motion-safe:animate-pulse"]}
        aria-hidden="true"
      ></span>
      <span class="text-sm font-semibold">{paused ? id.rail.paused : id.rail.recording}</span>
      <span class="tabular ml-auto text-sm text-ink-soft">{formatTimestamp(elapsed)}</span>
    </div>
    <button type="button" class="btn btn-ink w-full" disabled={rec.busy} onclick={stopRecording}>
      {rec.busy ? id.home.stopping : id.home.stopRecording}
    </button>
  </div>
{/if}
