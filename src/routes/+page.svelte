<script lang="ts">
  // Beranda (PLAN-beranda.md): tampilan pertama, sengaja sederhana — tanya, status, notulen terbaru, tugas mendesak.
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { formatDateTime, formatTimestamp } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { rec } from "$lib/recording.svelte";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, AskAllResult, HomeOverview, TaskItem } from "$lib/types";

  const t = id.beranda;

  let data = $state<HomeOverview | null>(null);
  let name = $state("");
  let question = $state("");
  let asked = $state("");
  let answer = $state<AskAllResult | null>(null);
  let asking = $state(false);
  let importing = $state(false);

  const greeting = $derived.by(() => {
    const h = new Date().getHours();
    const part = h < 11 ? t.morning : h < 15 ? t.noon : h < 19 ? t.afternoon : t.evening;
    return name && name !== "Saya" ? `${part}, ${name}` : part;
  });

  const recording = $derived(rec.state.status === "recording" || rec.state.status === "paused");

  async function load() {
    try {
      data = await api.homeOverview();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function ask() {
    const q = question.trim();
    if (!q || asking) return;
    asking = true;
    asked = q;
    answer = null;
    try {
      answer = await api.askAllMeetings(q);
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      asking = false;
    }
  }

  async function toggleTask(task: TaskItem) {
    try {
      await api.setActionItemDone(task.id, task.done);
      await load();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function enableAutostart() {
    try {
      await api.updateSettings({ autostart: true });
      await load();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
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

  /** "Sel 14.00" untuk 6 hari terakhir, selain itu tanggal lengkap. */
  function when(ms: number): string {
    const d = new Date(ms);
    const time = `${String(d.getHours()).padStart(2, "0")}.${String(d.getMinutes()).padStart(2, "0")}`;
    if ((Date.now() - ms) / 86_400_000 < 6) return `${d.toLocaleDateString("id-ID", { weekday: "short" })} ${time}`;
    return formatDateTime(ms);
  }

  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
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
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<main class="mx-auto flex w-full max-w-2xl flex-col gap-8 px-6 pt-10 pb-16">
  <header class="flex flex-col gap-1">
    <h1 class="text-2xl font-bold tracking-[-0.02em]">{greeting}</h1>
    {#if data}
      {#if !data.meetingDetection}
        <p class="text-sm text-warn">{t.detectionOff}&ensp;<a href="/settings" class="link">{t.openSettings}</a></p>
      {:else if !data.autostart}
        <p class="text-sm text-ink-soft">
          {t.readyNoAutostart}&ensp;<button type="button" class="link" onclick={enableAutostart}>{t.enableAutostart}</button>
        </p>
      {:else}
        <p class="flex items-center gap-1.5 text-sm text-ink-soft">
          <span class="h-1.5 w-1.5 rounded-full bg-ok" aria-hidden="true"></span>{t.ready}
        </p>
      {/if}
    {/if}
  </header>

  <!-- Tanya semua meeting. -->
  <section class="flex flex-col gap-3" aria-label={t.askLabel}>
    <form
      class="flex items-center gap-2 rounded-2xl border border-line bg-sheet p-1.5 pl-4 shadow-[0_8px_24px_-16px_rgb(30_36_51/0.35)] focus-within:border-ink-faint"
      onsubmit={(e) => {
        e.preventDefault();
        ask();
      }}
    >
      <Icon name="search" size={18} class="shrink-0 text-ink-faint" />
      <input
        class="min-w-0 flex-1 bg-transparent py-2 text-base outline-none"
        placeholder={t.askPlaceholder}
        aria-label={t.askLabel}
        maxlength="500"
        bind:value={question}
      />
      <button type="submit" class="btn btn-ink btn-icon" disabled={asking || question.trim() === ""} aria-label={t.ask} title={t.ask}>
        <Icon name="send" size={16} />
      </button>
    </form>
    {#if asking}
      <p class="hint motion-safe:animate-pulse" role="status">{t.asking}</p>
    {:else if answer && asked}
      <div class="flex flex-col gap-2 px-1" role="status">
        <p class="leading-relaxed whitespace-pre-line">{answer.answer}</p>
        {#if answer.refs.length > 0}
          <ul class="flex flex-col gap-0.5">
            {#each answer.refs as r, i (i)}
              <li>
                <a class="link text-sm" href={`/meeting/${r.meetingId}${r.atMs !== null ? `?tab=transcript&at=${r.atMs}` : ""}`}>
                  {r.title}<span class="tabular text-ink-faint">&ensp;{when(r.startedAt)}{#if r.atMs !== null}&ensp;{formatTimestamp(r.atMs)}{/if}</span>
                </a>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  </section>

  {#if !data}
    <div class="flex flex-col gap-3 motion-safe:animate-pulse" aria-hidden="true">
      <div class="h-4 w-40 rounded bg-line-soft"></div>
      <div class="h-12 rounded-lg bg-line-soft"></div>
      <div class="h-12 rounded-lg bg-line-soft"></div>
    </div>
  {:else}
    <!-- Satu baris status, hanya jika ada. -->
    {#if recording}
      <a
        href={rec.state.meetingId ? `/meeting/${rec.state.meetingId}?tab=transcript` : "/meetings"}
        class="flex items-center gap-2.5 rounded-xl bg-wash px-4 py-3 hover:bg-line-soft"
      >
        <span class="h-2.5 w-2.5 rounded-full bg-rec motion-safe:animate-pulse" aria-hidden="true"></span>
        <span class="flex-1 font-medium">{rec.state.status === "paused" ? t.paused : t.recording}</span>
        <span class="text-sm text-ink-soft">{t.viewTranscript}</span>
      </a>
    {:else if data.processing}
      {@const p = data.processing}
      <a href={`/meeting/${p.id}`} class="flex items-center gap-2.5 rounded-xl bg-wash px-4 py-3 hover:bg-line-soft">
        <Icon name="refresh" size={16} class="shrink-0 text-ink-soft motion-safe:animate-spin" />
        <span class="min-w-0 flex-1 truncate">{t.processing(p.title)}</span>
        {#if p.progressTotal > 0}<span class="tabular text-sm text-ink-soft">{Math.round((p.progressDone / p.progressTotal) * 100)}%</span>{/if}
      </a>
    {:else if data.queuePaused}
      <a href="/settings?tab=ai" class="flex items-center gap-2.5 rounded-xl bg-bad-wash px-4 py-3 text-bad">
        <Icon name="alert-circle" size={16} class="shrink-0" /><span class="flex-1 font-medium">{t.queuePaused}</span>
      </a>
    {:else if data.attentionCount > 0 && data.attentionId}
      <a href={`/meeting/${data.attentionId}`} class="flex items-center gap-2.5 rounded-xl bg-warn-wash px-4 py-3 text-warn">
        <Icon name="alert-circle" size={16} class="shrink-0" /><span class="flex-1 font-medium">{t.attention(data.attentionCount)}</span>
        <span class="text-sm">{t.view}</span>
      </a>
    {/if}

    {#if !data.hasMeetings}
      <!-- Pengguna baru. -->
      <section class="flex flex-col gap-3 rounded-xl border border-dashed border-line px-5 py-6">
        <p class="font-semibold">{t.newTitle}</p>
        <p class="text-ink-soft">{t.newRecord}</p>
        <div class="flex flex-wrap items-center gap-3">
          <span class="text-ink-soft">{t.newImport}</span>
          <button type="button" class="btn btn-line btn-sm" disabled={importing} onclick={importFile}>
            <Icon name="download" size={14} />{importing ? id.home.importing : id.home.importButton}
          </button>
        </div>
      </section>
    {:else}
      {#if data.recent.length > 0}
        <section class="flex flex-col gap-1" aria-labelledby="recent-title">
          <h2 id="recent-title" class="section-title mb-1">{t.recent}</h2>
          <ul class="flex flex-col">
            {#each data.recent as m (m.id)}
              <li>
                <a href={`/meeting/${m.id}`} class="-mx-3 flex flex-col gap-0.5 rounded-lg px-3 py-2.5 hover:bg-wash">
                  <span class="flex items-baseline gap-3">
                    <span class="min-w-0 flex-1 truncate font-semibold">{m.title}</span>
                    <span class="tabular shrink-0 text-sm text-ink-faint">{when(m.startedAt)}</span>
                  </span>
                  {#if m.line}<span class="line-clamp-2 text-ink-soft">{m.line}</span>{/if}
                </a>
              </li>
            {/each}
          </ul>
          <a href="/meetings" class="link mt-1 self-start text-sm">{t.allMeetings}</a>
        </section>
      {/if}

      {#if data.urgentTasks.length > 0}
        <section class="flex flex-col gap-1" aria-labelledby="tasks-title">
          <h2 id="tasks-title" class="section-title mb-1">{t.urgent}</h2>
          <ul class="flex flex-col">
            {#each data.urgentTasks as task (task.id)}
              {@const due = dueLabel(task)}
              <li class="flex items-start gap-3 py-2">
                <input
                  type="checkbox"
                  class="mt-1 h-4 w-4 shrink-0"
                  aria-label={id.tasks.markDone(task.task)}
                  bind:checked={task.done}
                  onchange={() => toggleTask(task)}
                />
                <span class="min-w-0 flex-1">{task.task}</span>
                {#if due}
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
          <a href="/tasks" class="link mt-1 self-start text-sm">{t.allTasks(data.openTasks)}</a>
        </section>
      {/if}
    {/if}
  {/if}
</main>
