<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import { formatDateTime } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, TaskItem } from "$lib/types";

  const t = id.tasks;

  let items = $state<TaskItem[]>([]);
  let loaded = $state(false);
  let myName = $state("Saya");
  let onlyMine = $state(true);
  let showDone = $state(false);

  /** PJ dianggap "saya" jika memuat nama pengguna atau label "Saya". */
  function isMine(assignee: string | null): boolean {
    if (!assignee) return false;
    const a = assignee.toLowerCase();
    return [myName, "saya"].some((n) => n.trim() !== "" && a.includes(n.trim().toLowerCase()));
  }

  const visible = $derived(items.filter((a) => (showDone || !a.done) && (!onlyMine || isMine(a.assignee))));

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
    myName = await api.getSettings().then((s) => s.userDisplayName, () => "Saya");
    await load();
    unlisten.push(await events.meetingUpdated(() => load()));
  });
  onDestroy(() => unlisten.forEach((u) => u()));
</script>

<main class="mx-auto flex w-full max-w-3xl flex-col gap-5 px-8 pt-7 pb-12">
  <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.title}</h1>

  <div class="flex flex-wrap gap-x-6 gap-y-2 text-sm">
    <label class="flex items-center gap-2">
      <input type="checkbox" class="h-4 w-4" bind:checked={onlyMine} />
      {t.onlyMine(myName)}
    </label>
    <label class="flex items-center gap-2">
      <input type="checkbox" class="h-4 w-4" bind:checked={showDone} />
      {t.showDone}
    </label>
  </div>

  {#if !loaded}
    <p class="text-ink-soft">{id.common.loading}</p>
  {:else if visible.length === 0}
    <section class="flex flex-col items-start gap-2 rounded-xl border border-dashed border-line px-6 py-10">
      <p class="text-lg font-semibold">{t.emptyTitle}</p>
      <p class="max-w-prose text-ink-soft">{t.empty}</p>
    </section>
  {:else}
    <ul class="flex flex-col">
      {#each visible as a (a.id)}
        <li class="flex items-start gap-3 border-b border-line-soft py-3 last:border-b-0">
          <input
            type="checkbox"
            class="mt-1 h-4 w-4 shrink-0"
            aria-label={a.task}
            bind:checked={a.done}
            onchange={() => toggle(a)}
          />
          <div class="flex min-w-0 flex-col gap-0.5">
            <span class={a.done ? "text-ink-faint line-through" : "font-medium"}>{a.task}</span>
            <span class="flex flex-wrap gap-x-4 text-sm text-ink-soft">
              {#if a.assignee}<span>{id.detail.assignee} <span class="text-ink">{a.assignee}</span></span>{/if}
              {#if a.due}<span>{id.detail.due} <span class="text-ink">{a.due}</span></span>{/if}
              <a href={`/meeting/${a.meetingId}?tab=actions`} class="link">{a.meetingTitle}</a>
              <span class="tabular">{formatDateTime(a.startedAt)}</span>
            </span>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</main>
