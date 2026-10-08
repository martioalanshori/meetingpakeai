<script lang="ts">
  import { id as t } from "$lib/i18n/id";
  import type { MeetingDetail, SummaryEdit } from "$lib/types";

  // Editor ringkasan + action item (langkah 23). Keputusan satu per baris, topik dipisah koma.
  let {
    meeting,
    onsave,
    oncancel,
  }: { meeting: MeetingDetail; onsave: (edit: SummaryEdit) => Promise<void>; oncancel: () => void } = $props();

  type Row = { task: string; assignee: string; due: string; done: boolean };

  // Salinan awal sengaja diambil sekali saat editor dibuka.
  // svelte-ignore state_referenced_locally
  const s = meeting.summary;
  let summary = $state(s?.summary ?? "");
  let decisions = $state((s?.decisions ?? []).join("\n"));
  let topics = $state((s?.topics ?? []).join(", "));
  let rows = $state<Row[]>(
    // svelte-ignore state_referenced_locally
    meeting.actionItems.map((a) => ({ task: a.task, assignee: a.assignee ?? "", due: a.due ?? "", done: a.done })),
  );
  let saving = $state(false);

  function addRow() {
    rows.push({ task: "", assignee: "", due: "", done: false });
  }

  async function save(e: SubmitEvent) {
    e.preventDefault();
    saving = true;
    try {
      await onsave({
        summary,
        decisions: decisions.split("\n"),
        topics: topics.split(","),
        actionItems: rows.map((r) => ({ task: r.task, assignee: r.assignee || null, due: r.due || null, done: r.done })),
      });
    } finally {
      saving = false;
    }
  }

  const input = "rounded-lg border border-gray-300 px-3 py-2";
</script>

<form class="flex flex-col gap-5" onsubmit={save}>
  <label class="flex flex-col gap-1">
    <span class="font-semibold">{t.detail.summary}</span>
    <textarea class={input} rows="6" bind:value={summary}></textarea>
  </label>

  <label class="flex flex-col gap-1">
    <span class="font-semibold">{t.detail.decisions}</span>
    <textarea class={input} rows="4" bind:value={decisions}></textarea>
    <span class="text-sm text-gray-500">{t.edit.decisionsHint}</span>
  </label>

  <label class="flex flex-col gap-1">
    <span class="font-semibold">{t.detail.topics}</span>
    <input class={input} bind:value={topics} />
    <span class="text-sm text-gray-500">{t.edit.topicsHint}</span>
  </label>

  <fieldset class="flex flex-col gap-2">
    <legend class="mb-1 font-semibold">{t.detail.tabActionItems}</legend>
    {#each rows as r, i (i)}
      <div class="flex flex-wrap items-center gap-2 rounded-lg border border-gray-200 bg-white p-2">
        <input type="checkbox" class="h-4 w-4" bind:checked={r.done} aria-label={t.edit.done} />
        <input class={[input, "min-w-48 flex-1"]} placeholder={t.edit.task} aria-label={t.edit.task} bind:value={r.task} />
        <input class={[input, "w-36"]} placeholder={t.edit.assignee} aria-label={t.edit.assignee} bind:value={r.assignee} />
        <input class={[input, "w-36"]} placeholder={t.edit.due} aria-label={t.edit.due} bind:value={r.due} />
        <button
          type="button"
          class="rounded-lg px-2 py-1 text-sm text-red-700 hover:bg-red-50"
          onclick={() => rows.splice(i, 1)}
        >
          {t.edit.remove}
        </button>
      </div>
    {/each}
    <button type="button" class="self-start rounded-lg px-3 py-1.5 text-sm text-indigo-700 hover:bg-indigo-50" onclick={addRow}>
      {t.edit.addItem}
    </button>
  </fieldset>

  <div class="flex gap-2">
    <button
      type="submit"
      class="rounded-lg bg-indigo-600 px-5 py-2 font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
      disabled={saving}
    >
      {t.edit.save}
    </button>
    <button type="button" class="rounded-lg px-4 py-2 hover:bg-gray-100" onclick={oncancel}>{t.common.cancel}</button>
  </div>
</form>
