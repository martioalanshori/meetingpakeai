<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import RecordButton from "$lib/components/RecordButton.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import { formatDateTime, formatDuration } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, MeetingListItem } from "$lib/types";

  const PAGE = 50;

  let items = $state<MeetingListItem[]>([]);
  let loaded = $state(false);
  let hasMore = $state(false);

  const interrupted = $derived(items.filter((m) => m.status === "interrupted"));
  const queuePaused = $derived(
    items.some((m) => m.status === "failed" && (m.errorCode === "INVALID_API_KEY" || m.errorCode === "NO_API_KEY")),
  );

  /** Muat ulang semua item yang sedang tampil (minimal satu halaman). */
  async function reload() {
    try {
      const limit = Math.max(PAGE, items.length);
      const rows = await api.listMeetings(limit, 0);
      items = rows;
      hasMore = rows.length === limit;
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      loaded = true;
    }
  }

  async function loadMore() {
    try {
      const rows = await api.listMeetings(PAGE, items.length);
      items = [...items, ...rows];
      hasMore = rows.length === PAGE;
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function resolve(meetingId: string, action: "process" | "discard") {
    try {
      await api.resolveInterrupted(meetingId, action);
      if (action === "discard") showToast(id.toast.deleted);
      await reload();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
    await reload();
    unlisten.push(
      // Progres real-time tanpa refresh (AC F6.1).
      await events.jobProgress((p) => {
        const m = items.find((x) => x.id === p.meetingId);
        if (m) {
          m.status = p.status;
          m.progressDone = p.progressDone;
          m.progressTotal = p.progressTotal;
        }
      }),
      await events.meetingUpdated(() => reload()),
      await events.recordingState(() => reload()),
    );
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<main class="mx-auto flex min-h-full max-w-4xl flex-col gap-5 p-6">
  <header class="flex items-center justify-between gap-4">
    <h1 class="text-xl font-semibold">{id.appName}</h1>
    <div class="flex items-center gap-2">
      <RecordButton />
      <a
        href="/settings"
        class="rounded-lg px-3 py-2.5 text-lg text-gray-700 hover:bg-gray-200"
        aria-label={id.home.settings}
        title={id.home.settings}
      >
        ⚙
      </a>
    </div>
  </header>

  {#if queuePaused}
    <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl border border-red-200 bg-red-50 p-4 text-red-900">
      <span class="flex-1">{id.home.queuePaused}</span>
      <a href="/settings" class="rounded-lg bg-red-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-800">
        {id.home.openSettings}
      </a>
    </div>
  {/if}

  {#each interrupted as m (m.id)}
    <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl border border-amber-200 bg-amber-50 p-4 text-amber-950">
      <span class="flex-1">{id.home.interrupted(m.title)}</span>
      <button
        type="button"
        class="rounded-lg bg-amber-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-800"
        onclick={() => resolve(m.id, "process")}
      >
        {id.home.interruptedProcess}
      </button>
      <button
        type="button"
        class="rounded-lg px-3 py-1.5 text-sm text-amber-900 hover:bg-amber-100"
        onclick={() => resolve(m.id, "discard")}
      >
        {id.home.interruptedDiscard}
      </button>
    </div>
  {/each}

  {#if !loaded}
    <p class="text-gray-500">{id.common.loading}</p>
  {:else if items.length === 0}
    <section
      class="flex flex-1 items-center justify-center rounded-xl border border-dashed border-gray-300 bg-white p-10 text-center text-gray-600"
    >
      {id.home.empty}
    </section>
  {:else}
    <ul class="flex flex-col divide-y divide-gray-200 overflow-hidden rounded-xl border border-gray-200 bg-white">
      {#each items as m (m.id)}
        <li>
          <a href={`/meeting/${m.id}`} class="flex items-center gap-4 px-4 py-3 hover:bg-gray-50 focus-visible:bg-gray-50">
            <div class="flex min-w-0 flex-1 flex-col">
              <span class="truncate font-medium">{m.title}</span>
              <span class="text-sm text-gray-500">
                {formatDateTime(m.startedAt)}{#if m.durationMs > 0}&nbsp;· {formatDuration(m.durationMs)}{/if}
              </span>
            </div>
            <StatusBadge status={m.status} progressDone={m.progressDone} progressTotal={m.progressTotal} />
          </a>
        </li>
      {/each}
    </ul>
    {#if hasMore}
      <button type="button" class="self-center rounded-lg px-4 py-2 text-indigo-700 hover:bg-indigo-50" onclick={loadMore}>
        {id.home.loadMore}
      </button>
    {/if}
  {/if}
</main>
