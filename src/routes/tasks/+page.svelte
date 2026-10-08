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

<main class="mx-auto flex max-w-4xl flex-col gap-4 p-6">
  <a href="/" class="text-sm text-indigo-700 hover:underline">{id.common.back}</a>
  <h1 class="text-xl font-semibold">{t.title}</h1>

  <div class="flex flex-wrap gap-4 text-sm">
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
    <p class="text-gray-500">{id.common.loading}</p>
  {:else if visible.length === 0}
    <p class="rounded-xl border border-dashed border-gray-300 bg-white p-8 text-center text-gray-600">{t.empty}</p>
  {:else}
    <ul class="flex flex-col divide-y divide-gray-200 rounded-xl border border-gray-200 bg-white">
      {#each visible as a (a.id)}
        <li class="flex items-start gap-3 px-4 py-3">
          <input
            type="checkbox"
            class="mt-1 h-4 w-4 shrink-0"
            aria-label={a.task}
            bind:checked={a.done}
            onchange={() => toggle(a)}
          />
          <div class="flex min-w-0 flex-col">
            <span class={a.done ? "text-gray-400 line-through" : "text-gray-900"}>{a.task}</span>
            <span class="text-sm text-gray-500">
              {#if a.assignee}{id.detail.assignee} {a.assignee} · {/if}{#if a.due}{id.detail.due} {a.due} · {/if}
              <a href={`/meeting/${a.meetingId}`} class="text-indigo-700 hover:underline">{a.meetingTitle}</a>
              ({formatDateTime(a.startedAt)})
            </span>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</main>
