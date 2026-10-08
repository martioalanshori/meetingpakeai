<script lang="ts">
  import { onDestroy, onMount, tick, untrack } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import SummaryEditor from "$lib/components/SummaryEditor.svelte";
  import CopyButton from "$lib/components/CopyButton.svelte";
  import { formatActionItems, formatMinutes, formatSummaryTab, formatTranscript } from "$lib/minutes";
  import { formatDateTime, formatDuration, formatTime, formatTimestamp } from "$lib/format";
  import { id as t } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type {
    AppError,
    MeetingDetail,
    MeetingStatus,
    SummaryEdit,
    TemplateOption,
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
  let menuOpen = $state(false);
  let copyOpen = $state(false);
  let editing = $state(false);
  let titleDraft = $state("");
  let titleInput = $state<HTMLInputElement | null>(null);
  let deleteDialog = $state<HTMLDialogElement | null>(null);
  let regenerateDialog = $state<HTMLDialogElement | null>(null);
  let editingSummary = $state(false);
  let audioSrc = $state<string | null>(null);
  let audioEl = $state<HTMLAudioElement | null>(null);
  let audioLoading = $state(false);
  let templates = $state<TemplateOption[]>([]);

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

  async function load() {
    try {
      const m = await api.getMeeting(meetingId);
      const firstLoad = meeting === null;
      meeting = m;
      transcript = await api.getTranscript(meetingId);
      const wanted = initialTab;
      if (firstLoad && (wanted === "summary" || wanted === "actions" || wanted === "transcript")) tab = wanted;
      else if (firstLoad && m.status !== "done" && m.summary === null) tab = transcript.length > 0 ? "transcript" : "summary";
    } catch (e) {
      if ((e as AppError).code === "NOT_FOUND") notFound = true;
      else showToast((e as AppError).message, "error");
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
      audioEl?.pause();
      meeting = null;
      transcript = [];
      notFound = false;
      tab = "summary";
      menuOpen = false;
      copyOpen = false;
      editing = false;
      editingSummary = false;
      audioSrc = null;
      load();
    });
  });

  onMount(async () => {
    templates = await api.listSummaryTemplates().catch(() => []);
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
    menuOpen = false;
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

  function requestRegenerate() {
    menuOpen = false;
    if (meeting?.summary?.edited) regenerateDialog?.showModal();
    else act(() => api.regenerateSummary(meetingId), t.toast.requeued);
  }

  /** Template dipilih → ringkasan dibuat ulang ("" = otomatis). */
  async function changeTemplate(e: Event) {
    const select = e.currentTarget as HTMLSelectElement;
    const value = select.value === "" ? null : select.value;
    if (meeting?.summary?.edited && !confirm(t.edit.templateConfirm)) {
      select.value = meeting.summary.template ?? "";
      return;
    }
    await act(() => api.setSummaryTemplate(meetingId, value), t.toast.requeued);
  }

  const templateLabel = (key: string | null) => templates.find((x) => x.key === key)?.label ?? key ?? "";

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
    deleteDialog?.close();
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
    copyOpen = false;
    if (!meeting) return;
    try {
      const saved = await api.saveExport(exportName(style === "markdown" ? "md" : "txt"), formatMinutes(meeting, style, transcript));
      if (saved) showToast(t.minutes.exported, "success");
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function printMinutes() {
    copyOpen = false;
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
      if (!audioEl) return;
      audioEl.currentTime = ms / 1000;
      await audioEl.play();
    } catch (e) {
      showToast((e as AppError).message ?? t.errors.INTERNAL, "error");
    } finally {
      audioLoading = false;
    }
  }


  const tabs: { key: Tab; text: string }[] = [
    { key: "summary", text: t.detail.tabSummary },
    { key: "actions", text: t.detail.tabActionItems },
    { key: "transcript", text: t.detail.tabTranscript },
  ];
</script>

<svelte:window
  onkeydown={(e) => {
    if (e.key === "Escape") {
      menuOpen = false;
      copyOpen = false;
    }
  }}
/>

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
    "flex w-full flex-col gap-6 pt-7 pb-16 print:hidden",
    embedded ? "max-w-6xl px-8 2xl:px-12" : "max-w-3xl px-6 lg:px-10",
  ]}
>
  {#if notFound}
    <p class="text-ink-soft">{t.errors.NOT_FOUND}</p>
  {:else if !meeting}
    <p class="text-ink-soft">{t.common.loading}</p>
  {:else}
    <header class="flex flex-col gap-3">
      <div class="flex items-start gap-2">
        {#if editing}
          <input
            bind:this={titleInput}
            bind:value={titleDraft}
            maxlength="100"
            aria-label={t.detail.editTitle}
            class="field min-w-0 flex-1 px-2 py-1 text-2xl font-bold tracking-[-0.02em]"
            onkeydown={(e) => {
              if (e.key === "Enter") saveTitle();
              if (e.key === "Escape") editing = false;
            }}
            onblur={saveTitle}
          />
        {:else}
          <button
            type="button"
            class="-ml-2 min-w-0 flex-1 rounded-lg px-2 py-1 text-left text-2xl leading-tight font-bold tracking-[-0.02em] hover:bg-wash"
            title={t.detail.editTitle}
            onclick={startEdit}
          >
            {meeting.title}
          </button>
        {/if}

        {#if meeting.summary}
          <div class="relative">
            <button
              type="button"
              class="btn btn-line"
              aria-haspopup="menu"
              aria-expanded={copyOpen}
              onclick={() => {
                copyOpen = !copyOpen;
                menuOpen = false;
              }}
            >
              {t.minutes.menu}
            </button>
            {#if copyOpen}
              <div role="menu" class="menu">
                <button type="button" role="menuitem" class="menu-item" onclick={() => exportFile("markdown")}>
                  {t.minutes.exportMd}
                </button>
                <button type="button" role="menuitem" class="menu-item" onclick={() => exportFile("text")}>
                  {t.minutes.exportTxt}
                </button>
                <button type="button" role="menuitem" class="menu-item" onclick={printMinutes}>
                  {t.minutes.print}
                </button>
              </div>
            {/if}
          </div>
        {/if}

        <div class="relative">
          <button
            type="button"
            class="btn btn-quiet px-2.5 text-lg leading-5"
            aria-label={t.detail.menu}
            aria-haspopup="menu"
            aria-expanded={menuOpen}
            onclick={() => {
              menuOpen = !menuOpen;
              copyOpen = false;
            }}
          >
            ⋯
          </button>
          {#if menuOpen}
            <div role="menu" class="menu">
              <button type="button" role="menuitem" class="menu-item" disabled={!canRegenerate} onclick={requestRegenerate}>
                {t.detail.regenerate}
              </button>
              <button
                type="button"
                role="menuitem"
                class="menu-item"
                disabled={!canRetranscribe}
                onclick={() => act(() => api.retranscribe(meetingId), t.toast.requeued)}
              >
                {t.detail.retranscribe}
                {#if meeting.audioDeleted}<span class="block text-xs text-ink-faint">{t.detail.audioDeleted}</span>{/if}
              </button>
              <button
                type="button"
                role="menuitem"
                class="menu-item text-bad"
                disabled={meeting.status === "recording"}
                onclick={() => {
                  menuOpen = false;
                  deleteDialog?.showModal();
                }}
              >
                {t.detail.delete}
              </button>
            </div>
          {/if}
        </div>
      </div>

      <div class="flex flex-wrap items-center gap-x-5 gap-y-1 text-sm text-ink-soft">
        <span class="tabular">
          {formatDateTime(meeting.startedAt)}{#if meeting.endedAt}–{formatTime(meeting.endedAt)}{/if}
        </span>
        {#if meeting.durationMs > 0}<span class="tabular">{formatDuration(meeting.durationMs)}</span>{/if}
        <StatusBadge status={meeting.status} progressDone={meeting.progressDone} progressTotal={meeting.progressTotal} />
      </div>
    </header>

    {#if processing}
      <div class="flex flex-col gap-2" role="status">
        <span class="text-sm text-ink">
          {t.status[meeting.status]}{#if meeting.status === "waiting_quota" || meeting.status === "waiting_network"}. {meeting.status ===
            "waiting_quota"
              ? t.errors.RATE_LIMITED
              : t.errors.NETWORK}{/if}
        </span>
        <div class="h-1.5 overflow-hidden rounded-full bg-line-soft">
          <div
            class={["h-full rounded-full bg-ink transition-[width] duration-500", meeting.progressTotal === 0 && "motion-safe:animate-pulse"]}
            style:width={meeting.progressTotal > 0 ? `${progressPct}%` : "100%"}
          ></div>
        </div>
      </div>
    {/if}

    {#if meeting.status === "failed"}
      <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-bad-wash px-4 py-3 text-bad">
        <span class="flex-1 text-sm font-medium">{meeting.errorMessage ?? t.errors.INTERNAL}</span>
        <button type="button" class="btn btn-ink" onclick={() => act(() => api.retryJob(meetingId), t.toast.requeued)}>
          {t.detail.retry}
        </button>
      </div>
    {/if}

    {#if meeting.status === "interrupted"}
      <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl bg-warn-wash px-4 py-3 text-warn">
        <span class="flex-1 text-sm font-medium">{t.detail.interruptedNote}</span>
        <button
          type="button"
          class="btn btn-ink"
          onclick={() => act(() => api.resolveInterrupted(meetingId, "process"), t.toast.requeued)}
        >
          {t.home.interruptedProcess}
        </button>
      </div>
    {/if}

    <div role="tablist" class="flex gap-6 border-b border-line">
      {#each tabs as tb (tb.key)}
        <button
          type="button"
          role="tab"
          aria-selected={tab === tb.key}
          class={[
            "-mb-px border-b-2 pt-1 pb-2.5 text-[0.9375rem]",
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

    <section role="tabpanel" class="flex flex-col gap-6">
      {#if (tab === "summary" || tab === "actions") && editingSummary && meeting.summary}
        <SummaryEditor {meeting} onsave={saveSummary} oncancel={() => (editingSummary = false)} />
      {:else if tab === "summary"}
        {#if !meeting.summary}
          <p class="text-ink-soft">{processing ? t.detail.processing : "—"}</p>
        {:else if meeting.summary.status === "empty"}
          <p class="text-ink-soft">{t.summary.noSpeech}</p>
        {:else}
          <div class="flex flex-wrap items-center gap-2">
            {#if meeting.status === "done" && templates.length > 0}
              <label class="flex items-center gap-2 text-sm text-ink-soft">
                {t.edit.template}
                <select class="field py-1 pr-7 text-sm" onchange={changeTemplate}>
                  <option value="" selected={!meeting.summaryTemplate}>
                    {meeting.summary.template && !meeting.summaryTemplate
                      ? t.edit.templateAutoDetected(templateLabel(meeting.summary.template))
                      : t.edit.templateAuto}
                  </option>
                  {#each templates as tp (tp.key)}
                    <option value={tp.key} selected={meeting.summaryTemplate === tp.key}>{tp.label}</option>
                  {/each}
                </select>
              </label>
            {/if}
            {#if meeting.summary.edited}
              <span class="text-sm text-ink-faint">{t.edit.edited}</span>
            {/if}
            <div class="ml-auto flex items-center gap-1.5">
              <CopyButton text={(style) => formatSummaryTab(meeting!, style)} />
              {#if meeting.status === "done"}
                <button type="button" class="btn btn-quiet" onclick={() => (editingSummary = true)}>{t.edit.button}</button>
              {/if}
            </div>
          </div>

          <!-- ≥ 1536 px: action item tampil di samping ringkasan (ruang lebar terpakai, tugas terlihat sambil membaca). -->
          <div class="flex flex-col gap-8 2xl:grid 2xl:grid-cols-[minmax(0,68ch)_minmax(15rem,22rem)] 2xl:items-start 2xl:gap-12">
          <article class="flex max-w-[68ch] flex-col gap-7">
            <div class="flex flex-col gap-2">
              <h2 class="text-lg font-bold">{t.detail.summary}</h2>
              <p class="text-[1.0625rem] leading-[1.75] whitespace-pre-line">{meeting.summary.summary}</p>
            </div>
            <div class="flex flex-col gap-2">
              <h2 class="text-lg font-bold">{t.detail.decisions}</h2>
              {#if meeting.summary.decisions.length === 0}
                <p class="text-ink-soft">{t.detail.noDecisions}</p>
              {:else}
                <ul class="flex flex-col gap-2">
                  {#each meeting.summary.decisions as d, i (i)}
                    <li class="grid grid-cols-[1rem_1fr] text-[1.0625rem] leading-relaxed">
                      <span class="mt-[0.7em] h-1.5 w-1.5 rounded-full bg-ink" aria-hidden="true"></span>{d}
                    </li>
                  {/each}
                </ul>
              {/if}
            </div>
            {#if meeting.summary.topics.length > 0}
              <div class="flex flex-col gap-2">
                <h2 class="text-lg font-bold">{t.detail.topics}</h2>
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
        {/if}
      {:else if tab === "actions"}
        {#if !meeting.summary}
          <p class="text-ink-soft">{processing ? t.detail.processing : "—"}</p>
        {:else}
          <div class="flex items-center justify-end gap-1.5">
            {#if meeting.actionItems.length > 0}
              <CopyButton text={(style) => formatActionItems(meeting!, style)} okText={t.minutes.actionsCopied} />
            {/if}
            {#if meeting.status === "done"}
              <button type="button" class="btn btn-quiet" onclick={() => (editingSummary = true)}>{t.edit.button}</button>
            {/if}
          </div>
          {#if meeting.actionItems.length === 0}
            <p class="text-ink-soft">{t.detail.noActionItems}</p>
          {:else}
            <ul class="flex flex-col">
              {#each meeting.actionItems as a (a.id)}
                <li class="flex items-start gap-3 border-b border-line-soft py-3 last:border-b-0">
                  <input
                    type="checkbox"
                    class="mt-1 h-4 w-4 shrink-0 accent-ink"
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
        {/if}
      {:else if transcript.length === 0}
        <p class="text-ink-soft">{processing ? t.detail.processing : t.detail.emptyTranscript}</p>
      {:else}
        <div class="flex flex-wrap items-center gap-x-5 gap-y-2">
          {#if !meeting.audioDeleted}<span class="text-sm text-ink-faint">{t.detail.clickToPlay}</span>{/if}
          <div class="ml-auto">
            <CopyButton text={() => formatTranscript(meeting!, transcript)} okText={t.minutes.transcriptCopied} />
          </div>
        </div>

        {#if audioSrc || audioLoading}
          <div class="sticky top-2 z-10 rounded-xl border border-line bg-sheet p-2 shadow-[0_8px_24px_-12px_rgb(30_36_51/0.3)]">
            {#if audioLoading && !audioSrc}<span class="px-2 text-sm text-ink-soft">{t.detail.preparingAudio}</span>{/if}
            {#if audioSrc}
              <audio bind:this={audioEl} src={audioSrc} controls preload="auto" class="h-9 w-full"></audio>
            {/if}
          </div>
        {/if}

        <!-- Transkrip polos: tanpa label pembicara (keputusan pemilik), satu baris per segment. -->
        <ol class={["flex flex-col gap-2", transcript.length > 500 && "virtualized"]}>
          {#each transcript as s (s.id)}
            <li class="row grid grid-cols-[4.25rem_1fr] gap-x-3">
              <span class="tabular pt-1 text-sm text-ink-faint">{formatTimestamp(s.startMs)}</span>
              <p class="max-w-[70ch] border-l-[3px] border-line py-0.5 pl-3.5 leading-[1.7]">
                {#if meeting.audioDeleted}
                  {s.text.trim()}
                {:else}
                  <button
                    type="button"
                    class="inline rounded-sm text-left hover:bg-wash focus-visible:bg-wash"
                    title={`${t.detail.playFrom} ${formatTimestamp(s.startMs)}`}
                    onclick={() => playAt(s.startMs)}>{s.text.trim()}</button
                  >
                {/if}
              </p>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}
</main>

<dialog bind:this={deleteDialog} class="sheet-dialog" aria-labelledby="delete-title">
  <div class="flex flex-col gap-5 p-6">
    <p id="delete-title" class="leading-relaxed">{t.detail.deleteConfirm}</p>
    <div class="flex justify-end gap-2">
      <button type="button" class="btn btn-quiet" onclick={() => deleteDialog?.close()}>{t.common.cancel}</button>
      <button type="button" class="btn bg-bad text-white hover:bg-[#931c13]" onclick={confirmDelete}>
        {t.detail.deleteButton}
      </button>
    </div>
  </div>
</dialog>

<dialog bind:this={regenerateDialog} class="sheet-dialog" aria-labelledby="regen-title">
  <div class="flex flex-col gap-5 p-6">
    <p id="regen-title" class="leading-relaxed">{t.edit.regenerateConfirm}</p>
    <div class="flex justify-end gap-2">
      <button type="button" class="btn btn-quiet" onclick={() => regenerateDialog?.close()}>{t.common.cancel}</button>
      <button
        type="button"
        class="btn btn-ink"
        onclick={() => {
          regenerateDialog?.close();
          act(() => api.regenerateSummary(meetingId), t.toast.requeued);
        }}
      >
        {t.edit.regenerateButton}
      </button>
    </div>
  </div>
</dialog>

<style>
  /* Transkrip panjang: browser hanya me-render baris yang terlihat (PRD §14.5). */
  .virtualized .row {
    content-visibility: auto;
    contain-intrinsic-size: auto 2.5rem;
  }
</style>
