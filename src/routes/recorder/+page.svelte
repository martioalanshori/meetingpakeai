<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
  import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
  import { sendNotification } from "@tauri-apps/plugin-notification";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { formatTimestamp } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import type { AppError, Channel, RecordingState } from "$lib/types";


  let rs = $state<RecordingState>({
    status: "idle",
    meetingId: null,
    elapsedMs: 0,
    micMuted: false,
    micAlive: true,
    systemAlive: true,
  });
  // Timer berjalan lokal di antara event state.
  let baseMs = 0;
  let baseAt = performance.now();
  let now = $state(performance.now());
  let micDb = $state(-90);
  let sysDb = $state(-90);
  let lostChannels = $state<Channel[]>([]);
  let systemSilent = $state(false);
  /** Transkripsi bertahap sudah berjalan (notulen akan cepat siap setelah Stop). */
  let liveTranscribing = $state(false);
  let autoStopDeadline = $state<number | null>(null);
  let autoStopReason = $state<"silence" | "meeting_ended">("silence");
  let silenceMin = $state(10);
  let systemSilentMin = $state(2);
  let limitSoonMin = $state<number | null>(null);
  let bookmarkCount = $state(0);
  let bookmarkFlash = $state(false);
  let flashTimer: ReturnType<typeof setTimeout> | undefined;
  let root = $state<HTMLDivElement | null>(null);
  let errorTimer: ReturnType<typeof setTimeout> | undefined;
  let busy = $state(false);
  let error = $state<string | null>(null);

  const elapsed = $derived(rs.status === "recording" ? baseMs + Math.max(0, now - baseAt) : baseMs);
  const secondsLeft = $derived(
    autoStopDeadline === null ? 0 : Math.max(0, Math.ceil((autoStopDeadline - now) / 1000)),
  );

  function applyState(s: RecordingState) {
    rs = s;
    baseMs = s.elapsedMs;
    baseAt = performance.now();
    if (s.status === "idle") {
      lostChannels = [];
      systemSilent = false;
      autoStopDeadline = null;
      error = null;
    }
    lostChannels = lostChannels.filter((c) => (c === "mic" ? !s.micAlive : !s.systemAlive));
  }

  /** dBFS (−60..0) → lebar meter 0..100%. */
  function meterWidth(db: number) {
    return `${Math.max(0, Math.min(100, ((db + 60) / 60) * 100))}%`;
  }

  // Tinggi jendela mengikuti isi (baris peringatan bisa 1–3 baris teks), bukan jumlah baris × tinggi tetap.
  $effect(() => {
    if (!root) return;
    const el = root;
    const fit = () => getCurrentWindow().setSize(new LogicalSize(300, Math.ceil(el.scrollHeight))).catch(() => {});
    const ro = new ResizeObserver(fit);
    for (const child of el.children) ro.observe(child);
    ro.observe(el);
    fit();
    return () => ro.disconnect();
  });

  // Pesan error hilang sendiri setelah 6 dtk.
  $effect(() => {
    if (!error) return;
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => (error = null), 6000);
  });

  const unlisten: UnlistenFn[] = [];
  let timer: ReturnType<typeof setInterval>;

  onMount(async () => {
    timer = setInterval(() => {
      // Timer hanya berdetak saat merekam (CPU idle ≈ 0).
      if (rs.status === "recording" || autoStopDeadline !== null) now = performance.now();
    }, 250);
    unlisten.push(
      await events.recordingState(applyState),
      await events.recordingLevel((l) => {
        micDb = l.micDbfs;
        sysDb = l.systemDbfs;
      }),
      await events.recordingWarning((w) => {
        if (w.code === "system_silent") {
          systemSilent = true;
          if (w.minutes) systemSilentMin = w.minutes;
        }
        else if (w.code === "system_ok") systemSilent = false;
        else if (w.code === "write_failed") error = id.recorder.writeFailed;
        else if (w.code === "limit_soon") limitSoonMin = w.minutes ?? 10;
        else if (!lostChannels.includes(w.channel)) lostChannels = [...lostChannels, w.channel];
      }),
      await events.recordingLive(() => (liveTranscribing = true)),
      // Tanda dari tombol bintang maupun shortcut global.
      await events.recordingBookmark((b) => {
        bookmarkCount = b.count;
        bookmarkFlash = true;
        clearTimeout(flashTimer);
        flashTimer = setTimeout(() => (bookmarkFlash = false), 900);
      }),
      await events.autoStopWarning((w) => {
        autoStopReason = w.reason;
        if (w.silenceMin) silenceMin = w.silenceMin;
        autoStopDeadline = performance.now() + w.secondsLeft * 1000;
      }),
    );
    try {
      applyState(await api.getRecordingState());
    } catch {
      /* abaikan */
    }
  });

  onDestroy(() => {
    clearInterval(timer);
    unlisten.forEach((u) => u());
  });

  async function run(action: () => Promise<unknown>) {
    busy = true;
    error = null;
    try {
      await action();
    } catch (e) {
      error = (e as AppError).message;
    } finally {
      busy = false;
    }
  }

  const togglePause = () =>
    run(async () => applyState(rs.status === "paused" ? await api.resumeRecording() : await api.pauseRecording()));
  const toggleMute = () => run(async () => applyState(await api.setMicMuted(!rs.micMuted)));
  const stop = () =>
    run(async () => {
      const res = await api.stopRecording();
      if (!res.meetingId) sendNotification({ title: id.appName, body: id.toast.tooShort });
    });
  const respondAutoStop = (cont: boolean) =>
    run(async () => {
      autoStopDeadline = null;
      await api.respondAutoStop(cont);
    });

  async function focusMain() {
    const main = await WebviewWindow.getByLabel("main");
    if (!main) return;
    await main.unminimize();
    await main.show();
    await main.setFocus();
  }
</script>

<div class="on-dark flex min-h-screen select-none flex-col bg-ink text-white" bind:this={root}>
  <div data-tauri-drag-region class="flex h-16 shrink-0 items-center gap-2.5 px-3">
    <span
      class={[
        "h-3 w-3 shrink-0 rounded-full",
        rs.status === "paused" ? "bg-warn-bright" : "bg-rec motion-safe:animate-pulse",
      ]}
      aria-hidden="true"
    ></span>
    <button
      type="button"
      class="tabular flex flex-col items-start rounded px-1 text-base font-semibold hover:bg-white/10"
      title={id.recorder.openMain}
      onclick={focusMain}
    >
      {formatTimestamp(elapsed)}
      {#if rs.status === "paused"}
        <span class="text-2xs leading-none font-medium text-warn-bright">{id.recorder.paused}</span>
      {:else if liveTranscribing}
        <span class="text-2xs leading-none font-medium text-white/60">{id.recorder.liveTranscribing}</span>
      {/if}
    </button>

    <!-- Meter berlabel ikon (mic = suara Anda, speaker = audio komputer), bukan hanya warna. -->
    <div data-tauri-drag-region class="flex min-w-0 flex-1 flex-col gap-1.5">
      <div data-tauri-drag-region class="flex items-center gap-1.5" title={id.recorder.mic}>
        <Icon name="mic" size={11} class="shrink-0 text-white/60" />
        <span class="sr-only">{id.recorder.mic}</span>
        <div data-tauri-drag-region class="h-1.5 flex-1 overflow-hidden rounded-full bg-white/12" aria-hidden="true">
          <div class="h-full rounded-full bg-mic-bright transition-[width] duration-100" style:width={meterWidth(micDb)}></div>
        </div>
      </div>
      <div data-tauri-drag-region class="flex items-center gap-1.5" title={id.recorder.system}>
        <Icon name="speaker" size={11} class="shrink-0 text-white/60" />
        <span class="sr-only">{id.recorder.system}</span>
        <div data-tauri-drag-region class="h-1.5 flex-1 overflow-hidden rounded-full bg-white/12" aria-hidden="true">
          <div class="h-full rounded-full bg-system-bright transition-[width] duration-100" style:width={meterWidth(sysDb)}></div>
        </div>
      </div>
    </div>

    <button
      type="button"
      class={["relative rounded p-1.5 hover:bg-white/15 disabled:opacity-50", bookmarkFlash && "bg-white/20"]}
      title={bookmarkCount > 0 ? `${id.recorder.bookmark} · ${id.recorder.bookmarked(bookmarkCount)}` : id.recorder.bookmark}
      aria-label={id.recorder.bookmark}
      disabled={busy || rs.status !== "recording"}
      onclick={() => api.addBookmark().catch((e: AppError) => (error = e.message))}
    >
      <Icon name="star" class={bookmarkCount > 0 ? "text-amber-300" : ""} />
      {#if bookmarkCount > 0}
        <span class="tabular absolute -top-0.5 -right-0.5 min-w-3.5 rounded-full bg-white px-0.5 text-center text-[0.625rem] leading-3.5 font-bold text-ink">{bookmarkCount}</span>
      {/if}
    </button>
    <button
      type="button"
      class="rounded p-1.5 hover:bg-white/15 disabled:opacity-50"
      title={rs.status === "paused" ? id.recorder.resume : id.recorder.pause}
      aria-label={rs.status === "paused" ? id.recorder.resume : id.recorder.pause}
      disabled={busy}
      onclick={togglePause}
    >
      <Icon name={rs.status === "paused" ? "play" : "pause"} />
    </button>
    <button
      type="button"
      class={["rounded p-1.5 hover:bg-white/15 disabled:opacity-50", rs.micMuted && "bg-rec/70"]}
      title={rs.micMuted ? id.recorder.unmute : id.recorder.mute}
      aria-label={rs.micMuted ? id.recorder.unmute : id.recorder.mute}
      aria-pressed={rs.micMuted}
      disabled={busy}
      onclick={toggleMute}
    >
      <Icon name={rs.micMuted ? "mic-off" : "mic"} />
    </button>
    <button
      type="button"
      class="rounded p-1.5 hover:bg-white/15 disabled:opacity-50"
      title={id.recorder.stop}
      aria-label={id.recorder.stop}
      disabled={busy}
      onclick={stop}
    >
      <Icon name="stop" size={16} class="text-rec-bright" />
    </button>
  </div>

  {#each lostChannels as ch (ch)}
    <div class="flex min-h-11 items-center border-t border-white/10 bg-warn-deep px-3 py-2 text-xs leading-snug text-warn-deep-text">
      {id.recorder.deviceLost(ch)}
    </div>
  {/each}

  {#if limitSoonMin !== null}
    <div class="flex min-h-11 items-center border-t border-white/10 px-3 py-2 text-xs leading-snug text-white/85">
      {id.recorder.limitSoon(limitSoonMin)}
    </div>
  {/if}

  {#if systemSilent && !lostChannels.includes("system")}
    <div class="flex min-h-11 items-center border-t border-white/10 bg-warn-deep px-3 py-2 text-xs leading-snug text-warn-deep-text">
      {id.recorder.systemSilent(systemSilentMin)}
    </div>
  {/if}

  {#if autoStopDeadline !== null}
    <div class="flex min-h-11 items-center gap-2 border-t border-white/10 px-3 py-2 text-xs">
      <span class="flex-1 leading-snug">
        {autoStopReason === "meeting_ended" ? id.recorder.meetingEnded : id.recorder.autoStop(silenceMin)}<br /><span
          class="tabular text-white/60">{id.recorder.autoStopCountdown(secondsLeft)}</span
        >
      </span>
      <button type="button" class="rounded-md bg-rec px-2 py-1 font-semibold" onclick={() => respondAutoStop(false)}>
        {id.recorder.autoStopStop}
      </button>
      <button type="button" class="rounded-md bg-white/15 px-2 py-1 font-semibold" onclick={() => respondAutoStop(true)}>
        {id.recorder.autoStopContinue}
      </button>
    </div>
  {/if}

  {#if error}
    <div class="flex min-h-11 items-center border-t border-white/10 bg-bad-deep px-3 py-2 text-xs leading-snug" role="alert">{error}</div>
  {/if}
</div>
