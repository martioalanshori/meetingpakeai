<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import { dayLabel, formatDateTime, formatDuration, formatTime } from "$lib/format";
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

  /** Kelompok per hari kalender lokal, urutan daftar dipertahankan. */
  const groups = $derived.by(() => {
    const out: { label: string; items: MeetingListItem[] }[] = [];
    for (const m of items) {
      const label = dayLabel(m.startedAt);
      const last = out.at(-1);
      if (last && last.label === label) last.items.push(m);
      else out.push({ label, items: [m] });
    }
    return out;
  });

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

<main class="mx-auto flex w-full max-w-3xl flex-col gap-6 px-8 pt-7 pb-12">
  <div class="flex flex-col gap-4">
    <h1 class="text-2xl font-bold tracking-[-0.02em]">{id.nav.meetings}</h1>
    <label class="relative flex items-center">
      <span class="sr-only">{id.home.searchLabel}</span>
      <Icon name="search" size={18} class="pointer-events-none absolute left-3.5 text-ink-faint" />
      <input
        type="search"
        class="field w-full py-2.5 pl-10"
        placeholder={id.home.searchPlaceholder}
        bind:value={query}
        oninput={onSearchInput}
      />
    </label>
  </div>

  {#if queuePaused}
    <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-bad-wash px-4 py-3 text-bad">
      <span class="flex-1 text-sm font-medium">{id.home.queuePaused}</span>
      <a href="/settings" class="btn btn-ink">{id.home.openSettings}</a>
    </div>
  {/if}

  {#each interrupted as m (m.id)}
    <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-warn-wash px-4 py-3 text-warn">
      <span class="flex-1 text-sm font-medium">{id.home.interrupted(m.title)}</span>
      <button type="button" class="btn btn-ink" onclick={() => resolve(m.id, "process")}>
        {id.home.interruptedProcess}
      </button>
      <button type="button" class="btn btn-quiet" onclick={() => resolve(m.id, "discard")}>
        {id.home.interruptedDiscard}
      </button>
    </div>
  {/each}

  {#if hits !== null}
    {#if hits.length === 0}
      <p class="text-ink-soft">{id.home.noResults}</p>
    {:else}
      <ul class="flex flex-col">
        {#each hits as h, i (i)}
          <li class="border-b border-line-soft last:border-b-0">
            <a href={hitHref(h)} class="-mx-3 flex flex-col gap-1 rounded-lg px-3 py-3 hover:bg-sheet">
              <span class="flex flex-wrap items-baseline gap-x-3 text-sm text-ink-soft">
                <span class="font-semibold text-ink">{h.title}</span>
                <span class="tabular">{formatDateTime(h.startedAt)}</span>
                <span>{id.home.hitKind[h.kind]}{#if h.startMs !== null}&nbsp;<span class="tabular">{formatTimestamp(h.startMs)}</span>{/if}</span>
              </span>
              <span class="leading-relaxed"><Highlight text={h.snippet} /></span>
            </a>
          </li>
        {/each}
      </ul>
    {/if}
  {:else if !loaded}
    <p class="text-ink-soft">{id.common.loading}</p>
  {:else if items.length === 0}
    <section class="flex flex-col items-start gap-2 rounded-xl border border-dashed border-line px-6 py-10">
      <p class="text-lg font-semibold">{id.home.emptyTitle}</p>
      <p class="max-w-prose text-ink-soft">{id.home.empty}</p>
    </section>
  {:else}
    <div class="flex flex-col gap-6">
      {#each groups as g (g.label)}
        <section aria-label={g.label}>
          <h2 class="sticky top-0 z-10 -mx-3 bg-paper/95 px-3 py-1.5 text-sm font-semibold text-ink-soft backdrop-blur-sm">
            {g.label}
          </h2>
          <ul class="flex flex-col">
            {#each g.items as m (m.id)}
              <li>
                <a
                  href={`/meeting/${m.id}`}
                  class="-mx-3 grid grid-cols-[3.5rem_1fr_auto] items-baseline gap-x-4 rounded-lg px-3 py-3 hover:bg-sheet"
                >
                  <span class="tabular text-sm text-ink-soft">{formatTime(m.startedAt)}</span>
                  <span class="flex min-w-0 flex-col gap-0.5">
                    <span class="truncate font-semibold">{m.title}</span>
                    {#if m.durationMs > 0}
                      <span class="tabular text-sm text-ink-faint">{formatDuration(m.durationMs)}</span>
                    {/if}
                  </span>
                  <StatusBadge status={m.status} progressDone={m.progressDone} progressTotal={m.progressTotal} />
                </a>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>
    {#if hasMore}
      <button type="button" class="btn btn-line self-start" onclick={loadMore}>{id.home.loadMore}</button>
    {/if}
  {/if}
</main>
