<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import { dayLabel, formatDateTime, formatDuration, formatTime } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import Highlight from "$lib/components/Highlight.svelte";
  import MeetingDetail from "$lib/components/MeetingDetail.svelte";
  import { viewport } from "$lib/viewport.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
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

  function hitTab(h: SearchHit): string {
    return h.kind === "transcript" ? "transcript" : h.kind === "action" ? "actions" : "summary";
  }

  function hitHref(h: SearchHit): string {
    return `/meeting/${h.meetingId}?tab=${hitTab(h)}`;
  }

  // Layar lebar: meeting dibuka di panel kanan (URL `/?m=<id>`), bukan pindah halaman.
  const selected = $derived(page.url.searchParams.get("m"));
  const selectedTab = $derived(page.url.searchParams.get("tab"));

  function select(meetingId: string, tab?: string) {
    goto(`/?m=${meetingId}${tab ? `&tab=${tab}` : ""}`, { replaceState: true, keepFocus: true, noScroll: true });
  }

  function openRow(e: MouseEvent, meetingId: string) {
    if (!viewport.wide) return;
    e.preventDefault();
    select(meetingId);
  }

  function openHit(e: MouseEvent, h: SearchHit) {
    if (!viewport.wide) return;
    e.preventDefault();
    select(h.meetingId, hitTab(h));
  }

  // Panel kanan tidak dibiarkan kosong: pilih meeting terbaru.
  $effect(() => {
    if (viewport.wide && !selected && items.length > 0) untrack(() => select(items[0].id));
  });

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
    if (action === "discard") {
      const ok = await confirmDialog({
        title: id.home.discardTitle,
        message: id.home.discardMessage,
        confirmText: id.home.interruptedDiscard,
        danger: true,
      });
      if (!ok) return;
    }
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

{#snippet listBody(compact: boolean)}
  <div class="flex flex-col gap-4">
    <h1 class="text-2xl font-bold tracking-[-0.02em]">{id.nav.meetings}</h1>
    <label class="relative flex items-center">
      <span class="sr-only">{id.home.searchLabel}</span>
      <Icon name="search" size={18} class="pointer-events-none absolute left-3.5 text-ink-faint" />
      <input
        type="search"
        class="field w-full py-2.5 pl-10"
        placeholder={compact ? id.home.searchPlaceholderShort : id.home.searchPlaceholder}
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
      <button type="button" class="btn btn-danger" onclick={() => resolve(m.id, "discard")}>
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
            <a
              href={hitHref(h)}
              onclick={(e) => openHit(e, h)}
              class={[
                "-mx-3 flex flex-col gap-1 rounded-lg px-3 py-3",
                h.meetingId === selected && viewport.wide ? "bg-sheet shadow-[inset_0_0_0_1px_var(--color-line)]" : "hover:bg-sheet",
              ]}
            >
              <span class="flex flex-wrap items-baseline gap-x-3 text-sm text-ink-soft">
                <span class="font-semibold text-ink">{h.title}</span>
                <span class="tabular">{formatDateTime(h.startedAt)}</span>
                <span
                  >{id.home.hitKind[h.kind]}{#if h.startMs !== null}&nbsp;<span class="tabular">{formatTimestamp(h.startMs)}</span
                    >{/if}</span
                >
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
    <div class="flex flex-col gap-5">
      {#each groups as g (g.label)}
        <section aria-label={g.label}>
          <h2 class="sticky top-0 z-10 -mx-3 bg-paper/95 px-3 py-1.5 text-sm font-semibold text-ink-soft backdrop-blur-sm">
            {g.label}
          </h2>
          <ul class="flex flex-col">
            {#each g.items as m (m.id)}
              {@const active = viewport.wide && m.id === selected}
              <li>
                <a
                  href={`/meeting/${m.id}`}
                  onclick={(e) => openRow(e, m.id)}
                  aria-current={active ? "true" : undefined}
                  class={[
                    "-mx-3 grid items-baseline gap-x-4 rounded-lg px-3 py-3",
                    compact ? "grid-cols-[3rem_minmax(0,1fr)]" : "grid-cols-[3.5rem_minmax(0,1fr)_auto]",
                    active ? "bg-sheet shadow-[inset_0_0_0_1px_var(--color-line)]" : "hover:bg-sheet",
                  ]}
                >
                  <span class="tabular text-sm text-ink-soft">{formatTime(m.startedAt)}</span>
                  <span class="flex min-w-0 flex-col gap-1">
                    <span class="truncate font-semibold">{m.title}</span>
                    {#if compact}
                      <span class="flex flex-wrap items-center gap-x-3 gap-y-0.5">
                        {#if m.durationMs > 0}
                          <span class="tabular text-sm text-ink-faint">{formatDuration(m.durationMs)}</span>
                        {/if}
                        {#if m.status !== "done"}
                          <StatusBadge status={m.status} progressDone={m.progressDone} progressTotal={m.progressTotal} />
                        {/if}
                      </span>
                    {:else if m.durationMs > 0}
                      <span class="tabular text-sm text-ink-faint">{formatDuration(m.durationMs)}</span>
                    {/if}
                  </span>
                  {#if !compact}
                    <StatusBadge status={m.status} progressDone={m.progressDone} progressTotal={m.progressTotal} />
                  {/if}
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
{/snippet}

{#if viewport.wide}
  <!-- Layar lebar: daftar di kiri, notulen meeting terpilih di kanan; tiap panel bergulir sendiri. -->
  <div class="grid h-full grid-cols-[minmax(20rem,26rem)_minmax(0,1fr)] print:block">
    <div class="flex min-h-0 flex-col gap-5 overflow-y-auto border-r border-line px-6 pt-7 pb-10 print:hidden">
      {@render listBody(true)}
    </div>
    <div class="min-h-0 overflow-y-auto print:overflow-visible">
      {#if selected}
        <MeetingDetail meetingId={selected} initialTab={selectedTab} embedded ondeleted={() => goto("/", { replaceState: true })} />
      {:else if loaded}
        <div class="flex h-full items-center justify-center px-10 text-center text-ink-soft">
          <p class="max-w-sm">{items.length === 0 ? id.home.empty : id.home.pickMeeting}</p>
        </div>
      {/if}
    </div>
  </div>
{:else}
  <main class="flex w-full max-w-3xl flex-col gap-6 px-6 pt-7 pb-12 lg:px-10">
    {@render listBody(false)}
  </main>
{/if}
