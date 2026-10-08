<script lang="ts">
  import { onDestroy, onMount, tick } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import StatusBadge from "$lib/components/StatusBadge.svelte";
  import SummaryEditor from "$lib/components/SummaryEditor.svelte";
  import { formatActionItems, formatMinutes, type MinutesStyle } from "$lib/minutes";
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

  const meetingId = $derived(page.params.id ?? "");

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
  let speakerDraft = $state("");
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
      if (firstLoad) speakerDraft = m.labels.system;
      transcript = await api.getTranscript(meetingId);
      const wanted = page.url.searchParams.get("tab");
      if (firstLoad && (wanted === "summary" || wanted === "actions" || wanted === "transcript")) tab = wanted;
      else if (firstLoad && m.status !== "done" && m.summary === null) tab = transcript.length > 0 ? "transcript" : "summary";
    } catch (e) {
      if ((e as AppError).code === "NOT_FOUND") notFound = true;
      else showToast((e as AppError).message, "error");
    }
  }

  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
    templates = await api.listSummaryTemplates().catch(() => []);
    await load();
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

  async function saveSpeaker(e: SubmitEvent) {
    e.preventDefault();
    await act(() => api.setSpeakerName(meetingId, speakerDraft), t.edit.speakerSaved);
    if (meeting) speakerDraft = meeting.labels.system;
  }

  async function confirmDelete() {
    deleteDialog?.close();
    try {
      await api.deleteMeeting(meetingId);
      showToast(t.toast.deleted);
      await goto("/");
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

  async function copyText(text: string, okText: string) {
    copyOpen = false;
    try {
      await navigator.clipboard.writeText(text);
      showToast(okText, "success");
    } catch {
      showToast(t.minutes.copyFailed, "error");
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

  const copyMinutes = (style: MinutesStyle) => meeting && copyText(formatMinutes(meeting, style), t.minutes.copied);
  const copyActions = () => meeting && copyText(formatActionItems(meeting, "text"), t.minutes.actionsCopied);

  function label(ch: "mic" | "system") {
    return ch === "mic" ? meeting!.labels.mic : meeting!.labels.system;
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

<main class="mx-auto flex min-h-full max-w-4xl flex-col gap-5 p-6 print:hidden">
  <a href="/" class="text-sm text-indigo-700 hover:underline">{t.common.back}</a>

  {#if notFound}
    <p class="text-gray-700">{t.errors.NOT_FOUND}</p>
  {:else if !meeting}
    <p class="text-gray-500">{t.common.loading}</p>
  {:else}
    <header class="flex flex-col gap-2">
      <div class="flex items-start gap-3">
        {#if editing}
          <input
            bind:this={titleInput}
            bind:value={titleDraft}
            maxlength="100"
            class="min-w-0 flex-1 rounded-lg border border-indigo-400 px-2 py-1 text-xl font-semibold"
            onkeydown={(e) => {
              if (e.key === "Enter") saveTitle();
              if (e.key === "Escape") editing = false;
            }}
            onblur={saveTitle}
          />
        {:else}
          <button
            type="button"
            class="min-w-0 flex-1 rounded-lg px-2 py-1 text-left text-xl font-semibold hover:bg-gray-100"
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
              class="flex items-center gap-1.5 rounded-lg border border-gray-300 bg-white px-3 py-1.5 text-sm font-medium hover:bg-gray-100"
              aria-haspopup="menu"
              aria-expanded={copyOpen}
              onclick={() => (copyOpen = !copyOpen)}
            >
              <Icon name="copy" size={16} />
              {t.minutes.menu}
            </button>
            {#if copyOpen}
              <div
                role="menu"
                class="absolute right-0 z-20 mt-1 flex w-56 flex-col overflow-hidden rounded-lg border border-gray-200 bg-white py-1 shadow-lg"
              >
                <button type="button" role="menuitem" class="px-4 py-2 text-left hover:bg-gray-100" onclick={() => copyMinutes("text")}>
                  {t.minutes.copyPlain}
                </button>
                <button
                  type="button"
                  role="menuitem"
                  class="px-4 py-2 text-left hover:bg-gray-100"
                  onclick={() => copyMinutes("whatsapp")}
                >
                  {t.minutes.copyWhatsapp}
                </button>
                <button type="button" role="menuitem" class="px-4 py-2 text-left hover:bg-gray-100" onclick={copyActions}>
                  {t.minutes.copyActions}
                </button>
                <hr class="my-1 border-gray-200" />
                <button
                  type="button"
                  role="menuitem"
                  class="px-4 py-2 text-left hover:bg-gray-100"
                  onclick={() => exportFile("markdown")}
                >
                  {t.minutes.exportMd}
                </button>
                <button type="button" role="menuitem" class="px-4 py-2 text-left hover:bg-gray-100" onclick={() => exportFile("text")}>
                  {t.minutes.exportTxt}
                </button>
                <button type="button" role="menuitem" class="px-4 py-2 text-left hover:bg-gray-100" onclick={printMinutes}>
                  {t.minutes.print}
                </button>
              </div>
            {/if}
          </div>
        {/if}

        <div class="relative">
          <button
            type="button"
            class="rounded-lg px-3 py-1.5 text-xl leading-none hover:bg-gray-200"
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
            <div
              role="menu"
              class="absolute right-0 z-20 mt-1 flex w-56 flex-col overflow-hidden rounded-lg border border-gray-200 bg-white py-1 shadow-lg"
            >
              <button
                type="button"
                role="menuitem"
                class="px-4 py-2 text-left hover:bg-gray-100 disabled:cursor-not-allowed disabled:text-gray-400"
                disabled={!canRegenerate}
                onclick={requestRegenerate}
              >
                {t.detail.regenerate}
              </button>
              <button
                type="button"
                role="menuitem"
                class="px-4 py-2 text-left hover:bg-gray-100 disabled:cursor-not-allowed disabled:text-gray-400"
                disabled={!canRetranscribe}
                title={meeting.audioDeleted ? t.detail.audioDeleted : undefined}
                onclick={() => act(() => api.retranscribe(meetingId), t.toast.requeued)}
              >
                {t.detail.retranscribe}
                {#if meeting.audioDeleted}<span class="block text-xs text-gray-400">{t.detail.audioDeleted}</span>{/if}
              </button>
              <button
                type="button"
                role="menuitem"
                class="px-4 py-2 text-left text-red-700 hover:bg-red-50 disabled:cursor-not-allowed disabled:text-gray-400"
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
      <div class="flex flex-wrap items-center gap-3 px-2 text-sm text-gray-600">
        <span>
          {formatDateTime(meeting.startedAt)}{#if meeting.endedAt}–{formatTime(meeting.endedAt)}{/if}
        </span>
        {#if meeting.durationMs > 0}<span>· {formatDuration(meeting.durationMs)}</span>{/if}
        <StatusBadge status={meeting.status} progressDone={meeting.progressDone} progressTotal={meeting.progressTotal} />
      </div>
    </header>

    {#if processing}
      <div class="flex flex-col gap-2 rounded-xl border border-indigo-100 bg-indigo-50 p-4" role="status">
        <span class="text-sm text-indigo-900">
          {t.status[meeting.status]}{#if meeting.status === "waiting_quota" || meeting.status === "waiting_network"} — {meeting.status ===
            "waiting_quota"
              ? t.errors.RATE_LIMITED
              : t.errors.NETWORK}{/if}
        </span>
        <div class="h-2 overflow-hidden rounded bg-indigo-100">
          <div
            class={["h-full bg-indigo-600 transition-[width]", meeting.progressTotal === 0 && "animate-pulse"]}
            style:width={meeting.progressTotal > 0 ? `${progressPct}%` : "100%"}
          ></div>
        </div>
      </div>
    {/if}

    {#if meeting.status === "failed"}
      <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl border border-red-200 bg-red-50 p-4 text-red-900">
        <span class="flex-1">{meeting.errorMessage ?? t.errors.INTERNAL}</span>
        <button
          type="button"
          class="rounded-lg bg-red-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-red-800"
          onclick={() => act(() => api.retryJob(meetingId), t.toast.requeued)}
        >
          {t.detail.retry}
        </button>
      </div>
    {/if}

    {#if meeting.status === "interrupted"}
      <div role="alert" class="flex flex-wrap items-center gap-3 rounded-xl border border-amber-200 bg-amber-50 p-4 text-amber-950">
        <span class="flex-1">{t.detail.interruptedNote}</span>
        <button
          type="button"
          class="rounded-lg bg-amber-700 px-3 py-1.5 text-sm font-medium text-white hover:bg-amber-800"
          onclick={() => act(() => api.resolveInterrupted(meetingId, "process"), t.toast.requeued)}
        >
          {t.home.interruptedProcess}
        </button>
      </div>
    {/if}

    <div role="tablist" class="flex gap-1 border-b border-gray-200">
      {#each tabs as tb (tb.key)}
        <button
          type="button"
          role="tab"
          aria-selected={tab === tb.key}
          class={[
            "-mb-px border-b-2 px-4 py-2 font-medium",
            tab === tb.key ? "border-indigo-600 text-indigo-700" : "border-transparent text-gray-600 hover:text-gray-900",
          ]}
          onclick={() => (tab = tb.key)}
        >
          {tb.text}
        </button>
      {/each}
    </div>

    <section role="tabpanel" class="flex flex-col gap-4">
      {#if (tab === "summary" || tab === "actions") && editingSummary && meeting.summary}
        <SummaryEditor {meeting} onsave={saveSummary} oncancel={() => (editingSummary = false)} />
      {:else if tab === "summary"}
        {#if !meeting.summary}
          <p class="text-gray-500">{processing ? t.detail.processing : "—"}</p>
        {:else if meeting.summary.status === "empty"}
          <p class="text-gray-600">{t.summary.noSpeech}</p>
        {:else}
          <div class="flex flex-col gap-2">
            <div class="flex items-center gap-3">
              <h2 class="font-semibold">{t.detail.summary}</h2>
              {#if meeting.summary.edited}
                <span class="rounded-full bg-gray-200 px-2 py-0.5 text-xs text-gray-700">{t.edit.edited}</span>
              {/if}
              {#if meeting.status === "done" && templates.length > 0}
                <label class="flex items-center gap-1.5 text-sm text-gray-600">
                  {t.edit.template}
                  <select class="rounded-lg border border-gray-300 bg-white px-2 py-1 text-sm" onchange={changeTemplate}>
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
              {#if meeting.status === "done"}
                <button
                  type="button"
                  class="ml-auto rounded-lg px-3 py-1 text-sm text-indigo-700 hover:bg-indigo-50"
                  onclick={() => (editingSummary = true)}
                >
                  {t.edit.button}
                </button>
              {/if}
            </div>
            <p class="leading-relaxed whitespace-pre-line text-gray-800">{meeting.summary.summary}</p>
          </div>
          <div class="flex flex-col gap-2">
            <h2 class="font-semibold">{t.detail.decisions}</h2>
            {#if meeting.summary.decisions.length === 0}
              <p class="text-gray-500">{t.detail.noDecisions}</p>
            {:else}
              <ul class="list-disc space-y-1 pl-6 text-gray-800">
                {#each meeting.summary.decisions as d, i (i)}<li>{d}</li>{/each}
              </ul>
            {/if}
          </div>
          {#if meeting.summary.topics.length > 0}
            <div class="flex flex-col gap-2">
              <h2 class="font-semibold">{t.detail.topics}</h2>
              <div class="flex flex-wrap gap-2">
                {#each meeting.summary.topics as tp, i (i)}
                  <span class="rounded-full bg-gray-200 px-3 py-1 text-sm text-gray-800">{tp}</span>
                {/each}
              </div>
            </div>
          {/if}
        {/if}
      {:else if tab === "actions"}
        {#if !meeting.summary}
          <p class="text-gray-500">{processing ? t.detail.processing : "—"}</p>
        {:else}
          {#if meeting.status === "done"}
            <button
              type="button"
              class="self-end rounded-lg px-3 py-1 text-sm text-indigo-700 hover:bg-indigo-50"
              onclick={() => (editingSummary = true)}
            >
              {t.edit.button}
            </button>
          {/if}
          {#if meeting.actionItems.length === 0}
            <p class="text-gray-600">{t.detail.noActionItems}</p>
          {:else}
          <ul class="flex flex-col divide-y divide-gray-200 rounded-xl border border-gray-200 bg-white">
            {#each meeting.actionItems as a (a.id)}
              <li class="flex items-start gap-3 px-4 py-3">
                <input
                  type="checkbox"
                  class="mt-1 h-4 w-4 shrink-0"
                  aria-label={a.task}
                  bind:checked={a.done}
                  onchange={() => toggleItem(a.id, a.done)}
                />
                <div class="flex flex-col">
                  <span class={a.done ? "text-gray-400 line-through" : "text-gray-900"}>{a.task}</span>
                  {#if a.assignee || a.due}
                    <span class="text-sm text-gray-500">
                      {#if a.assignee}{t.detail.assignee} {a.assignee}{/if}{#if a.assignee && a.due}&nbsp;·&nbsp;{/if}{#if a.due}{t.detail.due}
                        {a.due}{/if}
                    </span>
                  {/if}
                </div>
              </li>
            {/each}
          </ul>
          {/if}
        {/if}
      {:else if transcript.length === 0}
        <p class="text-gray-500">{processing ? t.detail.processing : t.detail.emptyTranscript}</p>
      {:else}
        <form class="flex flex-wrap items-end gap-2 rounded-xl border border-gray-200 bg-white p-3" onsubmit={saveSpeaker}>
          <label class="flex flex-col gap-1">
            <span class="text-sm font-medium">{t.edit.speakerLabel}</span>
            <input class="w-56 rounded-lg border border-gray-300 px-3 py-1.5" maxlength="50" bind:value={speakerDraft} />
          </label>
          <button type="submit" class="rounded-lg border border-gray-300 px-3 py-1.5 text-sm hover:bg-gray-50">
            {t.edit.speakerSave}
          </button>
          <span class="w-full text-xs text-gray-500">{t.edit.speakerHint}</span>
        </form>
        {#if audioSrc || audioLoading}
          <div class="sticky top-0 z-10 flex items-center gap-3 rounded-xl border border-gray-200 bg-white p-2 shadow-sm">
            {#if audioLoading && !audioSrc}<span class="text-sm text-gray-600">{t.detail.preparingAudio}</span>{/if}
            {#if audioSrc}
              <audio bind:this={audioEl} src={audioSrc} controls preload="auto" class="h-9 w-full"></audio>
            {/if}
          </div>
        {/if}
        <ol class={["flex flex-col gap-1.5", transcript.length > 500 && "virtualized"]}>
          {#each transcript as s (s.id)}
            <li class="segment leading-relaxed">
              {#if meeting.audioDeleted}
                <span class="font-mono text-xs text-gray-500">[{formatTimestamp(s.startMs)}]</span>
              {:else}
                <button
                  type="button"
                  class="rounded font-mono text-xs text-indigo-700 hover:bg-indigo-50 hover:underline"
                  title={t.detail.playFrom}
                  onclick={() => playAt(s.startMs)}
                >
                  [{formatTimestamp(s.startMs)}]
                </button>
              {/if}
              <span class={["font-semibold", s.channel === "mic" ? "text-mic" : "text-system"]}>{label(s.channel)}:</span>
              <span class="text-gray-800">{s.text}</span>
            </li>
          {/each}
        </ol>
      {/if}
    </section>
  {/if}
</main>

<dialog
  bind:this={deleteDialog}
  class="m-auto w-full max-w-md rounded-xl p-0 shadow-2xl backdrop:bg-black/40"
  aria-labelledby="delete-title"
>
  <div class="flex flex-col gap-4 p-6">
    <p id="delete-title" class="text-gray-800">{t.detail.deleteConfirm}</p>
    <div class="flex justify-end gap-2">
      <button type="button" class="rounded-lg px-4 py-2 hover:bg-gray-100" onclick={() => deleteDialog?.close()}>
        {t.common.cancel}
      </button>
      <button
        type="button"
        class="rounded-lg bg-red-600 px-4 py-2 font-medium text-white hover:bg-red-700"
        onclick={confirmDelete}
      >
        {t.detail.deleteButton}
      </button>
    </div>
  </div>
</dialog>

<dialog
  bind:this={regenerateDialog}
  class="m-auto w-full max-w-md rounded-xl p-0 shadow-2xl backdrop:bg-black/40"
  aria-labelledby="regen-title"
>
  <div class="flex flex-col gap-4 p-6">
    <p id="regen-title" class="text-gray-800">{t.edit.regenerateConfirm}</p>
    <div class="flex justify-end gap-2">
      <button type="button" class="rounded-lg px-4 py-2 hover:bg-gray-100" onclick={() => regenerateDialog?.close()}>
        {t.common.cancel}
      </button>
      <button
        type="button"
        class="rounded-lg bg-indigo-600 px-4 py-2 font-medium text-white hover:bg-indigo-700"
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
  /* > 500 segment: browser hanya me-render baris yang terlihat (PRD §14.5). */
  .virtualized .segment {
    content-visibility: auto;
    contain-intrinsic-size: auto 1.75rem;
  }
</style>
