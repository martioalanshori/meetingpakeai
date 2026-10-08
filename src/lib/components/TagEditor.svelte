<script lang="ts">
  // Label proyek/klien di detail meeting (langkah 59, feedback3 D4): chip + input dengan saran label yang ada.
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError } from "$lib/types";

  let { meetingId, tags }: { meetingId: string; tags: string[] } = $props();

  const t = id.detail;
  let current = $state<string[]>([]);
  let adding = $state(false);
  let draft = $state("");
  let known = $state<string[]>([]);
  const listId = `tags-${Math.random().toString(36).slice(2)}`;

  $effect(() => {
    current = [...tags];
  });

  async function save(next: string[]) {
    try {
      current = await api.setMeetingTags(meetingId, next);
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  async function add() {
    const v = draft.trim();
    draft = "";
    adding = false;
    if (v && !current.some((c) => c.toLowerCase() === v.toLowerCase())) await save([...current, v]);
  }

  onMount(async () => {
    known = await api.listTags().catch(() => []);
  });
</script>

<div class="flex flex-wrap items-center gap-1.5" aria-label={t.tags}>
  {#each current as tag (tag)}
    <span class="inline-flex items-center gap-1 rounded-full bg-wash py-0.5 pr-1 pl-2.5 text-sm">
      {tag}
      <button
        type="button"
        class="rounded-full p-0.5 text-ink-faint hover:bg-line-soft hover:text-ink"
        aria-label={t.removeTag(tag)}
        title={t.removeTag(tag)}
        onclick={() => save(current.filter((c) => c !== tag))}
      >
        <Icon name="x" size={12} />
      </button>
    </span>
  {/each}
  {#if adding}
    <form
      class="inline-flex"
      onsubmit={(e) => {
        e.preventDefault();
        add();
      }}
    >
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="field w-44 rounded-full px-3 py-0.5 text-sm"
        list={listId}
        maxlength="40"
        placeholder={t.tagPlaceholder}
        aria-label={t.addTag}
        bind:value={draft}
        autofocus
        onblur={add}
        onkeydown={(e) => {
          if (e.key === "Escape") {
            draft = "";
            adding = false;
          }
        }}
      />
      <datalist id={listId}>
        {#each known.filter((k) => !current.includes(k)) as k (k)}<option value={k}></option>{/each}
      </datalist>
    </form>
  {:else if current.length < 6}
    <button
      type="button"
      class="inline-flex items-center gap-1 rounded-full border border-dashed border-line px-2.5 py-0.5 text-sm text-ink-soft hover:border-ink-faint hover:text-ink"
      onclick={() => (adding = true)}
    >
      <Icon name="plus" size={12} />{t.addTag}
    </button>
  {/if}
</div>
