<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
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

  const snapshot = () => JSON.stringify({ summary, decisions, topics, rows });
  // svelte-ignore state_referenced_locally
  const initial = snapshot();

  async function cancel() {
    if (snapshot() !== initial) {
      const ok = await confirmDialog({
        title: t.edit.discardTitle,
        message: t.edit.discardMessage,
        confirmText: t.edit.discardButton,
        danger: true,
      });
      if (!ok) return;
    }
    oncancel();
  }

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

  const input = "field";
</script>

<form class="flex flex-col gap-5" onsubmit={save}>
  <label class="flex flex-col gap-1">
    <span class="label">{t.detail.summary}</span>
    <textarea class={input} rows="6" bind:value={summary}></textarea>
  </label>

  <label class="flex flex-col gap-1">
    <span class="label">{t.detail.decisions}</span>
    <textarea class={input} rows="4" bind:value={decisions}></textarea>
    <span class="hint">{t.edit.decisionsHint}</span>
  </label>

  <label class="flex flex-col gap-1">
    <span class="label">{t.detail.topics}</span>
    <input class={input} bind:value={topics} />
    <span class="hint">{t.edit.topicsHint}</span>
  </label>

  <fieldset class="flex flex-col gap-2">
    <legend class="label mb-1">{t.detail.tabActionItems}</legend>
    {#each rows as r, i (i)}
      <div class="flex flex-wrap items-center gap-2 rounded-lg border border-line bg-sheet p-2">
        <input type="checkbox" class="h-4 w-4" bind:checked={r.done} aria-label={t.edit.done} />
        <input class={[input, "min-w-48 flex-1"]} placeholder={t.edit.task} aria-label={t.edit.task} bind:value={r.task} />
        <input class={[input, "w-36"]} placeholder={t.edit.assignee} aria-label={t.edit.assignee} bind:value={r.assignee} />
        <input class={[input, "w-36"]} placeholder={t.edit.due} aria-label={t.edit.due} bind:value={r.due} />
        <button
          type="button"
          class="btn btn-danger"
          onclick={() => rows.splice(i, 1)}
        >
          {t.edit.remove}
        </button>
      </div>
    {/each}
    <button type="button" class="self-start btn btn-quiet" onclick={addRow}>
      <Icon name="plus" size={16} />
      {t.edit.addItem}
    </button>
  </fieldset>

  <div class="flex gap-2">
    <button
      type="submit"
      class="btn btn-ink"
      disabled={saving}
    >
      {saving ? t.edit.saving : t.edit.save}
    </button>
    <button type="button" class="btn btn-quiet" onclick={cancel}>{t.common.cancel}</button>
  </div>
</form>
