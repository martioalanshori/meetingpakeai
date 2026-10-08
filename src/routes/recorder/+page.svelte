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

  const BASE_HEIGHT = 64;
  const ROW_HEIGHT = 44;

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
  let autoStopDeadline = $state<number | null>(null);
  let autoStopReason = $state<"silence" | "meeting_ended">("silence");
  let busy = $state(false);
  let error = $state<string | null>(null);

  const elapsed = $derived(rs.status === "recording" ? baseMs + Math.max(0, now - baseAt) : baseMs);
  const secondsLeft = $derived(
    autoStopDeadline === null ? 0 : Math.max(0, Math.ceil((autoStopDeadline - now) / 1000)),
  );
  const extraRows = $derived(
    lostChannels.length + (systemSilent && !lostChannels.includes("system") ? 1 : 0) + (autoStopDeadline !== null ? 1 : 0) + (error ? 1 : 0),
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

  $effect(() => {
    const h = BASE_HEIGHT + extraRows * ROW_HEIGHT;
    getCurrentWindow().setSize(new LogicalSize(300, h)).catch(() => {});
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
        if (w.code === "system_silent") systemSilent = true;
        else if (w.code === "system_ok") systemSilent = false;
        else if (!lostChannels.includes(w.channel)) lostChannels = [...lostChannels, w.channel];
      }),
      await events.autoStopWarning((w) => {
        autoStopReason = w.reason;
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

<div class="flex h-screen select-none flex-col bg-gray-900 text-white">
  <div data-tauri-drag-region class="flex h-16 shrink-0 items-center gap-2 px-3">
    <span
      class={[
        "h-3 w-3 shrink-0 rounded-full",
        rs.status === "paused" ? "bg-yellow-400" : "animate-pulse bg-red-500",
      ]}
      aria-hidden="true"
    ></span>
    <button
      type="button"
      class="flex flex-col items-start rounded px-1 font-mono text-sm tabular-nums hover:bg-white/10"
      title={id.recorder.openMain}
      onclick={focusMain}
    >
      {formatTimestamp(elapsed)}
      {#if rs.status === "paused"}
        <span class="font-sans text-[10px] leading-none text-yellow-300">{id.recorder.paused}</span>
      {/if}
    </button>

    <div data-tauri-drag-region class="flex min-w-0 flex-1 flex-col gap-1.5" aria-hidden="true">
      <div data-tauri-drag-region class="h-1.5 overflow-hidden rounded bg-white/15" title={id.recorder.mic}>
        <div class="h-full bg-sky-400 transition-[width] duration-100" style:width={meterWidth(micDb)}></div>
      </div>
      <div data-tauri-drag-region class="h-1.5 overflow-hidden rounded bg-white/15" title={id.recorder.system}>
        <div class="h-full bg-emerald-400 transition-[width] duration-100" style:width={meterWidth(sysDb)}></div>
      </div>
    </div>

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
      class={["rounded p-1.5 hover:bg-white/15 disabled:opacity-50", rs.micMuted && "bg-red-600/60"]}
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
      <Icon name="stop" size={16} class="text-red-400" />
    </button>
  </div>

  {#each lostChannels as ch (ch)}
    <div class="flex h-11 items-center bg-amber-600/90 px-3 text-xs">{id.recorder.deviceLost(ch)}</div>
  {/each}

  {#if systemSilent && !lostChannels.includes("system")}
    <div class="flex h-11 items-center bg-amber-600/90 px-3 text-xs leading-tight">{id.recorder.systemSilent}</div>
  {/if}

  {#if autoStopDeadline !== null}
    <div class="flex h-11 items-center gap-2 bg-gray-800 px-3 text-xs">
      <span class="flex-1 leading-tight">
        {autoStopReason === "meeting_ended" ? id.recorder.meetingEnded : id.recorder.autoStop}<br /><span class="text-gray-400">{id.recorder.autoStopCountdown(secondsLeft)}</span>
      </span>
      <button type="button" class="rounded bg-red-600 px-2 py-1" onclick={() => respondAutoStop(false)}>
        {id.recorder.autoStopStop}
      </button>
      <button type="button" class="rounded bg-white/15 px-2 py-1" onclick={() => respondAutoStop(true)}>
        {id.recorder.autoStopContinue}
      </button>
    </div>
  {/if}

  {#if error}
    <div class="flex h-11 items-center bg-red-700 px-3 text-xs">{error}</div>
  {/if}
</div>
