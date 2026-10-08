<script lang="ts">
  // Tugas = "apa yang perlu dikerjakan berikutnya" dari semua meeting, siapa pun penanggung jawabnya.
  import { onDestroy, onMount } from "svelte";
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
  const openCount = $derived(items.filter((a) => !a.done).length);
  const done = $derived(items.filter((a) => a.done));

  /** Tanggal `YYYY-MM-DD` di teks tenggat (mis. "Jumat depan (2026-10-16)") sudah lewat. */
  function overdue(due: string | null): boolean {
    const m = due && /(\d{4})-(\d{2})-(\d{2})/.exec(due);
    if (!m) return false;
    const d = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    return d < today;
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

  const unlisten: UnlistenFn[] = [];
  onMount(async () => {
    await load();
    unlisten.push(await events.meetingUpdated(() => load()));
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

{#snippet task(a: TaskItem)}
  <li class="flex items-start gap-3 py-2.5">
    <input
      type="checkbox"
      class="mt-1 h-4 w-4 shrink-0"
      aria-label={a.done ? t.markOpen(a.task) : t.markDone(a.task)}
      bind:checked={a.done}
      onchange={() => toggle(a)}
    />
    <div class="flex min-w-0 flex-col gap-0.5">
      <span class={a.done ? "text-ink-faint line-through" : "font-medium"}>{a.task}</span>
      {#if a.assignee || a.due}
        <span class="flex flex-wrap gap-x-4 text-sm text-ink-soft">
          {#if a.assignee}<span>{id.detail.assignee} <span class="text-ink">{a.assignee}</span></span>{/if}
          {#if a.due}
            {@const late = !a.done && overdue(a.due)}
            <span class={late ? "font-semibold text-bad" : ""}>
              {id.detail.due} <span class={late ? "" : "text-ink"}>{a.due}</span>{#if late}&ensp;{t.overdue}{/if}
            </span>
          {/if}
        </span>
      {/if}
    </div>
  </li>
{/snippet}

<main class="mx-auto flex w-full max-w-3xl flex-col gap-6 px-6 pt-7 pb-12 xl:px-10">
  <div class="flex flex-col gap-1">
    <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.title}</h1>
    <p class="text-ink-soft">{loaded && openCount > 0 ? t.subtitleCount(openCount) : t.subtitle}</p>
  </div>

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
