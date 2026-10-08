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
  import FollowUpPanel from "$lib/components/FollowUpPanel.svelte";
  import NotesEditor from "$lib/components/NotesEditor.svelte";
  import AskPanel from "$lib/components/AskPanel.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Wordmark from "$lib/components/Wordmark.svelte";
  import Menu, { type MenuEntry } from "$lib/components/Menu.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import { detailTab, setWindowTitle } from "$lib/viewport.svelte";
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

  type Tab = "summary" | "actions" | "transcript" | "ask";

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
  let soFar = $state<string[]>([]);
  /** Bagian "Catatan saya" di Ringkasan terbuka bila catatan ada. */
  let notesOpen = $state(false);
  let soFarBusy = $state(false);
  /** Transkrip belum final (id negatif dari Rust = segment sementara). */
  const isLive = $derived(transcript.length > 0 && transcript[0].id < 0);

  async function summarizeSoFar() {
    if (!meeting || soFarBusy) return;
    soFarBusy = true;
    try {
      soFar = await api.summarizeSoFar(meeting.id);
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      soFarBusy = false;
    }
  }
  let followOpen = $state(false);
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
        const remembered = detailTab.meetingId === mid ? detailTab.tab : null;
        const wanted = remembered ?? initialTab;
        if (wanted === "summary" || wanted === "actions" || wanted === "transcript" || wanted === "ask") tab = wanted;
        else if (m.status !== "done" && m.summary === null)
          tab = tr.length > 0 || m.status === "recording" ? "transcript" : "summary";
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
      // Transkripsi bertahap selesai satu putaran → transkrip sementara bertambah.
      await events.recordingLive(() => {
        if (meeting?.status === "recording") load();
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

  // Buat ulang dengan permintaan tambahan & bahasa (feedback3 C4/C5): panel di atas isi tab.
  let regenOpen = $state(false);
  let regenInstruction = $state("");
  let regenLang = $state<"" | "id" | "en" | "auto">("");

  function requestRegenerate() {
    regenOpen = true;
    tab = "summary";
  }

  async function submitRegenerate() {
    regenOpen = false;
    const instruction = regenInstruction.trim() || undefined;
    await act(() => api.regenerateSummary(meetingId, instruction, regenLang || null), t.toast.requeued);
    regenInstruction = "";
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


  /** Segment terakhir yang mulai ≤ `ms` (pencarian biner); null jika transkrip kosong. */
  function segmentAt(ms: number): TranscriptSegment | null {
    if (transcript.length === 0) return null;
    let lo = 0;
    let hi = transcript.length - 1;
    while (lo < hi) {
      const mid = (lo + hi + 1) >> 1;
      if (transcript[mid].startMs <= ms) lo = mid;
      else hi = mid - 1;
    }
    return transcript[lo];
  }

  /** Kutipan transkrip ±20 dtk di sekitar momen ditandai (dipotong ±280 karakter). */
  function quoteAround(ms: number): string {
    const text = transcript
      .filter((x) => x.endMs >= ms - 20000 && x.startMs <= ms + 20000)
      .map((x) => x.text.trim())
      .join(" ");
    return text.length > 280 ? text.slice(0, 280).replace(/\s+\S*$/, "") + "…" : text;
  }

  /** Baris transkrip terdekat untuk tiap momen ditandai (penanda bintang). */
  const markedSegIds = $derived(
    new Set((meeting?.bookmarks ?? []).map((b) => segmentAt(b)?.id).filter((x): x is number => x !== undefined)),
  );

  // Perbaiki transkrip (feedback3 C3): ubah satu baris, lalu tawarkan ganti semua + glosarium.
  let editingSeg = $state<number | null>(null);
  let segDraft = $state("");
  let replaceOffer = $state<{ from: string; to: string } | null>(null);
  let replaceDone = $state<{ n: number; glossary: boolean } | null>(null);

  /** Bagian yang berubah (maks 3 kata) antara teks lama dan baru, untuk tawaran "ganti semua". */
  function changedPhrase(before: string, after: string): { from: string; to: string } | null {
    const a = before.trim().split(/\s+/);
    const b = after.trim().split(/\s+/);
    let i = 0;
    while (i < a.length && i < b.length && a[i] === b[i]) i++;
    let j = 0;
    while (j < a.length - i && j < b.length - i && a[a.length - 1 - j] === b[b.length - 1 - j]) j++;
    const strip = (w: string[]) => w.join(" ").replace(/^[^\p{L}\p{N}]+|[^\p{L}\p{N}]+$/gu, "");
    const from = strip(a.slice(i, a.length - j));
    const to = strip(b.slice(i, b.length - j));
    const words = (s: string) => s.split(" ").length;
    if (!from || !to || from === to || words(from) > 3 || words(to) > 3) return null;
    return { from, to };
  }

  async function saveSegment(s: TranscriptSegment) {
    const text = segDraft.trim();
    editingSeg = null;
    if (!text || text === s.text.trim()) return;
    try {
      await api.updateSegment(s.id, text);
      replaceOffer = changedPhrase(s.text, text);
      replaceDone = null;
      s.text = text;
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function applyReplace() {
    if (!replaceOffer || !meeting) return;
    const { from, to } = replaceOffer;
    replaceOffer = null;
    try {
      const r = await api.replaceInMeeting(meeting.id, from, to, true);
      replaceDone = { n: r.replaced, glossary: r.addedToGlossary };
      await load();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  /** Tandai / batalkan tanda momen dari baris transkrip (feedback3 F4). */
  async function toggleMoment(ms: number) {
    if (!meeting) return;
    try {
      meeting.bookmarks = await api.toggleBookmarkAt(meeting.id, ms);
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function removeBookmark(ms: number) {
    if (!meeting) return;
    try {
      await api.deleteBookmark(meeting.id, ms);
      meeting.bookmarks = meeting.bookmarks.filter((b) => b !== ms);
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  /** Chip waktu keputusan/tugas: buka Transkrip di baris terdekat (sedikit sebelum, agar konteks terbaca) dan putar. */
  async function jumpTo(ms: number) {
    const target = Math.max(0, ms - 3000);
    tab = "transcript";
    await tick();
    const seg = segmentAt(target);
    if (seg) {
      followUntil = Date.now() + 5000;
      const el = document.getElementById(`seg-${seg.id}`);
      el?.scrollIntoView({ block: "center", behavior: "smooth" });
      el?.classList.add("flash");
      setTimeout(() => el?.classList.remove("flash"), 1600);
    }
    if (!meeting?.audioDeleted) await playAt(seg?.startMs ?? target);
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

  // Panel tindak lanjut ditutup saat pindah meeting.
  $effect(() => {
    void meetingId;
    untrack(() => (followOpen = false));
  });

  // Tab aktif diingat per meeting (tata letak berganti saat jendela diubah ukurannya).
  $effect(() => {
    if (meeting && meeting.id === meetingId) {
      detailTab.meetingId = meetingId;
      detailTab.tab = tab;
    }
  });

  $effect(() => {
    if (meeting) setWindowTitle(meeting.title);
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

  const SEG_BLOCK = 200;
  const segBlocks = $derived.by(() => {
    const out: TranscriptSegment[][] = [];
    for (let i = 0; i < transcript.length; i += SEG_BLOCK) out.push(transcript.slice(i, i + SEG_BLOCK));
    return out;
  });

  /** Panah kiri/kanan/Home/End di baris tab (pola ARIA tabs). */
  function tabKey(e: KeyboardEvent) {
    const keys = ["ArrowLeft", "ArrowRight", "Home", "End"];
    if (!keys.includes(e.key) || editingSummary) return;
    e.preventDefault();
    const i = tabs.findIndex((x) => x.key === tab);
    const n = tabs.length;
    const next = e.key === "Home" ? 0 : e.key === "End" ? n - 1 : (i + (e.key === "ArrowRight" ? 1 : n - 1)) % n;
    tab = tabs[next].key;
    document.getElementById(`tab-${tab}`)?.focus();
  }

  const tabs: { key: Tab; text: string }[] = [
    { key: "summary", text: t.detail.tabSummary },
    { key: "actions", text: t.detail.tabActionItems },
    { key: "transcript", text: t.detail.tabTranscript },
    { key: "ask", text: t.detail.tabAsk },
  ];
</script>

{#snippet sourceChip(ms: number | null | undefined)}
  {#if ms !== null && ms !== undefined && transcript.length > 0}
    <button
      type="button"
      class="tabular ml-1.5 inline-flex translate-y-[-0.1em] items-center gap-1 rounded-md bg-wash px-1.5 py-px align-middle text-xs text-ink-soft hover:bg-line-soft hover:text-ink print:hidden"
      title={t.detail.sourceAt(formatTimestamp(ms))}
      aria-label={t.detail.sourceAt(formatTimestamp(ms))}
      onclick={() => jumpTo(ms)}
    >
      <Icon name="play" size={10} />{formatTimestamp(ms)}
    </button>
  {/if}
{/snippet}

{#if meeting}
  <!-- Hanya tampil saat dicetak (Cetak / simpan PDF): dokumen notulen berformat, bukan teks polos. -->
  <article class="print-doc hidden print:block">
    <header class="print-head">
      <span class="print-brand"><Wordmark size={18} /></span>
      <h1>{meeting.title}</h1>
      <p class="print-meta">
        {formatDateTime(meeting.startedAt)}{#if meeting.endedAt}–{formatTime(meeting.endedAt)}{/if}{#if meeting.durationMs > 0}&ensp;·&ensp;{formatDuration(meeting.durationMs)}{/if}
      </p>
    </header>
    {#if meeting.summary?.status === "ok"}
      <h2>{t.detail.summary}</h2>
      <p class="print-summary">{meeting.summary.summary}</p>
      <h2>{t.detail.decisions}</h2>
      {#if meeting.summary.decisions.length === 0}<p>{t.detail.noDecisions}</p>{:else}
        <ul>{#each meeting.summary.decisions as d, i (i)}<li>{d}</li>{/each}</ul>
      {/if}
      {#if meeting.summary.openQuestions.length > 0}
        <h2>{t.detail.openQuestions}</h2>
        <ul>{#each meeting.summary.openQuestions as q, i (i)}<li>{q}</li>{/each}</ul>
      {/if}
      <h2>{t.detail.tabActionItems}</h2>
      {#if meeting.actionItems.length === 0}<p>{t.detail.noActionItems}</p>{:else}
        <table>
          <thead><tr><th>{t.edit.task}</th><th>{t.edit.assignee}</th><th>{t.edit.due}</th></tr></thead>
          <tbody>
            {#each meeting.actionItems as a (a.id)}
              <tr><td>{a.done ? "✓ " : ""}{a.task}</td><td>{a.assignee ?? "—"}</td><td>{a.due ?? "—"}</td></tr>
            {/each}
          </tbody>
        </table>
      {/if}
      {#if meeting.summary.topics.length > 0}
        <h2>{t.detail.topics}</h2>
        <p>{meeting.summary.topics.join(", ")}</p>
      {/if}
    {:else if meeting.summary?.status === "empty"}
      <p>{t.summary.noSpeech}</p>
    {/if}
    {#if transcript.length > 0}
      <h2 class="print-break">{t.detail.tabTranscript}</h2>
      <ol class="print-transcript">
        {#each transcript as s (s.id)}<li><span>{formatTimestamp(s.startMs)}</span>{s.text.trim()}</li>{/each}
      </ol>
    {/if}
  </article>
{/if}

<main
  class={[
    "@container flex w-full flex-col gap-6 pt-7 pb-16 transition-opacity print:hidden",
    embedded ? "max-w-6xl px-8 2xl:px-12" : "mx-auto max-w-4xl px-6 xl:px-10",
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
          <a href="/settings?tab=ai" class="btn btn-line btn-sm">{t.home.openSettings}</a>
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
      <div role="tablist" class="flex gap-6" aria-label={t.detail.tabsLabel} tabindex="-1" onkeydown={tabKey}>
        {#each tabs as tb (tb.key)}
          <button
            type="button"
            role="tab"
            id={`tab-${tb.key}`}
            aria-controls="detail-panel"
            tabindex={tab === tb.key ? 0 : -1}
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
            {#if meeting.status === "done"}
              <button
                type="button"
                class="btn btn-line btn-sm"
                aria-expanded={followOpen}
                onclick={() => (followOpen = !followOpen)}
              >
                <Icon name="send" size={14} />{t.followUp.button}
              </button>
            {/if}
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

    <div id="detail-panel" role="tabpanel" aria-labelledby={`tab-${tab}`} class="flex flex-col gap-6">
      {#if regenOpen}
        <form
          class="flex flex-col gap-3 rounded-xl border border-line bg-paper/60 p-4"
          onsubmit={(e) => {
            e.preventDefault();
            submitRegenerate();
          }}
        >
          <div class="flex items-center gap-2">
            <h2 class="flex-1 font-bold">{t.detail.regenTitle}</h2>
            <button
              type="button"
              class="btn btn-quiet btn-icon btn-sm"
              aria-label={t.common.close}
              onclick={() => (regenOpen = false)}><Icon name="x" size={16} /></button
            >
          </div>
          <label class="flex flex-col gap-1.5">
            <span class="label">{t.detail.regenInstruction}</span>
            <textarea class="field resize-y" rows="2" maxlength="500" placeholder={t.detail.regenPlaceholder} bind:value={regenInstruction}
            ></textarea>
          </label>
          <label class="flex flex-wrap items-center gap-3">
            <span class="label">{t.detail.regenLanguage}</span>
            <select class="field w-auto py-1.5" bind:value={regenLang}>
              <option value="">{t.detail.regenLangDefault}</option>
              <option value="id">{t.settings.notesLangId}</option>
              <option value="en">{t.settings.notesLangEn}</option>
              <option value="auto">{t.settings.notesLangAuto}</option>
            </select>
          </label>
          {#if meeting.summary?.edited}<p class="hint text-warn">{t.detail.regenEdited}</p>{/if}
          <div><button type="submit" class="btn btn-ink btn-sm"><Icon name="refresh" size={14} />{t.detail.regenStart}</button></div>
        </form>
      {/if}
      {#if (tab === "summary" || tab === "actions") && editingSummary && meeting.summary}
        <SummaryEditor {meeting} onsave={saveSummary} oncancel={() => (editingSummary = false)} />
      {:else if tab === "summary" || tab === "actions"}
        {#if !meeting.summary}
          <p class="max-w-prose text-ink-soft">{emptyNote}</p>
        {:else if tab === "summary" && meeting.summary.status === "empty"}
          <p class="text-ink-soft">{t.summary.noSpeech}</p>
        {:else if tab === "summary"}
          {#if followOpen && meeting.status === "done"}
            {#key meeting.id}
              <FollowUpPanel
                meetingId={meeting.id}
                initial={meeting.summary.followUp}
                onclose={() => (followOpen = false)}
              />
            {/key}
          {/if}
          <!-- Panel detail ≥ 60rem (bukan lebar layar): tugas tampil di samping ringkasan. -->
          <div
            class="flex flex-col gap-8 @min-[60rem]:grid @min-[60rem]:grid-cols-[minmax(0,68ch)_minmax(15rem,22rem)] @min-[60rem]:items-start @min-[60rem]:gap-10"
          >
            <article class="flex max-w-[68ch] flex-col gap-7">
              {#if meeting.summary.keyPoints.length > 0}
                <!-- Intisari 3 poin untuk pembaca sibuk (feedback3 C2). -->
                <div class="flex flex-col gap-2 rounded-xl bg-paper/70 px-4 py-3.5">
                  <h2 class="section-title">{t.detail.keyPoints}</h2>
                  <ul class="flex flex-col gap-1.5">
                    {#each meeting.summary.keyPoints as p, i (i)}
                      <li class="grid grid-cols-[1.25rem_1fr] text-lg leading-relaxed font-medium">
                        <span class="tabular text-ink-faint">{i + 1}.</span>{p}
                      </li>
                    {/each}
                  </ul>
                </div>
              {/if}
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
                        <span class="mt-[0.7em] h-1.5 w-1.5 rounded-full bg-ink" aria-hidden="true"></span>
                        <span>{d}{@render sourceChip(meeting.summary.decisionSources[i])}</span>
                      </li>
                    {/each}
                  </ul>
                {/if}
              </div>
              {#if meeting.summary.openQuestions.length > 0}
                <div class="flex flex-col gap-2">
                  <h2 class="section-title">{t.detail.openQuestions}</h2>
                  <ul class="flex flex-col gap-2">
                    {#each meeting.summary.openQuestions as q, i (i)}
                      <li class="grid grid-cols-[1rem_1fr] text-lg leading-relaxed">
                        <span class="mt-[0.55em] text-warn" aria-hidden="true">?</span>
                        <span>{q}{@render sourceChip(meeting.summary.openQuestionSources[i])}</span>
                      </li>
                    {/each}
                  </ul>
                </div>
              {/if}
              <details class="group flex flex-col gap-2" open={notesOpen}>
                <summary class="section-title flex cursor-pointer list-none items-center gap-1.5">
                  <Icon name="pencil" size={15} />{t.detail.myNotes}
                  <Icon name="chevron-down" size={15} class="text-ink-faint transition-transform group-open:rotate-180" />
                </summary>
                <div class="pt-2"><NotesEditor meetingId={meeting.id} rows={4} /></div>
              </details>
              {#if meeting.bookmarks.length > 0}
                <div class="flex flex-col gap-2">
                  <h2 class="section-title flex items-center gap-1.5">
                    <Icon name="star" size={16} class="text-warn" />{t.detail.bookmarks}
                  </h2>
                  <ul class="flex flex-col gap-2.5">
                    {#each meeting.bookmarks as b (b)}
                      {@const quote = quoteAround(b)}
                      <li class="group flex items-start gap-2 rounded-lg border border-line-soft bg-paper/50 px-3 py-2">
                        <span class="min-w-0 flex-1 leading-relaxed">
                          {#if quote}<span class="text-ink">“{quote}”</span>{:else}<span class="text-ink-soft"
                              >{t.detail.bookmarkNoText}</span
                            >{/if}{@render sourceChip(b)}
                        </span>
                        <button
                          type="button"
                          class="btn btn-quiet btn-icon btn-sm opacity-60 group-hover:opacity-100 print:hidden"
                          aria-label={t.detail.bookmarkRemove}
                          title={t.detail.bookmarkRemove}
                          onclick={() => removeBookmark(b)}
                        >
                          <Icon name="x" size={14} />
                        </button>
                      </li>
                    {/each}
                  </ul>
                </div>
              {/if}
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
              <aside class="sticky top-4 hidden flex-col gap-1 rounded-xl border border-line bg-paper/60 p-4 @min-[60rem]:flex">
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
                        {#if a.assignee || a.sourceMs !== null}
                          <span class="text-ink-soft">{a.assignee ?? ""}{@render sourceChip(a.sourceMs)}</span>
                        {/if}
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
                  {#if a.assignee || a.due || a.sourceMs !== null}
                    <span class="flex flex-wrap items-baseline gap-x-4 text-sm text-ink-soft">
                      {#if a.assignee}<span>{t.detail.assignee} <span class="text-ink">{a.assignee}</span></span>{/if}
                      {#if a.due}<span>{t.detail.due} <span class="text-ink">{a.due}</span></span>{/if}
                      {#if a.sourceMs !== null}<span class="-ml-1.5">{@render sourceChip(a.sourceMs)}</span>{/if}
                    </span>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
        {/if}
      {:else if tab === "ask"}
        {#if transcript.length === 0}
          <p class="max-w-prose text-ink-soft">{meeting.status === "recording" ? t.detail.liveEmpty : t.detail.emptyTranscript}</p>
        {:else}
          {#key meeting.id}<AskPanel meetingId={meeting.id} onjump={jumpTo} />{/key}
        {/if}
      {:else if transcript.length === 0}
        <p class="max-w-prose text-ink-soft">
          {meeting.status === "recording" ? t.detail.liveEmpty : processing ? t.detail.processing : t.detail.emptyTranscript}
        </p>
      {:else}
        {#if meeting.status === "recording"}
          <div class="flex flex-col gap-2 rounded-xl border border-line p-4">
            <span class="font-semibold">{t.detail.myNotes}</span>
            <NotesEditor meetingId={meeting.id} rows={3} />
          </div>
        {/if}
        {#if isLive}
          <!-- Transkrip sementara (feedback3 B1) + Ringkas sejauh ini. -->
          <div class="-mt-1 flex flex-col gap-3 rounded-xl border border-line bg-paper/60 p-4">
            <div class="flex flex-wrap items-start gap-3">
              <div class="flex min-w-0 flex-1 flex-col gap-0.5">
                <span class="font-semibold">{t.detail.liveTranscript}</span>
                <span class="hint">{t.detail.liveTranscriptHint}</span>
              </div>
              <button type="button" class="btn btn-ink btn-sm" disabled={soFarBusy} onclick={summarizeSoFar}>
                <Icon name="list" size={14} />{soFarBusy ? t.detail.soFarBusy : t.detail.soFar}
              </button>
            </div>
            {#if soFar.length > 0}
              <div class="flex flex-col gap-1.5 border-t border-line pt-3" role="status">
                <span class="text-sm font-semibold">{t.detail.soFarTitle}</span>
                <ul class="flex flex-col gap-1.5">
                  {#each soFar as p, i (i)}
                    <li class="grid grid-cols-[1rem_1fr] leading-relaxed">
                      <span class="mt-[0.65em] h-1.5 w-1.5 rounded-full bg-ink" aria-hidden="true"></span>{p}
                    </li>
                  {/each}
                </ul>
              </div>
            {/if}
          </div>
        {:else if !meeting.audioDeleted}<p class="-mt-2 text-sm text-ink-faint">{t.detail.clickToPlay}</p>{/if}

        {#if replaceOffer}
          <div class="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-paper/60 px-4 py-3" role="status">
            <span class="flex-1 text-sm">{t.detail.replaceOffer(replaceOffer.from, replaceOffer.to)}</span>
            <button type="button" class="btn btn-ink btn-sm" onclick={applyReplace}>{t.detail.replaceAll}</button>
            <button type="button" class="btn btn-quiet btn-sm" onclick={() => (replaceOffer = null)}>{t.detail.dismiss}</button>
          </div>
        {:else if replaceDone}
          <div class="flex flex-wrap items-center gap-3 rounded-xl border border-line bg-paper/60 px-4 py-3" role="status">
            <span class="flex-1 text-sm">{t.detail.replaceDone(replaceDone.n, replaceDone.glossary)}</span>
            {#if canRegenerate}
              <button type="button" class="btn btn-ink btn-sm" onclick={() => ((replaceDone = null), requestRegenerate())}>{t.detail.regenStart}</button>
            {/if}
            <button type="button" class="btn btn-quiet btn-sm" onclick={() => (replaceDone = null)}>{t.detail.dismiss}</button>
          </div>
        {/if}

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
        <!-- Transkrip panjang dibagi blok 200 baris; blok di luar layar tidak di-layout/di-render (content-visibility). -->
        <div
          class={["flex flex-col gap-1", transcript.length > 500 && "virtualized"]}
          onwheel={() => (followUntil = Date.now() + 5000)}
        >
          {#each segBlocks as block, bi (bi)}
          <ol class="seg-block flex flex-col gap-1" style:--rows={block.length}>
          {#each block as s (s.id)}
            <li
              id={`seg-${s.id}`}
              class={[
                "row -mx-2 grid grid-cols-[4.5rem_1fr] items-baseline gap-x-3 rounded-lg px-2 py-1 transition-colors",
                s.id === activeSegId && "bg-wash",
              ]}
              aria-current={s.id === activeSegId ? "true" : undefined}
            >
              {#if meeting.audioDeleted || isLive}
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
              <p class="group/row relative max-w-[70ch] leading-[1.7]">
                {#if !isLive && editingSeg !== s.id}
                  <button
                    type="button"
                    class="absolute top-0.5 -right-16 rounded p-1 text-ink-faint opacity-0 group-hover/row:opacity-100 hover:bg-wash hover:text-ink focus:opacity-100 print:hidden"
                    title={t.detail.editLine}
                    aria-label={t.detail.editLine}
                    onclick={() => ((editingSeg = s.id), (segDraft = s.text.trim()))}
                  >
                    <Icon name="pencil" size={14} />
                  </button>
                  <button
                    type="button"
                    class={[
                      "absolute top-0.5 -right-8 rounded p-1 text-ink-faint hover:bg-wash hover:text-warn print:hidden",
                      markedSegIds.has(s.id) ? "opacity-100" : "opacity-0 group-hover/row:opacity-100 focus:opacity-100",
                    ]}
                    title={markedSegIds.has(s.id) ? t.detail.unmarkMoment : t.detail.markMoment}
                    aria-label={markedSegIds.has(s.id) ? t.detail.unmarkMoment : t.detail.markMoment}
                    onclick={() => toggleMoment(s.startMs)}
                  >
                    <Icon name="star" size={14} />
                  </button>
                {/if}
                {#if editingSeg === s.id}
                  <span class="flex flex-col gap-2">
                    <!-- svelte-ignore a11y_autofocus -->
                    <textarea
                      class="field resize-y leading-relaxed"
                      rows="2"
                      bind:value={segDraft}
                      autofocus
                      onkeydown={(e) => {
                        if (e.key === "Enter" && !e.shiftKey) {
                          e.preventDefault();
                          saveSegment(s);
                        } else if (e.key === "Escape") editingSeg = null;
                      }}
                    ></textarea>
                    <span class="flex gap-2">
                      <button type="button" class="btn btn-ink btn-sm" onclick={() => saveSegment(s)}>{t.detail.saveLine}</button>
                      <button type="button" class="btn btn-quiet btn-sm" onclick={() => (editingSeg = null)}>{t.detail.cancelLine}</button>
                    </span>
                  </span>
                {:else}
                  {#if markedSegIds.has(s.id)}<Icon name="star" size={14} class="mr-1 inline -translate-y-px text-warn" /><span
                      class="sr-only">{t.detail.bookmarkMarker}:</span
                    >{/if}{s.text.trim()}
                {/if}
              </p>
            </li>
          {/each}
          </ol>
          {/each}
        </div>
      {/if}
    </div>
  {/if}
</main>

<style>
  /* Dokumen cetak / PDF. */
  @page {
    margin: 18mm 16mm;
  }
  .print-doc {
    color: #000;
    font-size: 10.5pt;
    line-height: 1.55;
  }
  .print-head {
    border-bottom: 1.5pt solid #000;
    padding-bottom: 8pt;
    margin-bottom: 14pt;
  }
  .print-brand {
    display: block;
    margin-bottom: 8pt;
  }
  .print-doc h1 {
    font-size: 18pt;
    font-weight: 700;
    line-height: 1.25;
  }
  .print-meta {
    margin-top: 4pt;
    color: #444;
  }
  .print-doc h2 {
    margin: 14pt 0 5pt;
    font-size: 12pt;
    font-weight: 700;
    break-after: avoid;
  }
  .print-summary {
    white-space: pre-line;
  }
  .print-doc ul {
    padding-left: 14pt;
    list-style: disc;
  }
  .print-doc table {
    width: 100%;
    border-collapse: collapse;
  }
  .print-doc th,
  .print-doc td {
    border-bottom: 0.5pt solid #bbb;
    padding: 4pt 6pt 4pt 0;
    text-align: left;
    vertical-align: top;
  }
  .print-doc th {
    font-weight: 700;
  }
  .print-doc tr {
    break-inside: avoid;
  }
  .print-break {
    break-before: page;
  }
  .print-transcript li {
    display: grid;
    grid-template-columns: 52pt 1fr;
    break-inside: avoid;
  }
  .print-transcript span {
    color: #555;
    font-variant-numeric: tabular-nums;
  }

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

  /* Transkrip panjang: browser hanya me-render blok yang terlihat (PRD §14.5). Perkiraan tinggi
     dipakai sebelum blok pertama kali tampil, lalu diganti tinggi sebenarnya (`auto`). */
  :global(.row.flash) {
    background: var(--color-wash);
    box-shadow: inset 3px 0 0 var(--color-ink);
  }
  .virtualized .seg-block {
    content-visibility: auto;
    contain-intrinsic-size: auto calc(var(--rows) * 2.6rem);
  }
</style>
