<script lang="ts">
  // Kartu setelah Stop (langkah 48, feedback3 F1): "Menyusun notulen…" → "Notulen siap · Buka".
  // Menggantikan widget rekaman yang langsung hilang; tutup otomatis 15 dtk setelah selesai.
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { page } from "$app/state";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import type { MeetingStatus } from "$lib/types";

  const t = id.doneCard;
  const meetingId = page.url.searchParams.get("id") ?? "";
  const CLOSE_AFTER_DONE_MS = 15_000;

  let status = $state<MeetingStatus>("queued");
  let title = $state("");
  let pct = $state<number | null>(null);
  let closeTimer: ReturnType<typeof setTimeout> | undefined;
  const unlisten: UnlistenFn[] = [];

  const done = $derived(status === "done");
  const failed = $derived(status === "failed");

  function close() {
    getCurrentWindow().close().catch(() => {});
  }

  async function refresh() {
    try {
      const m = await api.getMeeting(meetingId);
      status = m.status;
      title = m.title;
      if ((m.status === "done" || m.status === "failed") && closeTimer === undefined) {
        closeTimer = setTimeout(close, CLOSE_AFTER_DONE_MS);
      }
    } catch {
      close();
    }
  }

  async function open() {
    await api.openMeetingInMain(meetingId).catch(() => {});
    close();
  }

  onMount(async () => {
    unlisten.push(
      await events.jobProgress((p) => {
        if (p.meetingId !== meetingId) return;
        status = p.status;
        pct = p.progressTotal > 0 ? Math.round((p.progressDone / p.progressTotal) * 100) : null;
      }),
      await events.meetingUpdated((p) => {
        if (p.meetingId === meetingId || p.meetingId === "") refresh();
      }),
    );
    refresh();
  });
  onDestroy(() => {
    unlisten.forEach((u) => u());
    clearTimeout(closeTimer);
  });
</script>

<div class="on-dark flex h-screen select-none flex-col justify-center gap-2 bg-ink px-3 text-white" data-tauri-drag-region>
  <div class="flex items-center gap-2.5" data-tauri-drag-region>
    {#if done}
      <Icon name="check-circle" size={18} class="shrink-0 text-ok-bright" />
    {:else if failed}
      <Icon name="alert-circle" size={18} class="shrink-0 text-bad-bright" />
    {:else}
      <Icon name="refresh" size={16} class="shrink-0 text-white/70 motion-safe:animate-spin" />
    {/if}
    <div class="flex min-w-0 flex-1 flex-col" data-tauri-drag-region>
      <span class="truncate text-sm font-semibold" role="status" data-tauri-drag-region>
        {done ? t.ready : failed ? t.failed : t.working}{#if !done && !failed && pct !== null}<span class="tabular text-white/60"
            >&ensp;{pct}%</span
          >{/if}
      </span>
      <span class="truncate text-sm text-white/70" data-tauri-drag-region>{done || failed ? title : t.workingHint}</span>
    </div>
    <button
      type="button"
      class="rounded p-1 text-white/70 hover:bg-white/15 hover:text-white"
      aria-label={id.common.close}
      title={id.common.close}
      onclick={close}
    >
      <Icon name="x" size={16} />
    </button>
  </div>
  {#if done || failed}
    <button
      type="button"
      class="flex items-center justify-center gap-1.5 rounded-md bg-white px-3 py-1.5 text-sm font-semibold text-ink hover:bg-white/90"
      onclick={open}
    >
      {t.open}
    </button>
  {/if}
</div>
