<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import SummaryEditor from "$lib/components/SummaryEditor.svelte";
  import AudioPlayer from "$lib/components/AudioPlayer.svelte";
  import CopyButton from "$lib/components/CopyButton.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Menu, { type MenuEntry } from "$lib/components/Menu.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import { formatActionItems, formatMinutes, formatSummaryTab, formatTranscript } from "$lib/minutes";
  import { formatDateTime, formatDuration, formatTime, formatTimestamp } from "$lib/format";
  import { id as t } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type {
    AppError,
    MeetingDetail,
    MeetingStatus,
    SummaryEdit,
    TranscriptSegment,
  } from "$lib/types";

  type Tab = "summary" | "actions" | "transcript";

  // Dipakai sebagai halaman sendiri (/meeting/[id]) atau panel kanan Beranda di layar lebar.
  let {
    meetingId,
    initialTab = null,
    embedded = false,
    ondeleted,
  }: { meetingId: string; initialTab?: string | null; embedded?: boolean; ondeleted?: () => void } = $props();

  let meeting = $state<MeetingDetail | null>(null);
  let transcript = $state<TranscriptSegment[]>([]);
  let notFound = $state(false);
  let tab = $state<Tab>("summary");
  let editing = $state(false);
  let titleDraft = $state("");
  let titleInput = $state<HTMLInputElement | null>(null);
  let editingSummary = $state(false);
  let audioSrc = $state<string | null>(null);
  let player = $state<AudioPlayer | null>(null);
  let audioLoading = $state(false);
  /** Posisi pemutaran (ms) untuk menyorot kalimat yang sedang diputar. */
  let playMs = $state<number | null>(null);
  /** Ikuti kalimat aktif dengan auto-scroll; berhenti sementara saat pengguna menggulir sendiri. */
  let followUntil = 0;

  const PROCESSING: MeetingStatus[] = [
    "queued",
    "preprocessing",
    "transcribing",
    "merging",
    "summarizing",
    "waiting_quota",
    "waiting_network",
  ];
  const processing = $derived(meeting !== null && PROCESSING.includes(meeting.status));
  const progressPct = $derived(
    meeting && meeting.progressTotal > 0 ? Math.round((meeting.progressDone / meeting.progressTotal) * 100) : 0,
  );
  const canRegenerate = $derived(
    meeting !== null &&
      (meeting.status === "done" || (meeting.status === "failed" && meeting.failedStep === "summarizing")),
  );
  const canRetranscribe = $derived(
    meeting !== null &&
      !meeting.audioDeleted &&
      ["done", "failed", "waiting_quota", "waiting_network"].includes(meeting.status),
  );

  /** Isi lama diredupkan selama meeting baru dimuat (tanpa kedipan "Memuat…"). */
  let switching = $state(false);

  /** `fresh` = meeting baru dibuka: pilih tab awal. Respons untuk meeting lain (klik cepat) diabaikan. */
  async function load(fresh = false) {
    const mid = meetingId;
    try {
      const m = await api.getMeeting(mid);
      const tr = await api.getTranscript(mid);
      if (mid !== meetingId) return;
      meeting = m;
      transcript = tr;
      notFound = false;
      if (fresh) {
        const wanted = initialTab;
        if (wanted === "summary" || wanted === "actions" || wanted === "transcript") tab = wanted;
        else if (m.status !== "done" && m.summary === null) tab = tr.length > 0 ? "transcript" : "summary";
        else tab = "summary";
      }
    } catch (e) {
      if (mid !== meetingId) return;
      if ((e as AppError).code === "NOT_FOUND") notFound = true;
      else showToast((e as AppError).message, "error");
    } finally {
      if (mid === meetingId) switching = false;
    }
  }

  const unlisten: UnlistenFn[] = [];
  // Meeting berganti (panel kanan) → kosongkan state lalu muat ulang.
  let loadedId = "";
  $effect(() => {
    const mid = meetingId;
    untrack(() => {
      if (mid === loadedId) return;
      loadedId = mid;
      player?.pause();
      playMs = null;
      switching = meeting !== null;
      editing = false;
      editingSummary = false;
      audioSrc = null;
      load(true);
    });
  });

  onMount(async () => {
    unlisten.push(
      await events.jobProgress((p) => {
        if (p.meetingId !== meetingId || !meeting) return;
        const changedStep = meeting.status !== p.status;
        meeting.status = p.status;
        meeting.progressDone = p.progressDone;
        meeting.progressTotal = p.progressTotal;
        if (changedStep) load();
      }),
      await events.meetingUpdated((p) => {
        if (p.meetingId === meetingId || p.meetingId === "") load();
      }),
    );
  });
  onDestroy(() => unlisten.forEach((u) => u()));

  async function act(action: () => Promise<unknown>, okText?: string) {
    try {
      await action();
      if (okText) showToast(okText, "success");
      await load();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function startEdit() {
    if (!meeting) return;
    titleDraft = meeting.title;
    editing = true;
    await tick();
    titleInput?.select();
  }

  async function saveTitle() {
    // Enter lalu blur memanggil ini dua kali; hanya yang pertama menyimpan.
    if (!meeting || !editing) return;
    const title = titleDraft.trim();
    editing = false;
    if (title === "" || title === meeting.title) return;
    await act(() => api.renameMeeting(meetingId, title), t.toast.titleSaved);
  }

  async function requestRegenerate() {
    if (meeting?.summary?.edited) {
      const ok = await confirmDialog({
        title: t.edit.regenerateTitle,
        message: t.edit.regenerateConfirm,
        confirmText: t.edit.regenerateButton,
      });
      if (!ok) return;
    }
    act(() => api.regenerateSummary(meetingId), t.toast.requeued);
  }

  async function requestDelete() {
    if (!meeting) return;
    const ok = await confirmDialog({
      title: t.detail.deleteTitle(meeting.title),
      message: t.detail.deleteConfirm,
      confirmText: t.detail.deleteButton,
      danger: true,
    });
    if (ok) confirmDelete();
  }

  /** Semua aksi sekunder dalam satu menu "Lainnya" (ekspor + aksi meeting). */
  const menuItems = $derived.by((): MenuEntry[] => {
    if (!meeting) return [];
    const m = meeting;
    const items: MenuEntry[] = [];
    if (m.summary) {
      items.push(
        { label: t.minutes.exportMd, icon: "download", onselect: () => exportFile("markdown") },
        { label: t.minutes.exportTxt, icon: "download", onselect: () => exportFile("text") },
        { label: t.minutes.print, icon: "printer", onselect: printMinutes },
        { separator: true },
      );
    }
    items.push(
      {
        label: t.detail.regenerate,
        icon: "refresh",
        disabled: !canRegenerate,
        hint: canRegenerate ? undefined : t.detail.regenerateUnavailable,
        onselect: requestRegenerate,
      },
      {
        label: t.detail.retranscribe,
        icon: "refresh",
        disabled: !canRetranscribe,
        hint: m.audioDeleted ? t.detail.audioDeleted : canRetranscribe ? undefined : t.detail.retranscribeUnavailable,
        onselect: () => act(() => api.retranscribe(meetingId), t.toast.requeued),
      },
      { separator: true },
      { label: t.detail.delete, icon: "trash", danger: true, disabled: m.status === "recording", onselect: requestDelete },
    );
    return items;
  });

  async function saveSummary(edit: SummaryEdit) {
    try {
      await api.updateSummary(meetingId, edit);
      editingSummary = false;
      showToast(t.edit.saved, "success");
      await load();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function confirmDelete() {
    try {
      await api.deleteMeeting(meetingId);
      showToast(t.toast.deleted);
      if (ondeleted) ondeleted();
      else await goto("/");
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function toggleItem(itemId: number, done: boolean) {
    try {
      await api.setActionItemDone(itemId, done);
    } catch (e) {
      showToast((e as AppError).message, "error");
      await load();
    }
  }

  /** `{judul}_{YYYY-MM-DD}.{ext}`; karakter ilegal Windows diganti `_` (PRD §14.7). */
  function exportName(ext: string): string {
    const d = new Date(meeting!.startedAt);
    const date = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
    const title = meeting!.title.replace(/[<>:"/\\|?*\u0000-\u001f]/g, "_").trim() || "Meeting";
    return `${title}_${date}.${ext}`;
  }

  async function exportFile(style: "markdown" | "text") {
    if (!meeting) return;
    try {
      const saved = await api.saveExport(exportName(style === "markdown" ? "md" : "txt"), formatMinutes(meeting, style, transcript));
      if (saved) showToast(t.minutes.exported, "success");
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function printMinutes() {
    await tick();
    window.print();
  }

  async function playAt(ms: number) {
    if (!meeting || meeting.audioDeleted) return;
    try {
      if (!audioSrc) {
        audioLoading = true;
        audioSrc = convertFileSrc(await api.preparePlayback(meetingId));
        await tick();
      }
      followUntil = 0;
      await player?.seekAndPlay(ms);
    } catch (e) {
      showToast((e as AppError).message ?? t.errors.INTERNAL, "error");
    } finally {
      audioLoading = false;
    }
  }


  /** Segment yang sedang diputar: `startMs` terbesar yang ≤ posisi (pencarian biner). */
  const activeSegId = $derived.by(() => {
    if (playMs === null || transcript.length === 0) return null;
    let lo = 0;
    let hi = transcript.length - 1;
    if (playMs < transcript[0].startMs) return null;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (transcript[mid].startMs <= playMs) lo = mid;
      else hi = mid - 1;
    }
    return transcript[lo].id;
  });

  $effect(() => {
    const segId = activeSegId;
    if (segId === null || Date.now() < followUntil) return;
    document.getElementById(`seg-${segId}`)?.scrollIntoView({ block: "nearest", behavior: "smooth" });
  });

  /** Teks saat notulen belum ada, sesuai status (bukan "—"). */
  const emptyNote = $derived(
    !meeting
      ? ""
      : processing
        ? t.detail.processing
        : meeting.status === "failed"
          ? t.detail.noSummaryFailed
          : meeting.status === "interrupted"
            ? t.detail.noSummaryInterrupted
            : t.detail.noSummary,
  );

  const tabs: { key: Tab; text: string }[] = [
    { key: "summary", text: t.detail.tabSummary },
    { key: "actions", text: t.detail.tabActionItems },
    { key: "transcript", text: t.detail.tabTranscript },
  ];
</script>

{#if meeting}
  <!-- Hanya tampil saat dicetak (Cetak / simpan PDF lewat dialog print WebView2). -->
  <pre class="hidden p-8 font-sans text-sm leading-relaxed whitespace-pre-wrap text-black print:block">{formatMinutes(
      meeting,
      "text",
      transcript,
    )}</pre>
{/if}

<main
  class={[
    "flex w-full flex-col gap-6 pt-7 pb-16 transition-opacity print:hidden",
    embedded ? "max-w-6xl px-8 2xl:px-12" : "mx-auto max-w-3xl px-6 lg:px-10",
    switching && "pointer-events-none opacity-50",
  ]}
  aria-busy={switching}
>
  {#if !embedded}
    <a href="/" class="btn btn-quiet btn-sm -mb-3 -ml-2.5 self-start">
      <Icon name="arrow-left" size={16} />{t.common.back}
    </a>
  {/if}

  {#if notFound}
    <p class="text-ink-soft">{t.errors.NOT_FOUND}</p>
  {:else if !meeting}
    <!-- Kerangka berstruktur sama dengan isi agar layout tidak melompat. -->
    <div class="flex flex-col gap-6 motion-safe:animate-pulse" aria-hidden="true">
      <div class="h-8 w-2/3 rounded-md bg-line-soft"></div>
      <div class="h-4 w-1/3 rounded-md bg-line-soft"></div>
      <div class="h-px bg-line"></div>
      <div class="flex flex-col gap-3">
        <div class="h-4 w-full rounded-md bg-line-soft"></div>
        <div class="h-4 w-11/12 rounded-md bg-line-soft"></div>
        <div class="h-4 w-4/5 rounded-md bg-line-soft"></div>
      </div>
    </div>
  {:else}
    <header class="flex flex-col gap-3">
      <div class="flex items-start gap-2">
        {#if editing}
          <input
            bind:this={titleInput}
            bind:value={titleDraft}
            maxlength="100"
            aria-label={t.detail.titleLabel}
            class="field min-w-0 flex-1 px-2 py-1 text-2xl font-bold tracking-[-0.02em]"
            onkeydown={(e) => {
              if (e.key === "Enter") saveTitle();
              if (e.key === "Escape") editing = false;
            }}
            onblur={saveTitle}
          />
        {:else}
          <h1 class="min-w-0 flex-1">
            <button
              type="button"
              class="group -ml-2 w-full rounded-lg px-2 py-1 text-left text-2xl leading-tight font-bold tracking-[-0.02em] break-words hover:bg-wash"
              title={t.detail.editTitle}
              onclick={startEdit}
            >
              {meeting.title}
              <Icon
                name="pencil"
                size={16}
                class="ml-1 inline-block align-baseline text-ink-faint opacity-0 group-hover:opacity-100 group-focus-visible:opacity-100"
              />
            </button>
          </h1>
        {/if}

        <Menu label={t.detail.menu} items={menuItems}>
          {#snippet trigger()}<Icon name="more" size={20} />{/snippet}
        </Menu>
      </div>

      <div class="flex flex-wrap items-center gap-x-5 gap-y-1 text-sm text-ink-soft">
        <span class="tabular">
          {formatDateTime(meeting.startedAt)}{#if meeting.endedAt}–{formatTime(meeting.endedAt)}{/if}
        </span>
        {#if meeting.durationMs > 0}<span class="tabular">{formatDuration(meeting.durationMs)}</span>{/if}
        {#if !processing}
          <StatusBadge status={meeting.status} progressDone={meeting.progressDone} progressTotal={meeting.progressTotal} />
        {/if}
      </div>
    </header>

    {#if processing}
      <!-- Satu blok status: label + persen + bar (tanpa badge ganda). -->
      <div class="flex flex-col gap-2" role="status">
        <div class="flex items-baseline justify-between gap-3 text-sm">
          <span class="font-medium">
            {t.status[meeting.status]}{#if meeting.status === "waiting_quota"}<span class="font-normal text-ink-soft">
                · {t.errors.RATE_LIMITED}</span
              >{:else if meeting.status === "waiting_network"}<span class="font-normal text-ink-soft"> · {t.errors.NETWORK}</span
              >{/if}
          </span>
          {#if meeting.progressTotal > 0}<span class="tabular text-ink-soft">{progressPct}%</span>{/if}
        </div>
        <div class="relative h-1.5 overflow-hidden rounded-full bg-line-soft">
          {#if meeting.progressTotal > 0}
            <div class="h-full rounded-full bg-ink-strong transition-[width] duration-500" style:width={`${progressPct}%`}></div>
          {:else}
            <div class="indeterminate absolute inset-y-0 w-1/3 rounded-full bg-ink-strong"></div>
          {/if}
        </div>
      </div>
    {/if}

    {#if meeting.status === "failed"}
      <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-bad-wash px-4 py-3 text-bad">
        <Icon name="alert-circle" size={18} class="shrink-0" />
        <span class="flex-1 text-sm font-medium">{meeting.errorMessage ?? t.errors.INTERNAL}</span>
        {#if meeting.errorMessage?.includes("Pengaturan") || meeting.errorCode === "INVALID_API_KEY" || meeting.errorCode === "NO_API_KEY"}
          <a href="/settings" class="btn btn-line btn-sm">{t.home.openSettings}</a>
        {/if}
        <button type="button" class="btn btn-ink btn-sm" onclick={() => act(() => api.retryJob(meetingId), t.toast.requeued)}>
          {t.detail.retry}
        </button>
      </div>
    {/if}

    {#if meeting.status === "interrupted"}
      <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-warn-wash px-4 py-3 text-warn">
        <Icon name="alert-circle" size={18} class="shrink-0" />
        <span class="flex-1 text-sm font-medium">{t.detail.interruptedNote}</span>
        <button
          type="button"
          class="btn btn-ink btn-sm"
          onclick={() => act(() => api.resolveInterrupted(meetingId, "process"), t.toast.requeued)}
        >
          {t.home.interruptedProcess}
        </button>
      </div>
    {/if}

    <!-- Baris tab + aksi tab aktif (Salin / Ubah) di ujung kanan: tanpa baris toolbar terpisah. -->
    <div class="flex flex-wrap items-end gap-x-6 gap-y-2 border-b border-line">
      <div role="tablist" class="flex gap-6">
        {#each tabs as tb (tb.key)}
          <button
            type="button"
            role="tab"
            aria-selected={tab === tb.key}
            disabled={editingSummary && tab !== tb.key}
            title={editingSummary && tab !== tb.key ? t.edit.finishFirst : undefined}
            class={[
              "-mb-px border-b-2 pt-1 pb-2.5 text-base disabled:cursor-not-allowed disabled:opacity-40",
              tab === tb.key ? "border-ink font-semibold text-ink" : "border-transparent text-ink-soft hover:text-ink",
            ]}
            onclick={() => (tab = tb.key)}
          >
            {tb.text}{#if tb.key === "actions" && meeting.actionItems.length > 0}<span class="tabular ml-1.5 text-ink-faint"
                >{meeting.actionItems.length}</span
              >{/if}
          </button>
        {/each}
      </div>
      {#if !editingSummary}
        <div class="ml-auto flex items-center gap-1.5 pb-2">
          {#if tab === "summary" && meeting.summary?.status === "ok"}
            {#if meeting.summary.edited}<span class="mr-1 text-sm text-ink-faint">{t.edit.edited}</span>{/if}
            <CopyButton text={(style) => formatSummaryTab(meeting!, style)} />
          {:else if tab === "actions" && meeting.actionItems.length > 0}
            <CopyButton text={(style) => formatActionItems(meeting!, style)} okText={t.minutes.actionsCopied} />
          {:else if tab === "transcript" && transcript.length > 0}
            <CopyButton text={() => formatTranscript(meeting!, transcript)} okText={t.minutes.transcriptCopied} />
          {/if}
          {#if (tab === "summary" || tab === "actions") && meeting.status === "done" && meeting.summary}
            <button type="button" class="btn btn-quiet btn-sm" onclick={() => (editingSummary = true)}>
              <Icon name="pencil" size={14} />{t.edit.button}
            </button>
          {/if}
        </div>
      {/if}
    </div>

    <section role="tabpanel" class="flex flex-col gap-6">
      {#if (tab === "summary" || tab === "actions") && editingSummary && meeting.summary}
        <SummaryEditor {meeting} onsave={saveSummary} oncancel={() => (editingSummary = false)} />
      {:else if tab === "summary" || tab === "actions"}
        {#if !meeting.summary}
          <p class="max-w-prose text-ink-soft">{emptyNote}</p>
        {:else if tab === "summary" && meeting.summary.status === "empty"}
          <p class="text-ink-soft">{t.summary.noSpeech}</p>
        {:else if tab === "summary"}
          <!-- ≥ 1536 px: tugas tampil di samping ringkasan. -->
          <div class="flex flex-col gap-8 2xl:grid 2xl:grid-cols-[minmax(0,68ch)_minmax(15rem,22rem)] 2xl:items-start 2xl:gap-12">
            <article class="flex max-w-[68ch] flex-col gap-7">
              <div class="flex flex-col gap-2">
                <h2 class="section-title">{t.detail.summary}</h2>
                <p class="text-lg leading-[1.75] whitespace-pre-line">{meeting.summary.summary}</p>
              </div>
              <div class="flex flex-col gap-2">
                <h2 class="section-title">{t.detail.decisions}</h2>
                {#if meeting.summary.decisions.length === 0}
                  <p class="text-ink-soft">{t.detail.noDecisions}</p>
                {:else}
                  <ul class="flex flex-col gap-2">
                    {#each meeting.summary.decisions as d, i (i)}
                      <li class="grid grid-cols-[1rem_1fr] text-lg leading-relaxed">
                        <span class="mt-[0.7em] h-1.5 w-1.5 rounded-full bg-ink" aria-hidden="true"></span>{d}
                      </li>
                    {/each}
                  </ul>
                {/if}
              </div>
              {#if meeting.summary.topics.length > 0}
                <div class="flex flex-col gap-2">
                  <h2 class="section-title">{t.detail.topics}</h2>
                  <ul class="flex flex-wrap gap-1.5">
                    {#each meeting.summary.topics as tp, i (i)}
                      <li class="rounded-md bg-wash px-2.5 py-1 text-sm">{tp}</li>
                    {/each}
                  </ul>
                </div>
              {/if}
            </article>
            {#if meeting.actionItems.length > 0}
              <aside class="sticky top-4 hidden flex-col gap-1 rounded-xl border border-line bg-sheet p-4 2xl:flex">
                <button
                  type="button"
                  class="-mx-1 mb-1 flex items-baseline justify-between rounded-md px-1 text-left hover:bg-wash"
                  onclick={() => (tab = "actions")}
                >
                  <h2 class="font-bold">{t.detail.tabActionItems}</h2>
                  <span class="tabular text-sm text-ink-faint">{meeting.actionItems.length}</span>
                </button>
                <ul class="flex flex-col">
                  {#each meeting.actionItems as a (a.id)}
                    <li class="flex items-start gap-2.5 border-b border-line-soft py-2 last:border-b-0">
                      <input
                        type="checkbox"
                        class="mt-1 h-4 w-4 shrink-0"
                        aria-label={a.task}
                        bind:checked={a.done}
                        onchange={() => toggleItem(a.id, a.done)}
                      />
                      <span class="flex min-w-0 flex-col text-sm">
                        <span class={a.done ? "text-ink-faint line-through" : ""}>{a.task}</span>
                        {#if a.assignee}<span class="text-ink-soft">{a.assignee}</span>{/if}
                      </span>
                    </li>
                  {/each}
                </ul>
              </aside>
            {/if}
          </div>
        {:else if meeting.actionItems.length === 0}
          <p class="text-ink-soft">{t.detail.noActionItems}</p>
        {:else}
          <ul class="flex flex-col">
            {#each meeting.actionItems as a (a.id)}
              <li class="flex items-start gap-3 border-b border-line-soft py-3 last:border-b-0">
                <input
                  type="checkbox"
                  class="mt-1 h-4 w-4 shrink-0"
                  aria-label={a.task}
                  bind:checked={a.done}
                  onchange={() => toggleItem(a.id, a.done)}
                />
                <div class="flex min-w-0 flex-col gap-0.5">
                  <span class={a.done ? "text-ink-faint line-through" : "font-medium"}>{a.task}</span>
                  {#if a.assignee || a.due}
                    <span class="flex flex-wrap gap-x-4 text-sm text-ink-soft">
                      {#if a.assignee}<span>{t.detail.assignee} <span class="text-ink">{a.assignee}</span></span>{/if}
                      {#if a.due}<span>{t.detail.due} <span class="text-ink">{a.due}</span></span>{/if}
                    </span>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      {:else if transcript.length === 0}
        <p class="max-w-prose text-ink-soft">{processing ? t.detail.processing : t.detail.emptyTranscript}</p>
      {:else}
        {#if !meeting.audioDeleted}<p class="-mt-2 text-sm text-ink-faint">{t.detail.clickToPlay}</p>{/if}

        {#if audioSrc || audioLoading}
          <div class="sticky top-2 z-10 rounded-xl border border-line bg-sheet px-3 py-2 shadow-[0_8px_24px_-12px_rgb(30_36_51/0.3)]">
            {#if audioLoading && !audioSrc}
              <span class="text-sm text-ink-soft">{t.detail.preparingAudio}</span>
            {/if}
            {#if audioSrc}
              <AudioPlayer bind:this={player} src={audioSrc} ontime={(ms) => (playMs = ms)} />
            {/if}
          </div>
        {/if}

        <!-- Transkrip polos (tanpa label pembicara, keputusan pemilik). Satu tombol putar per baris di kolom waktu. -->
        <ol
          class={["flex flex-col gap-1", transcript.length > 500 && "virtualized"]}
          onwheel={() => (followUntil = Date.now() + 5000)}
        >
          {#each transcript as s (s.id)}
            <li
              id={`seg-${s.id}`}
              class={[
                "row -mx-2 grid grid-cols-[4.5rem_1fr] items-baseline gap-x-3 rounded-lg px-2 py-1 transition-colors",
                s.id === activeSegId && "bg-wash",
              ]}
              aria-current={s.id === activeSegId ? "true" : undefined}
            >
              {#if meeting.audioDeleted}
                <span class="tabular text-sm text-ink-faint">{formatTimestamp(s.startMs)}</span>
              {:else}
                <button
                  type="button"
                  class="tabular -ml-1.5 rounded-md px-1.5 text-left text-sm text-ink-faint hover:bg-wash hover:text-ink"
                  title={`${t.detail.playFrom} ${formatTimestamp(s.startMs)}`}
                  aria-label={`${t.detail.playFrom} ${formatTimestamp(s.startMs)}`}
                  onclick={() => playAt(s.startMs)}>{formatTimestamp(s.startMs)}</button
                >
              {/if}
              <p class="max-w-[70ch] leading-[1.7]">{s.text.trim()}</p>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}
</main>

<style>
  /* Bar progres tak tentu: garis bergerak (bukan bar penuh yang terlihat sudah selesai). */
  .indeterminate {
    animation: slide 1.4s ease-in-out infinite;
  }
  @keyframes slide {
    from {
      left: -33%;
    }
    to {
      left: 100%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .indeterminate {
      animation: none;
      left: 0;
      width: 100%;
      opacity: 0.4;
    }
  }

  /* Transkrip panjang: browser hanya me-render baris yang terlihat (PRD §14.5). */
  .virtualized .row {
    content-visibility: auto;
    contain-intrinsic-size: auto 2.5rem;
  }
</style>
