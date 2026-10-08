<script lang="ts">
  import { id } from "$lib/i18n/id";
  import { rec, startRecording, stopRecording } from "$lib/recording.svelte";

  const recording = $derived(rec.state.status !== "idle");
</script>

<button
  type="button"
  class={[
    "rounded-lg px-5 py-2.5 font-medium text-white disabled:opacity-60",
    recording ? "bg-gray-800 hover:bg-gray-900" : "bg-red-600 hover:bg-red-700",
  ]}
  disabled={rec.busy}
  onclick={() => (recording ? stopRecording() : startRecording())}
>
  {#if rec.busy}
    {recording ? id.home.stopping : id.home.starting}
  {:else}
    {recording ? id.home.stopRecording : id.home.startRecording}
  {/if}
</button>
