<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import RecordButton from "$lib/components/RecordButton.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import { formatDateTime, formatDuration } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import Highlight from "$lib/components/Highlight.svelte";
  import { formatTimestamp } from "$lib/format";
  import type { AppError, MeetingListItem, SearchHit } from "$lib/types";

  const PAGE = 50;

  let items = $state<MeetingListItem[]>([]);
  let loaded = $state(false);
  let hasMore = $state(false);

  let queuePaused = $state(false);
  let query = $state("");
  let hits = $state<SearchHit[] | null>(null);
  let searchTimer: ReturnType<typeof setTimeout> | undefined;

  const interrupted = $derived(items.filter((m) => m.status === "interrupted"));

  /** Status dari worker (A9): tidak bergantung pada halaman daftar yang sudah dimuat. */
  async function refreshQueuePaused() {
    queuePaused = await api.getOnboardingStatus().then((s) => s.queuePaused, () => false);
  }

  function onSearchInput() {
    clearTimeout(searchTimer);
    searchTimer = setTimeout(runSearch, 250);
  }

  async function runSearch() {
    const q = query.trim();
    if (q === "") {
      hits = null;
      return;
    }
    try {
      hits = await api.searchMeetings(q);
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  function hitHref(h: SearchHit): string {
    const tab = h.kind === "transcript" ? "transcript" : h.kind === "action" ? "actions" : "summary";
    return `/meeting/${h.meetingId}?tab=${tab}`;
  }

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

  // Pause/mute juga memicu recording://state; daftar hanya berubah saat mulai/berhenti merekam.
  let lastRecordingStatus = "idle";

  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
    await reload();
    refreshQueuePaused();
    lastRecordingStatus = await api.getRecordingState().then((r) => r.status, () => "idle");
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
      await events.meetingUpdated(() => {
        reload();
        refreshQueuePaused();
        if (query.trim() !== "") runSearch();
      }),
      await events.recordingState((s) => {
        const wasIdle = lastRecordingStatus === "idle";
        lastRecordingStatus = s.status;
        if (wasIdle !== (s.status === "idle")) reload();
      }),
    );
  });
  onDestroy(() => {
    clearTimeout(searchTimer);
    unlisten.forEach((u) => u());
  });
</script>

<main class="mx-auto flex min-h-full max-w-4xl flex-col gap-5 p-6">
  <header class="flex items-center justify-between gap-4">
    <h1 class="text-xl font-semibold">{id.appName}</h1>
    <div class="flex items-center gap-2">
      <RecordButton />
      <a href="/tasks" class="rounded-lg px-3 py-2.5 text-gray-700 hover:bg-gray-200">{id.home.tasks}</a>
      <a
        href="/settings"
        class="rounded-lg p-2.5 text-gray-700 hover:bg-gray-200"
        aria-label={id.home.settings}
        title={id.home.settings}
      >
        <Icon name="settings" size={20} />
      </a>
    </div>
  </header>

  <label class="relative flex items-center">
    <span class="sr-only">{id.home.searchLabel}</span>
    <Icon name="search" size={18} class="pointer-events-none absolute left-3 text-gray-400" />
    <input
      type="search"
      class="w-full rounded-xl border border-gray-300 bg-white py-2.5 pr-3 pl-10"
      placeholder={id.home.searchPlaceholder}
      bind:value={query}
      oninput={onSearchInput}
    />
  </label>

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

  {#if hits !== null}
    {#if hits.length === 0}
      <p class="text-gray-600">{id.home.noResults}</p>
    {:else}
      <ul class="flex flex-col divide-y divide-gray-200 overflow-hidden rounded-xl border border-gray-200 bg-white">
        {#each hits as h, i (i)}
          <li>
            <a href={hitHref(h)} class="flex flex-col gap-0.5 px-4 py-3 hover:bg-gray-50">
              <span class="text-sm text-gray-500">
                {h.title} · {formatDateTime(h.startedAt)} · {id.home.hitKind[h.kind]}{#if h.startMs !== null}
                  [{formatTimestamp(h.startMs)}]{/if}
              </span>
              <span class="text-gray-900"><Highlight text={h.snippet} /></span>
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if !loaded}
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
