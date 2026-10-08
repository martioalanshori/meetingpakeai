<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { formatTimestamp } from "$lib/format";
  import { id } from "$lib/i18n/id";

  // Pemutar audio meeting: kontrol sendiri di atas <audio> tersembunyi (bukan kontrol bawaan WebView2).
  let { src, ontime }: { src: string; ontime?: (ms: number) => void } = $props();

  const RATES = [1, 1.5, 2];

  let audio = $state<HTMLAudioElement | null>(null);
  let playing = $state(false);
  let currentMs = $state(0);
  let durationMs = $state(0);
  let rate = $state(1);
  let seeking = $state(false);

  /** Dipanggil induk (bind:this): lompat ke posisi lalu putar. */
  export async function seekAndPlay(ms: number) {
    if (!audio) return;
    audio.currentTime = ms / 1000;
    currentMs = ms;
    await audio.play().catch(() => {});
  }

  export function pause() {
    audio?.pause();
  }

  function toggle() {
    if (!audio) return;
    if (audio.paused) audio.play().catch(() => {});
    else audio.pause();
  }

  function cycleRate() {
    rate = RATES[(RATES.indexOf(rate) + 1) % RATES.length];
    if (audio) audio.playbackRate = rate;
  }

  function onSeekInput(e: Event) {
    seeking = true;
    currentMs = Number((e.currentTarget as HTMLInputElement).value);
  }

  function onSeekChange(e: Event) {
    const ms = Number((e.currentTarget as HTMLInputElement).value);
    if (audio) audio.currentTime = ms / 1000;
    seeking = false;
    ontime?.(ms);
  }

  const progress = $derived(durationMs > 0 ? (currentMs / durationMs) * 100 : 0);
</script>

<div class="flex items-center gap-3">
  <audio
    bind:this={audio}
    {src}
    preload="auto"
    class="hidden"
    onplay={() => (playing = true)}
    onpause={() => (playing = false)}
    onended={() => (playing = false)}
    onloadedmetadata={() => (durationMs = (audio?.duration ?? 0) * 1000)}
    ontimeupdate={() => {
      if (!audio || seeking) return;
      currentMs = audio.currentTime * 1000;
      ontime?.(currentMs);
    }}
  ></audio>

  <button
    type="button"
    class="btn btn-ink btn-icon shrink-0 rounded-full"
    aria-label={playing ? id.player.pause : id.player.play}
    title={playing ? id.player.pause : id.player.play}
    onclick={toggle}
  >
    <Icon name={playing ? "pause" : "play"} size={16} />
  </button>

  <span class="tabular w-[4.5rem] shrink-0 text-sm text-ink-soft">{formatTimestamp(currentMs)}</span>

  <input
    type="range"
    class="seek min-w-0 flex-1"
    min="0"
    max={Math.max(1, Math.round(durationMs))}
    step="100"
    value={Math.round(currentMs)}
    style:--progress={`${progress}%`}
    aria-label={id.player.seek}
    oninput={onSeekInput}
    onchange={onSeekChange}
  />

  <span class="tabular w-[4.5rem] shrink-0 text-right text-sm text-ink-faint">{formatTimestamp(durationMs)}</span>

  <button
    type="button"
    class="btn btn-line btn-sm tabular w-12 shrink-0 justify-center"
    aria-label={id.player.speed}
    title={id.player.speed}
    onclick={cycleRate}
  >
    {rate}×
  </button>
</div>

<style>
  .seek {
    appearance: none;
    height: 1.25rem;
    background: transparent;
    cursor: pointer;
  }
  .seek::-webkit-slider-runnable-track {
    height: 0.375rem;
    border-radius: 999px;
    background: linear-gradient(
      to right,
      var(--color-ink-strong) 0 var(--progress),
      var(--color-line-soft) var(--progress) 100%
    );
  }
  .seek::-webkit-slider-thumb {
    appearance: none;
    width: 0.875rem;
    height: 0.875rem;
    margin-top: -0.25rem;
    border-radius: 999px;
    background: var(--color-ink-strong);
    border: 2px solid #fff;
    box-shadow: 0 0 0 1px var(--color-line);
  }
  .seek:focus-visible {
    outline: 2px solid var(--color-ink);
    outline-offset: 2px;
    border-radius: 999px;
  }
</style>
