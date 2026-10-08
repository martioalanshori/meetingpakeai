<script lang="ts">
  // "Tanya meeting ini" (langkah 51, feedback3 C1): jawaban dari transkrip & notulen dengan chip waktu sumber.
  import { onMount, tick } from "svelte";
  import { api } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { formatTimestamp } from "$lib/format";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, QaItem } from "$lib/types";

  let { meetingId, onjump }: { meetingId: string; onjump: (ms: number) => void } = $props();

  const t = id.ask;
  let items = $state<QaItem[]>([]);
  let question = $state("");
  let busy = $state(false);
  let pending = $state("");
  let input = $state<HTMLTextAreaElement | null>(null);
  let end = $state<HTMLDivElement | null>(null);

  async function ask(q?: string) {
    const text = (q ?? question).trim();
    if (!text || busy) return;
    busy = true;
    pending = text;
    question = "";
    await tick();
    end?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    try {
      items = [...items, await api.askMeeting(meetingId, text)];
    } catch (e) {
      question = text;
      showToast((e as AppError).message, "error");
    } finally {
      busy = false;
      pending = "";
      await tick();
      end?.scrollIntoView({ block: "nearest", behavior: "smooth" });
      input?.focus();
    }
  }

  async function clearAll() {
    try {
      await api.clearMeetingQa(meetingId);
      items = [];
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }

  onMount(async () => {
    items = await api.listMeetingQa(meetingId).catch(() => []);
  });
</script>

<section class="flex max-w-[72ch] flex-col gap-5" aria-label={t.title}>
  {#if items.length === 0 && !busy}
    <div class="flex flex-col gap-3">
      <p class="text-ink-soft">{t.intro}</p>
      <div class="flex flex-wrap gap-2">
        {#each t.suggestions as s (s)}
          <button type="button" class="rounded-full border border-line px-3 py-1.5 text-sm hover:border-ink-faint hover:bg-wash" onclick={() => ask(s)}>
            {s}
          </button>
        {/each}
      </div>
    </div>
  {/if}

  {#each items as qa (qa.id)}
    <article class="flex flex-col gap-2">
      <p class="self-end rounded-2xl rounded-br-md bg-ink-strong px-4 py-2 text-white">{qa.question}</p>
      <div class="flex flex-col gap-2 rounded-2xl rounded-bl-md border border-line bg-sheet px-4 py-3">
        <p class="leading-relaxed whitespace-pre-line">{qa.answer}</p>
        {#if qa.sources.length > 0}
          <div class="flex flex-wrap items-center gap-1.5">
            <span class="text-xs text-ink-faint">{t.sources}</span>
            {#each qa.sources as ms (ms)}
              <button
                type="button"
                class="tabular inline-flex items-center gap-1 rounded-md bg-wash px-1.5 py-px text-xs text-ink-soft hover:bg-line-soft hover:text-ink"
                title={id.detail.sourceAt(formatTimestamp(ms))}
                onclick={() => onjump(ms)}
              >
                <Icon name="play" size={10} />{formatTimestamp(ms)}
              </button>
            {/each}
          </div>
        {/if}
      </div>
    </article>
  {/each}

  {#if busy}
    <article class="flex flex-col gap-2" aria-busy="true">
      <p class="self-end rounded-2xl rounded-br-md bg-ink-strong px-4 py-2 text-white">{pending}</p>
      <p class="hint motion-safe:animate-pulse" role="status">{t.thinking}</p>
    </article>
  {/if}
  <div bind:this={end}></div>

  <form
    class="sticky bottom-3 flex items-end gap-2 rounded-2xl border border-line bg-sheet p-2 shadow-[0_8px_24px_-12px_rgb(30_36_51/0.3)]"
    onsubmit={(e) => {
      e.preventDefault();
      ask();
    }}
  >
    <textarea
      bind:this={input}
      bind:value={question}
      rows="1"
      maxlength="500"
      class="max-h-32 min-h-10 flex-1 resize-none bg-transparent px-2 py-2 outline-none"
      placeholder={t.placeholder}
      aria-label={t.placeholder}
      onkeydown={(e) => {
        if (e.key === "Enter" && !e.shiftKey) {
          e.preventDefault();
          ask();
        }
      }}
    ></textarea>
    <button type="submit" class="btn btn-ink btn-icon" disabled={busy || question.trim() === ""} aria-label={t.send} title={t.send}>
      <Icon name="send" size={16} />
    </button>
  </form>
  {#if items.length > 0}
    <button type="button" class="link self-start text-sm" onclick={clearAll}>{t.clear}</button>
  {/if}
</section>
