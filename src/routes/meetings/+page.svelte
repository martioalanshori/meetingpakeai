<script lang="ts">
  import { onDestroy, onMount, untrack } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWebview } from "@tauri-apps/api/webview";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import { dayLabel, formatDateTime, formatDuration, formatTime } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import Highlight from "$lib/components/Highlight.svelte";
  import MeetingDetail from "$lib/components/MeetingDetail.svelte";
  import { detailTab, viewport } from "$lib/viewport.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import { formatTimestamp } from "$lib/format";
  import type { AppError, AskAllResult, MeetingListItem, SearchHit } from "$lib/types";

  const PAGE = 50;

  let items = $state<MeetingListItem[]>([]);
  let loaded = $state(false);
  let hasMore = $state(false);
  let loadingMore = $state(false);
  let loadError = $state<string | null>(null);

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
    return `/meeting/${h.meetingId}?tab=${hitTab(h)}${h.startMs !== null ? `&at=${h.startMs}` : ""}`;
  }

  // Tanya semua meeting (feedback3 D1): jawaban AI dari index pencarian.
  let askResult = $state<AskAllResult | null>(null);
  let askBusy = $state(false);
  let askedFor = $state("");

  async function askAll() {
    const q = query.trim();
    if (!q || askBusy) return;
    askBusy = true;
    askedFor = q;
    askResult = null;
    try {
      askResult = await api.askAllMeetings(q);
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      askBusy = false;
    }
  }

  function openRef(e: MouseEvent, meetingId: string, atMs: number | null) {
    if (!viewport.wide) return;
    e.preventDefault();
    select(meetingId, atMs !== null ? "transcript" : undefined, atMs);
  }

  // Layar lebar: meeting dibuka di panel kanan (URL `/meetings?m=<id>`), bukan pindah halaman.
  const selected = $derived(page.url.searchParams.get("m"));
  const selectedTab = $derived(page.url.searchParams.get("tab"));
  const selectedAt = $derived(Number(page.url.searchParams.get("at") ?? "NaN"));

  function select(meetingId: string, tab?: string, at?: number | null) {
    const atPart = at !== null && at !== undefined ? `&at=${at}` : "";
    goto(`/meetings?m=${meetingId}${tab ? `&tab=${tab}` : ""}${atPart}`, { replaceState: true, keepFocus: true, noScroll: true });
  }

  function openRow(e: MouseEvent, meetingId: string) {
    if (!viewport.wide) return;
    e.preventDefault();
    select(meetingId);
  }

  function openHit(e: MouseEvent, h: SearchHit) {
    if (!viewport.wide) return;
    e.preventDefault();
    select(h.meetingId, hitTab(h), h.startMs);
  }

  // Panel kanan tidak dibiarkan kosong: pilih meeting terbaru.
  $effect(() => {
    if (viewport.wide && !selected && items.length > 0) untrack(() => select(items[0].id));
  });

  // Jendela diperkecil di bawah batas dua panel saat ada meeting terpilih → buka sebagai halaman sendiri.
  $effect(() => {
    if (!viewport.wide && selected) {
      const tab = detailTab.meetingId === selected ? detailTab.tab : selectedTab;
      untrack(() => goto(`/meeting/${selected}${tab ? `?tab=${tab}` : ""}`, { replaceState: true }));
    }
  });

  /** Muat ulang semua item yang sedang tampil (minimal satu halaman). */
  async function reload() {
    try {
      const limit = Math.max(PAGE, items.length);
      const rows = await api.listMeetings(limit, 0, activeTag);
      items = rows;
      hasMore = rows.length === limit;
      loadError = null;
    } catch (e) {
      if (!loaded) loadError = (e as AppError).message;
      else showToast((e as AppError).message, "error");
    } finally {
      loaded = true;
    }
  }

  async function loadMore() {
    if (loadingMore) return;
    loadingMore = true;
    try {
      const rows = await api.listMeetings(PAGE, items.length, activeTag);
      const seen = new Set(items.map((m) => m.id));
      items = [...items, ...rows.filter((m) => !seen.has(m.id))];
      hasMore = rows.length === PAGE;
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      loadingMore = false;
    }
  }

  /** Halaman berikutnya dimuat otomatis saat ujung daftar mendekati layar (tombol tetap ada sebagai cadangan). */
  function autoLoad(el: HTMLElement) {
    const io = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting) && hasMore && !loadingMore) loadMore();
    }, { rootMargin: "600px 0px" });
    io.observe(el);
    return () => io.disconnect();
  }

  // Ringkasan mingguan (feedback3 D5): kartu sampai ditutup untuk minggu itu.
  let week = $state<Awaited<ReturnType<typeof api.weeklyDigest>> | null>(null);
  let weekText = $state("");
  let weekBusy = $state(false);
  let weekHidden = $state(false);
  const WEEK_KEY = "weekly-digest-closed";

  async function loadWeek() {
    try {
      const w = await api.weeklyDigest();
      let closed: string | null = null;
      try {
        closed = localStorage.getItem(WEEK_KEY);
      } catch {
        /* penyimpanan tidak tersedia */
      }
      weekHidden = closed === w.weekStart;
      week = w;
    } catch {
      week = null;
    }
  }

  async function summarizeWeek() {
    weekBusy = true;
    try {
      weekText = await api.weeklySummaryText();
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      weekBusy = false;
    }
  }

  function closeWeek() {
    weekHidden = true;
    try {
      if (week) localStorage.setItem(WEEK_KEY, week.weekStart);
    } catch {
      /* abaikan */
    }
  }

  // Label proyek/klien (feedback3 D4): saring daftar.
  let allTags = $state<string[]>([]);
  let activeTag = $state<string | null>(null);

  async function loadTags() {
    allTags = await api.listTags().catch(() => []);
    if (activeTag && !allTags.includes(activeTag)) activeTag = null;
  }

  function pickTag(tag: string | null) {
    activeTag = tag;
    items = [];
    loaded = false;
    reload();
  }

  let importing = $state(false);
  let dragOver = $state(false);

  /** Impor file (dialog jika tanpa path); meeting baru langsung dibuka. */
  async function importFile(path?: string) {
    if (importing) return;
    importing = true;
    try {
      const meetingId = await api.importRecording(path);
      if (meetingId) {
        showToast(id.home.imported, "success");
        await reload();
        if (viewport.wide) select(meetingId);
        else goto(`/meeting/${meetingId}`);
      }
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      importing = false;
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
    // Seret-lepas file rekaman ke jendela → impor.
    unlisten.push(
      await getCurrentWebview().onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") dragOver = true;
        else if (p.type === "leave") dragOver = false;
        else if (p.type === "drop") {
          dragOver = false;
          if (p.paths.length > 0) importFile(p.paths[0]);
        }
      }),
    );
    await reload();
    loadTags();
    loadWeek();
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
        loadTags();
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
    <div class="flex items-center justify-between gap-3">
      <h1 class="text-2xl font-bold tracking-[-0.02em]">{id.nav.meetings}</h1>
      <!-- Impor rekaman yang sudah ada (feedback3 A1); file juga bisa diseret ke jendela. -->
      <button
        type="button"
        class="btn btn-line btn-sm"
        title={`${id.home.importButton} (${id.home.importHint})`}
        disabled={importing}
        onclick={() => importFile()}
      >
        <Icon name={importing ? "refresh" : "download"} size={14} class={importing ? "motion-safe:animate-spin" : ""} />
        {#if !compact}{importing ? id.home.importing : id.home.importButton}{/if}
      </button>
    </div>
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
    {#if allTags.length > 0}
      <div class="flex flex-wrap gap-1.5" role="group" aria-label={id.home.tagFilter}>
        {#each [null, ...allTags] as tag (tag ?? "")}
          <button
            type="button"
            class={[
              "rounded-full border px-2.5 py-0.5 text-sm",
              activeTag === tag ? "border-ink bg-ink-strong text-white" : "border-line text-ink-soft hover:border-ink-faint hover:text-ink",
            ]}
            aria-pressed={activeTag === tag}
            onclick={() => pickTag(tag)}>{tag ?? id.home.allTags}</button
          >
        {/each}
      </div>
    {/if}
  </div>

  {#if queuePaused}
    <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-bad-wash px-4 py-3 text-bad">
      <span class="flex-1 text-sm font-medium">{id.home.queuePaused}</span>
      <a href="/settings?tab=ai" class="btn btn-ink">{id.home.openSettings}</a>
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

  {#if week && week.meetings > 0 && !weekHidden && query.trim() === ""}
    <section class="flex flex-col gap-2 rounded-xl border border-line bg-sheet px-4 py-3.5" aria-label={id.home.weekTitle}>
      <div class="flex items-start gap-2">
        <div class="flex min-w-0 flex-1 flex-col gap-0.5">
          <span class="font-semibold">{id.home.weekTitle}</span>
          <span class="text-sm text-ink-soft">{id.home.weekStats(week.meetings, week.minutes, week.decisions)}</span>
          {#if week.openTasks > 0}
            <a href="/tasks" class={["text-sm", week.overdue > 0 ? "font-semibold text-bad" : "link"]}
              >{id.home.weekTasks(week.openTasks, week.overdue)}</a
            >
          {/if}
        </div>
        <button type="button" class="btn btn-quiet btn-icon btn-sm" aria-label={id.home.weekClose} title={id.home.weekClose} onclick={closeWeek}>
          <Icon name="x" size={14} />
        </button>
      </div>
      {#if weekText}
        <p class="text-sm leading-relaxed">{weekText}</p>
      {:else}
        <button type="button" class="btn btn-line btn-sm self-start" disabled={weekBusy} onclick={summarizeWeek}>
          {weekBusy ? id.home.weekBusy : id.home.weekSummarize}
        </button>
      {/if}
    </section>
  {/if}

  {#if query.trim().length >= 3}
    <!-- Tanya AI lintas meeting (feedback3 D1). -->
    <div class="flex flex-col gap-3">
      {#if askedFor !== query.trim() || (!askResult && !askBusy)}
        <button
          type="button"
          class="-mx-1 flex items-center gap-2 self-start rounded-lg px-2 py-1.5 text-left text-sm font-semibold hover:bg-wash"
          onclick={askAll}
        >
          <Icon name="send" size={14} class="shrink-0" />{id.home.askAll(query.trim())}
        </button>
      {/if}
      {#if askBusy}
        <p class="hint motion-safe:animate-pulse" role="status">{id.home.askAllBusy}</p>
      {:else if askResult && askedFor === query.trim()}
        <section class="flex flex-col gap-2 rounded-xl border border-line bg-sheet p-4" aria-label={id.home.askAllTitle}>
          <span class="text-sm font-semibold text-ink-soft">{id.home.askAllTitle}</span>
          <p class="leading-relaxed whitespace-pre-line">{askResult.answer}</p>
          {#if askResult.refs.length > 0}
            <ul class="flex flex-col gap-1 border-t border-line-soft pt-2">
              {#each askResult.refs as r, i (i)}
                <li>
                  <a
                    class="link text-sm"
                    href={`/meeting/${r.meetingId}${r.atMs !== null ? `?tab=transcript&at=${r.atMs}` : ""}`}
                    onclick={(e) => openRef(e, r.meetingId, r.atMs)}
                    >{r.title}<span class="tabular text-ink-faint"
                      >&ensp;{formatDateTime(r.startedAt)}{#if r.atMs !== null}&ensp;{formatTimestamp(r.atMs)}{/if}</span
                    ></a
                  >
                </li>
              {/each}
            </ul>
          {/if}
        </section>
      {/if}
    </div>
  {/if}

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
                h.meetingId === selected && viewport.wide
                  ? "bg-sheet shadow-[inset_0_0_0_1px_var(--color-line)]"
                  : compact
                    ? "hover:bg-sheet"
                    : "hover:bg-wash",
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
    <!-- Kerangka daftar (layout tidak melompat saat data datang). -->
    <ul class="flex flex-col gap-1 motion-safe:animate-pulse" aria-hidden="true">
      {#each [0, 1, 2, 3, 4] as i (i)}
        <li class="flex items-center gap-4 py-3">
          <div class="h-4 w-12 rounded bg-line-soft"></div>
          <div class="flex flex-1 flex-col gap-2">
            <div class="h-4 w-3/4 rounded bg-line-soft"></div>
            <div class="h-3 w-1/4 rounded bg-line-soft"></div>
          </div>
        </li>
      {/each}
    </ul>
  {:else if loadError}
    <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-bad-wash px-4 py-3 text-bad">
      <span class="flex-1 text-sm font-medium">{loadError}</span>
      <button type="button" class="btn btn-ink btn-sm" onclick={reload}>{id.common.retry}</button>
    </div>
  {:else if items.length === 0}
    <section class="flex flex-col items-start gap-2 rounded-xl border border-dashed border-line px-6 py-10">
      <p class="text-lg font-semibold">{id.home.emptyTitle}</p>
      <p class="max-w-prose text-ink-soft">{id.home.empty}</p>
    </section>
  {:else}
    <div class="flex flex-col gap-5">
      {#each groups as g (g.label)}
        <section aria-label={g.label}>
          <h2
            class={[
              "sticky top-0 z-10 -mx-3 px-3 py-1.5 text-sm font-semibold text-ink-soft backdrop-blur-sm",
              compact ? "bg-paper/95" : "bg-sheet/95",
            ]}
          >
            {g.label}
          </h2>
          <ul class="flex flex-col">
            {#each g.items as m (m.id)}
              {@const active = viewport.wide && m.id === selected}
              <li class="mrow -mx-3">
                <a
                  href={`/meeting/${m.id}`}
                  onclick={(e) => openRow(e, m.id)}
                  aria-current={active ? "true" : undefined}
                  class={[
                    "grid items-baseline gap-x-4 rounded-lg px-3 py-3",
                    compact ? "grid-cols-[3rem_minmax(0,1fr)]" : "grid-cols-[3.5rem_minmax(0,1fr)_auto]",
                    active
                      ? "bg-sheet shadow-[0_1px_2px_rgb(28_31_38/0.08),inset_0_0_0_1px_var(--color-line)]"
                      : compact
                        ? "hover:bg-sheet"
                        : "hover:bg-wash",
                  ]}
                >
                  <span class="tabular text-sm text-ink-soft">{formatTime(m.startedAt)}</span>
                  <span class="flex min-w-0 flex-col gap-1">
                    <span class="leading-snug font-semibold text-pretty wrap-anywhere">{m.title}</span>
                    {#if m.tags.length > 0}
                      <span class="flex flex-wrap gap-1">
                        {#each m.tags as tag (tag)}<span class="rounded-full bg-wash px-2 py-px text-xs text-ink-soft">{tag}</span>{/each}
                      </span>
                    {/if}
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
                  {#if !compact && m.status !== "done"}
                    <StatusBadge status={m.status} progressDone={m.progressDone} progressTotal={m.progressTotal} />
                  {:else if !compact}
                    <span></span>
                  {/if}
                </a>
              </li>
            {/each}
          </ul>
        </section>
      {/each}
    </div>
    {#if hasMore}
      <button type="button" class="btn btn-line self-start" disabled={loadingMore} onclick={loadMore} {@attach autoLoad}>
        {loadingMore ? id.common.loading : id.home.loadMore}
      </button>
    {/if}
  {/if}
{/snippet}

{#if dragOver}
  <div
    class="pointer-events-none fixed inset-3 z-50 flex items-center justify-center rounded-2xl border-2 border-dashed border-ink bg-sheet/90 text-lg font-semibold"
    role="status"
  >
    <Icon name="download" size={22} class="mr-2" />{id.home.dropHere}
  </div>
{/if}

{#if viewport.wide}
  <!-- Layar lebar: daftar di kiri, notulen meeting terpilih di kanan; tiap panel bergulir sendiri. -->
  <div class="grid h-full grid-cols-[minmax(17.5rem,24rem)_minmax(0,1fr)] print:block">
    <div class="flex min-h-0 flex-col gap-5 overflow-y-auto [scrollbar-gutter:stable] border-r border-line bg-paper px-5 pt-7 pb-10 print:hidden">
      {@render listBody(true)}
    </div>
    <div class="min-h-0 overflow-y-auto [scrollbar-gutter:stable] print:overflow-visible">
      {#if selected}
        <MeetingDetail
          meetingId={selected}
          initialTab={selectedTab}
          initialMs={Number.isFinite(selectedAt) ? selectedAt : null}
          embedded ondeleted={() => goto("/meetings", { replaceState: true })} />
      {:else if loaded}
        <div class="flex h-full items-center justify-center px-10 text-center text-ink-soft">
          <p class="max-w-sm">{items.length === 0 ? id.home.emptyRight : id.home.pickMeeting}</p>
        </div>
      {/if}
    </div>
  </div>
{:else}
  <main class="mx-auto flex w-full max-w-3xl flex-col gap-6 px-6 pt-7 pb-12 xl:px-10">
    {@render listBody(false)}
  </main>
{/if}

<style>
  /* Baris di luar layar tidak di-layout/di-render; tinggi baris hampir tetap. */
  .mrow {
    content-visibility: auto;
    contain-intrinsic-size: auto 4.25rem;
  }
</style>
