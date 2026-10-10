<script lang="ts">
  // Tugas = "apa yang perlu dikerjakan berikutnya" dari semua meeting, siapa pun penanggung jawabnya.
  // Langkah 54 (feedback3 D2): ubah PJ/tenggat, tambah tugas manual, ekspor .ics, pengingat harian.
  import { onDestroy, onMount, tick } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { formatDateTime } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, TaskItem } from "$lib/types";

  const t = id.tasks;

  let items = $state<TaskItem[]>([]);
  let loaded = $state(false);
  let showDone = $state(false);

  // Ubah satu tugas.
  let editingId = $state<number | null>(null);
  let draft = $state({ task: "", assignee: "", dueDate: "" });
  // Tambah tugas ke meeting.
  let addingFor = $state<string | null>(null);
  let newTask = $state({ task: "", assignee: "", dueDate: "" });

  type Group = { meetingId: string; title: string; startedAt: number; items: TaskItem[] };

  /** Dikelompokkan per meeting (meeting terbaru dulu); urutan dari Rust sudah per meeting lalu `idx`. */
  function groupByMeeting(list: TaskItem[]): Group[] {
    const groups: Group[] = [];
    for (const a of list) {
      let g = groups.find((x) => x.meetingId === a.meetingId);
      if (!g) {
        g = { meetingId: a.meetingId, title: a.meetingTitle, startedAt: a.startedAt, items: [] };
        groups.push(g);
      }
      g.items.push(a);
    }
    return groups.sort((x, y) => y.startedAt - x.startedAt);
  }

  const open = $derived(groupByMeeting(items.filter((a) => !a.done)));
  const done = $derived(items.filter((a) => a.done));

  function todayIso(): string {
    const d = new Date();
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }

  /** Status tenggat dari tanggal terstruktur (fallback: tanggal di teks tenggat). */
  function dueState(a: TaskItem): "late" | "today" | null {
    if (a.done) return null;
    const iso = a.dueDate ?? /(\d{4}-\d{2}-\d{2})/.exec(a.due ?? "")?.[1];
    if (!iso) return null;
    const today = todayIso();
    return iso < today ? "late" : iso === today ? "today" : null;
  }

  /** Teks tenggat: tanggal terstruktur ditampilkan rapi, selain itu teks asli dari notulen. */
  function dueLabel(a: TaskItem): string | null {
    if (a.dueDate && (!a.due || a.due === a.dueDate)) {
      const [y, m, d] = a.dueDate.split("-").map(Number);
      return new Date(y, m - 1, d).toLocaleDateString("id-ID", { day: "numeric", month: "short", year: "numeric" });
    }
    return a.due;
  }

  async function load() {
    try {
      items = await api.listActionItems();
    } catch (e) {
      showToast((e as AppError).message, "error");
    } finally {
      loaded = true;
    }
  }

  async function toggle(item: TaskItem) {
    try {
      await api.setActionItemDone(item.id, item.done);
    } catch (e) {
      showToast((e as AppError).message, "error");
      await load();
    }
  }

  async function startEdit(a: TaskItem) {
    editingId = a.id;
    draft = { task: a.task, assignee: a.assignee ?? "", dueDate: a.dueDate ?? "" };
    await tick();
    document.getElementById(`task-edit-${a.id}`)?.focus();
  }

  async function saveEdit(a: TaskItem) {
    editingId = null;
    try {
      await api.updateActionItem(a.id, {
        task: draft.task.trim() !== a.task ? draft.task.trim() : undefined,
        assignee: draft.assignee.trim() !== (a.assignee ?? "") ? draft.assignee.trim() : undefined,
        dueDate: draft.dueDate !== (a.dueDate ?? "") ? draft.dueDate : undefined,
      });
      await load();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function startAdd(meetingId: string) {
    addingFor = meetingId;
    newTask = { task: "", assignee: "", dueDate: "" };
    await tick();
    document.getElementById(`task-add-${meetingId}`)?.focus();
  }

  async function saveAdd(meetingId: string) {
    if (!newTask.task.trim()) {
      addingFor = null;
      return;
    }
    try {
      await api.addActionItem(meetingId, newTask.task.trim(), newTask.assignee.trim() || undefined, newTask.dueDate || undefined);
      addingFor = null;
      await load();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
    await load();
    unlisten.push(await events.meetingUpdated(() => load()));
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

{#snippet task(a: TaskItem)}
  <li class="group flex items-start gap-3 py-2.5">
    <input
      type="checkbox"
      class="mt-1 h-4 w-4 shrink-0"
      aria-label={a.done ? t.markOpen(a.task) : t.markDone(a.task)}
      bind:checked={a.done}
      onchange={() => toggle(a)}
    />
    {#if editingId === a.id}
      <form
        class="flex min-w-0 flex-1 flex-col gap-2"
        onsubmit={(e) => {
          e.preventDefault();
          saveEdit(a);
        }}
      >
        <input id={`task-edit-${a.id}`} class="field" bind:value={draft.task} aria-label={id.edit.task} />
        <div class="flex flex-wrap gap-2">
          <input class="field w-48 py-1.5" bind:value={draft.assignee} placeholder={t.assigneePlaceholder} aria-label={id.edit.assignee} />
          <input type="date" class="field w-44 py-1.5" bind:value={draft.dueDate} aria-label={t.duePick} />
        </div>
        <div class="flex gap-2">
          <button type="submit" class="btn btn-ink btn-sm">{t.save}</button>
          <button type="button" class="btn btn-quiet btn-sm" onclick={() => (editingId = null)}>{t.cancel}</button>
        </div>
      </form>
    {:else}
      <div class="flex min-w-0 flex-1 flex-col gap-0.5">
        <span class={a.done ? "text-ink-faint line-through" : "font-medium"}>{a.task}</span>
        {#if a.assignee || a.due || a.dueDate}
          {@const state = dueState(a)}
          <span class="flex flex-wrap gap-x-4 text-sm text-ink-soft">
            {#if a.assignee}<span>{id.detail.assignee} <span class="text-ink">{a.assignee}</span></span>{/if}
            {#if dueLabel(a)}
              <span class={state === "late" ? "font-semibold text-bad" : state === "today" ? "font-semibold text-warn" : ""}>
                {id.detail.due}
                <span class={state ? "" : "text-ink"}>{dueLabel(a)}</span>{#if state === "late"}&ensp;{t.overdue}{:else if state === "today"}&ensp;{t.dueToday}{/if}
              </span>
            {/if}
          </span>
        {/if}
      </div>
      {#if !a.done}
        <button
          type="button"
          class="btn btn-quiet btn-icon btn-sm opacity-0 group-hover:opacity-100 focus:opacity-100"
          aria-label={`${t.edit}: ${a.task}`}
          title={t.edit}
          onclick={() => startEdit(a)}
        >
          <Icon name="pencil" size={14} />
        </button>
      {/if}
    {/if}
  </li>
{/snippet}

<main class="mx-auto flex w-full max-w-3xl flex-col gap-6 px-6 pt-7 pb-12 xl:px-10">
  <h1 data-tour="tasks-title" class="text-2xl font-bold tracking-[-0.02em]">{t.title}</h1>

  {#if !loaded}
    <ul class="flex flex-col gap-1 motion-safe:animate-pulse" aria-hidden="true">
      {#each [0, 1, 2, 3] as i (i)}
        <li class="flex items-start gap-3 py-3">
          <div class="h-4 w-4 rounded bg-line-soft"></div>
          <div class="flex flex-1 flex-col gap-2">
            <div class="h-4 w-2/3 rounded bg-line-soft"></div>
            <div class="h-3 w-1/3 rounded bg-line-soft"></div>
          </div>
        </li>
      {/each}
    </ul>
  {:else if items.length === 0}
    <section class="flex flex-col items-start gap-2 rounded-xl border border-dashed border-line px-6 py-10">
      <p class="text-lg font-semibold">{t.emptyTitle}</p>
      <p class="max-w-prose text-ink-soft">{t.empty}</p>
    </section>
  {:else}
    {#if open.length === 0}
      <section class="flex items-center gap-3 rounded-xl border border-line bg-paper/50 px-5 py-6">
        <Icon name="check-circle" size={22} class="shrink-0 text-ok" />
        <p class="font-semibold">{t.allDone}</p>
      </section>
    {/if}

    {#each open as g (g.meetingId)}
      <section class="flex flex-col" aria-label={g.title}>
        <h2 class="flex flex-wrap items-baseline gap-x-3 border-b border-line pb-1.5">
          <a href={`/meeting/${g.meetingId}?tab=actions`} class="font-bold hover:underline">{g.title}</a>
          <span class="tabular text-sm text-ink-faint">{formatDateTime(g.startedAt)}</span>
        </h2>
        <ul class="flex flex-col">
          {#each g.items as a (a.id)}{@render task(a)}{/each}
        </ul>
        {#if addingFor === g.meetingId}
          <form
            class="ml-7 flex flex-col gap-2 py-2"
            onsubmit={(e) => {
              e.preventDefault();
              saveAdd(g.meetingId);
            }}
          >
            <input id={`task-add-${g.meetingId}`} class="field" bind:value={newTask.task} placeholder={t.addPlaceholder} aria-label={t.add} />
            <div class="flex flex-wrap gap-2">
              <input class="field w-48 py-1.5" bind:value={newTask.assignee} placeholder={t.assigneePlaceholder} aria-label={id.edit.assignee} />
              <input type="date" class="field w-44 py-1.5" bind:value={newTask.dueDate} aria-label={t.duePick} />
            </div>
            <div class="flex gap-2">
              <button type="submit" class="btn btn-ink btn-sm">{t.save}</button>
              <button type="button" class="btn btn-quiet btn-sm" onclick={() => (addingFor = null)}>{t.cancel}</button>
            </div>
          </form>
        {:else}
          <button
            type="button"
            class="-mx-2 ml-5 flex items-center gap-1.5 self-start rounded-md px-2 py-1 text-sm text-ink-soft hover:bg-wash hover:text-ink"
            onclick={() => startAdd(g.meetingId)}
          >
            <Icon name="plus" size={14} />{t.add}
          </button>
        {/if}
      </section>
    {/each}

    {#if done.length > 0}
      <section class="flex flex-col gap-1">
        <button
          type="button"
          class="-mx-2 flex items-center gap-1.5 self-start rounded-md px-2 py-1 text-sm font-semibold text-ink-soft hover:bg-wash hover:text-ink"
          aria-expanded={showDone}
          onclick={() => (showDone = !showDone)}
        >
          <Icon name="chevron-down" size={16} class={showDone ? "transition-transform" : "-rotate-90 transition-transform"} />
          {t.doneSection(done.length)}
        </button>
        {#if showDone}
          <ul class="flex flex-col">
            {#each done as a (a.id)}{@render task(a)}{/each}
          </ul>
        {/if}
      </section>
    {/if}
  {/if}
</main>
