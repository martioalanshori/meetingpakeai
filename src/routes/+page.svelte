<script lang="ts">
  // Beranda (PLAN-beranda.md + PLAN-evaluasi-beranda.md), disederhanakan 2026-10-09 atas permintaan pemilik:
  // sapaan + kolom Tanya, status hanya jika perlu perhatian, lalu daftar ringkas notulen terbaru & tugas mendesak.
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { api, events } from "$lib/api";
  import { lastAsk } from "$lib/ask.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { formatDateTime, formatTime, formatTimestamp } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { rec } from "$lib/recording.svelte";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, HomeOverview, TaskItem } from "$lib/types";

  const t = id.beranda;
  /** Tugas yang baru dicentang tetap tampil (tercoret) selama ini agar bisa diurungkan. */
  const UNDO_MS = 4000;

  let data = $state<HomeOverview | null>(null);
  let name = $state("");
  let question = $state("");
  let asking = $state(false);
  let importing = $state(false);
  let input = $state<HTMLInputElement | null>(null);
  /** id tugas → timer simpan "selesai" (urungkan membatalkan timer). */
  let pendingDone = $state<Record<number, ReturnType<typeof setTimeout>>>({});

  const greeting = $derived.by(() => {
    const h = new Date().getHours();
    const part = h < 11 ? t.morning : h < 15 ? t.noon : h < 18 ? t.afternoon : t.evening;
    return name && name !== "Saya" ? `${part}, ${name}` : part;
  });

  const recording = $derived(rec.state.status === "recording" || rec.state.status === "paused");
  /** Maks. 2 contoh pertanyaan agar tidak ramai. */
  const suggestions = $derived((data && data.suggestions.length > 0 ? data.suggestions : t.genericSuggestions).slice(0, 2));

  async function load() {
    try {
      data = await api.homeOverview();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function ask(q = question) {
    const text = q.trim();
    if (!text || asking) return;
    question = text;
    asking = true;
    lastAsk.question = text;
    lastAsk.answer = null;
    try {
      lastAsk.answer = await api.askAllMeetings(text);
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      asking = false;
    }
  }

  function closeAnswer() {
    lastAsk.question = "";
    lastAsk.answer = null;
    question = "";
    input?.focus();
  }

  /** Centang: tercoret dulu dengan tombol Urungkan, baru disimpan setelah 4 dtk. */
  function checkTask(task: TaskItem) {
    if (pendingDone[task.id]) return;
    pendingDone[task.id] = setTimeout(async () => {
      delete pendingDone[task.id];
      try {
        await api.setActionItemDone(task.id, true);
      } catch (e) {
        showToast((e as AppError).message, "error");
      }
      await load();
    }, UNDO_MS);
  }

  function undoTask(task: TaskItem) {
    clearTimeout(pendingDone[task.id]);
    delete pendingDone[task.id];
  }

  async function importFile() {
    if (importing) return;
    importing = true;
    try {
      const meetingId = await api.importRecording();
      if (meetingId) await goto(`/meeting/${meetingId}`);
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      importing = false;
    }
  }

  function isoDay(offset = 0): string {
    const d = new Date();
    d.setDate(d.getDate() + offset);
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  /** Label tenggat singkat: lewat tenggat / hari ini / besok / tanggal. */
  function dueLabel(task: TaskItem): { text: string; tone: "late" | "today" | null } | null {
    if (!task.dueDate) return task.due ? { text: task.due, tone: null } : null;
    if (task.dueDate < isoDay()) return { text: t.late, tone: "late" };
    if (task.dueDate === isoDay()) return { text: t.today, tone: "today" };
    if (task.dueDate === isoDay(1)) return { text: t.tomorrow, tone: null };
    const [y, m, d] = task.dueDate.split("-").map(Number);
    return { text: new Date(y, m - 1, d).toLocaleDateString("id-ID", { day: "numeric", month: "short" }), tone: null };
  }

  /** "Sel 14.00" untuk 6 hari terakhir, selain itu tanggal lengkap (jam dari `formatTime`). */
  function when(ms: number): string {
    if ((Date.now() - ms) / 86_400_000 < 6) return `${new Date(ms).toLocaleDateString("id-ID", { weekday: "short" })} ${formatTime(ms)}`;
    return formatDateTime(ms);
  }

  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
    question = lastAsk.question;
    name = await api.getSettings().then((s) => s.userDisplayName, () => "");
    await load();
    unlisten.push(
      await events.meetingUpdated(() => load()),
      await events.jobProgress((p) => {
        if (data?.processing && data.processing.id === p.meetingId) {
          data.processing.status = p.status;
          data.processing.progressDone = p.progressDone;
          data.processing.progressTotal = p.progressTotal;
        } else {
          load();
        }
      }),
    );
  });
  onDestroy(() => {
    unlisten.forEach((u) => u());
    // Centang yang belum tersimpan tetap disimpan saat meninggalkan Beranda.
    for (const [taskId, timer] of Object.entries(pendingDone)) {
      clearTimeout(timer);
      api.setActionItemDone(Number(taskId), true).catch(() => {});
    }
  });
</script>

<main class="mx-auto flex w-full max-w-2xl flex-col px-6 pt-12 pb-16">
  <!-- Sekarang: sapaan → Tanya → status (hanya jika perlu perhatian). -->
  <section class="flex flex-col gap-3" aria-label={t.askLabel}>
    <h1 class="text-2xl font-bold tracking-[-0.02em]">{greeting}</h1>

    <form
      class="mt-1 flex items-center gap-2 rounded-xl border border-line bg-sheet py-1.5 pr-1.5 pl-4 transition-colors focus-within:border-ink-strong"
      onsubmit={(e) => {
        e.preventDefault();
        ask();
      }}
    >
      <Icon name="search" size={18} class="shrink-0 text-ink-faint" />
      <input
        bind:this={input}
        class="min-w-0 flex-1 bg-transparent py-2 text-base outline-none placeholder:text-ink-faint"
        placeholder={t.askPlaceholder}
        aria-label={t.askLabel}
        maxlength="500"
        bind:value={question}
      />
      <button type="submit" class="btn btn-ink btn-icon h-9 w-9" disabled={asking || question.trim() === ""} aria-label={t.ask} title={t.ask}>
        <Icon name="send" size={16} />
      </button>
    </form>

    {#if asking}
      <p class="px-1 text-ink-soft" role="status">{t.asking}</p>
    {:else if lastAsk.answer && lastAsk.question}
      <div class="flex flex-col gap-2.5 px-1" role="status">
        <div class="flex items-start gap-2">
          <p class="flex-1 leading-relaxed whitespace-pre-line">{lastAsk.answer.answer}</p>
          <button type="button" class="btn btn-quiet btn-icon btn-sm shrink-0" aria-label={t.closeAnswer} title={t.closeAnswer} onclick={closeAnswer}>
            <Icon name="x" size={16} />
          </button>
        </div>
        {#if lastAsk.answer.refs.length > 0}
          <ul class="flex flex-wrap gap-1.5">
            {#each lastAsk.answer.refs as r, i (i)}
              <li>
                <a
                  href={`/meeting/${r.meetingId}${r.atMs !== null ? `?tab=transcript&at=${r.atMs}` : ""}`}
                  class="inline-flex items-center gap-1.5 rounded-md bg-wash px-2 py-1 text-sm hover:bg-line-soft"
                >
                  {#if r.atMs !== null}<Icon name="play" size={10} class="text-ink-soft" />{/if}
                  <span class="max-w-56 truncate">{r.title}</span>
                  <span class="tabular text-ink-faint">{r.atMs !== null ? formatTimestamp(r.atMs) : when(r.startedAt)}</span>
                </a>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {:else if data}
      <!-- Contoh pertanyaan dari meeting pengguna sendiri. -->
      <div class="flex flex-wrap gap-2" role="group" aria-label={t.tryAsking}>
        {#each suggestions as s (s)}
          <button
            type="button"
            class="max-w-full truncate rounded-full border border-line px-3 py-1 text-sm text-ink-soft hover:border-ink-faint hover:text-ink"
            onclick={() => ask(s)}>{s}</button
          >
        {/each}
      </div>
    {/if}

    <!-- Status: hanya tampil jika ada yang berjalan atau perlu tindakan (kondisi normal tidak perlu kalimat). -->
    {#if data && (recording || data.processing || data.queuePaused || data.attentionCount > 0 || !data.meetingDetection)}
      <div class="mt-1 px-1 text-sm">
        {#if recording}
          <a href={rec.state.meetingId ? `/meeting/${rec.state.meetingId}?tab=transcript` : "/meetings"} class="group flex items-center gap-2">
            <span class="h-2 w-2 shrink-0 rounded-full bg-rec motion-safe:animate-pulse" aria-hidden="true"></span>
            <span class="font-semibold">{rec.state.status === "paused" ? t.paused : t.recording}</span>
            <span class="text-ink-soft group-hover:underline">{t.viewTranscript}</span>
          </a>
        {:else if data.processing}
          {@const p = data.processing}
          <a href={`/meeting/${p.id}`} class="group flex items-center gap-2">
            <Icon name="refresh" size={14} class="shrink-0 text-ink-soft" />
            <span class="min-w-0 truncate group-hover:underline">{t.processing(p.title)}</span>
            {#if p.progressTotal > 0}<span class="tabular text-ink-faint">{Math.round((p.progressDone / p.progressTotal) * 100)}%</span>{/if}
          </a>
        {:else if data.queuePaused}
          <a href="/settings?tab=ai" class="block border-l-2 border-bad pl-3 font-medium text-bad hover:underline">{t.queuePaused}</a>
        {:else if data.attentionCount > 0 && data.attentionId}
          <a href={`/meeting/${data.attentionId}`} class="block border-l-2 border-warn pl-3 font-medium text-warn hover:underline">
            {t.attention(data.attentionCount)}
          </a>
        {:else if !data.meetingDetection}
          <p class="text-warn">{t.detectionOff}&ensp;<a href="/settings" class="link">{t.openSettings}</a></p>
        {/if}
      </div>
    {/if}
  </section>

  <!-- Sebelumnya: notulen terbaru → tugas mendesak. -->
  {#if !data}
    <div class="mt-12 flex flex-col gap-3 motion-safe:animate-pulse" aria-hidden="true">
      <div class="h-4 w-40 rounded bg-line-soft"></div>
      <div class="h-12 rounded-lg bg-line-soft"></div>
      <div class="h-12 rounded-lg bg-line-soft"></div>
    </div>
  {:else if !data.hasMeetings}
    <section class="mt-12 flex flex-col gap-3">
      <h2 class="section-title">{t.newTitle}</h2>
      <p class="max-w-prose text-ink-soft">{t.newRecord}</p>
      <div class="flex flex-wrap items-center gap-3">
        <span class="text-ink-soft">{t.newImport}</span>
        <button type="button" class="btn btn-line btn-sm" disabled={importing} onclick={importFile}>
          <Icon name="upload" size={14} />{importing ? id.home.importing : id.home.importButton}
        </button>
      </div>
    </section>
  {:else}
    <div class="mt-12 flex flex-col gap-10">
      {#if data.recent.length > 0}
        <section class="flex flex-col" aria-labelledby="recent-title">
          <div class="mb-1 flex items-baseline justify-between gap-3">
            <h2 id="recent-title" class="section-title">{t.recent}</h2>
            <a href="/meetings" class="link text-sm">{t.allMeetings}</a>
          </div>
          <ul class="flex flex-col">
            {#each data.recent as m (m.id)}
              <li>
                <a href={`/meeting/${m.id}`} class="-mx-3 flex items-baseline gap-3 rounded-lg px-3 py-2.5 hover:bg-wash">
                  <span class="min-w-0 flex-1 truncate">{m.title}</span>
                  <span class="tabular shrink-0 text-sm text-ink-faint">{when(m.startedAt)}</span>
                </a>
              </li>
            {/each}
          </ul>
        </section>
      {/if}

      {#if data.urgentTasks.length > 0}
        <section class="flex flex-col" aria-labelledby="tasks-title">
          <div class="mb-1 flex items-baseline justify-between gap-3">
            <h2 id="tasks-title" class="section-title">{t.urgent}</h2>
            <a href="/tasks" class="link text-sm">{t.allTasks(data.openTasks)}</a>
          </div>
          <ul class="flex flex-col">
            {#each data.urgentTasks as task (task.id)}
              {@const due = dueLabel(task)}
              {@const pending = task.id in pendingDone}
              <li class="flex items-start gap-3 py-2">
                <input
                  type="checkbox"
                  class="mt-1 h-4 w-4 shrink-0"
                  aria-label={id.tasks.markDone(task.task)}
                  checked={pending}
                  disabled={pending}
                  onchange={() => checkTask(task)}
                />
                <span class={["min-w-0 flex-1 transition-colors", pending && "text-ink-faint line-through"]}>{task.task}</span>
                {#if pending}
                  <button type="button" class="link shrink-0 text-sm" onclick={() => undoTask(task)}>{t.undo}</button>
                {:else if due}
                  <span
                    class={[
                      "shrink-0 text-sm",
                      due.tone === "late" ? "font-semibold text-bad" : due.tone === "today" ? "font-semibold text-warn" : "text-ink-faint",
                    ]}>{due.text}</span
                  >
                {/if}
              </li>
            {/each}
          </ul>
        </section>
      {/if}
    </div>
  {/if}
</main>
