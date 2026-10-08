<script lang="ts">
  // Draf pesan tindak lanjut (langkah 43): dibuat AI dari notulen, disimpan per meeting.
  import { onMount } from "svelte";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { api } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import { textToHtml } from "$lib/minutes";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, FollowUp } from "$lib/types";

  let {
    meetingId,
    initial,
    onclose,
  }: { meetingId: string; initial: FollowUp | null; onclose: () => void } = $props();

  const t = id.followUp;
  let draft = $state<FollowUp | null>(null);
  let lang = $state<"id" | "en">("id");
  let busy = $state(false);
  let error = $state<string | null>(null);
  let copied = $state(false);

  async function generate(force: boolean) {
    busy = true;
    error = null;
    try {
      draft = await api.generateFollowUp(meetingId, lang, force);
    } catch (e) {
      error = (e as AppError).message;
    } finally {
      busy = false;
    }
  }

  function switchLang(next: "id" | "en") {
    if (next === lang || busy) return;
    lang = next;
    generate(false);
  }

  async function copy() {
    if (!draft) return;
    const plain = `${draft.subject}\n\n${draft.body}`;
    try {
      const html = `<p><b>${escapeHtml(draft.subject)}</b></p>${textToHtml(draft.body)}`;
      await navigator.clipboard.write([
        new ClipboardItem({
          "text/plain": new Blob([plain], { type: "text/plain" }),
          "text/html": new Blob([html], { type: "text/html" }),
        }),
      ]);
    } catch {
      try {
        await navigator.clipboard.writeText(plain);
      } catch {
        showToast(id.minutes.copyFailed, "error");
        return;
      }
    }
    copied = true;
    setTimeout(() => (copied = false), 2000);
    showToast(t.copied, "success", 2000);
  }

  function openMail() {
    if (!draft) return;
    const url = `mailto:?subject=${encodeURIComponent(draft.subject)}&body=${encodeURIComponent(draft.body)}`;
    openUrl(url).catch(() => showToast(t.mailFailed, "error"));
  }

  function escapeHtml(s: string): string {
    return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
  }

  onMount(() => {
    if (initial) {
      lang = initial.lang;
      draft = initial;
    } else {
      generate(false);
    }
  });
</script>

<section class="flex flex-col gap-3 rounded-xl border border-line bg-paper/60 p-4" aria-label={t.title} aria-busy={busy}>
  <div class="flex flex-wrap items-center gap-2">
    <h2 class="flex-1 font-bold">{t.title}</h2>
    <div class="flex rounded-[var(--radius-ctl)] border border-line bg-sheet p-0.5 text-sm" role="group" aria-label={t.language}>
      {#each [["id", "Indonesia"], ["en", "English"]] as [code, label] (code)}
        <button
          type="button"
          class={["rounded-[calc(var(--radius-ctl)-2px)] px-2.5 py-0.5", lang === code ? "bg-ink-strong text-white" : "text-ink-soft hover:text-ink"]}
          aria-pressed={lang === code}
          disabled={busy}
          onclick={() => switchLang(code as "id" | "en")}>{label}</button
        >
      {/each}
    </div>
    <button type="button" class="btn btn-quiet btn-icon btn-sm" aria-label={id.common.close} title={id.common.close} onclick={onclose}>
      <Icon name="x" size={16} />
    </button>
  </div>

  {#if busy}
    <div class="flex flex-col gap-2 py-1 motion-safe:animate-pulse" aria-hidden="true">
      <div class="h-4 w-2/5 rounded bg-line-soft"></div>
      <div class="h-3 w-full rounded bg-line-soft"></div>
      <div class="h-3 w-11/12 rounded bg-line-soft"></div>
      <div class="h-3 w-3/4 rounded bg-line-soft"></div>
    </div>
    <p class="hint" role="status">{t.generating}</p>
  {:else if error}
    <div role="alert" class="flex flex-wrap items-center gap-3 rounded-lg bg-bad-wash px-3 py-2 text-bad">
      <span class="flex-1 text-sm font-medium">{error}</span>
      <button type="button" class="btn btn-ink btn-sm" onclick={() => generate(false)}>{id.common.retry}</button>
    </div>
  {:else if draft}
    <div class="flex flex-col gap-2 rounded-lg border border-line bg-sheet p-4">
      <p class="text-sm text-ink-soft">{t.subject} <span class="font-semibold text-ink">{draft.subject}</span></p>
      <p class="max-w-[68ch] leading-relaxed whitespace-pre-line">{draft.body}</p>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      <button type="button" class="btn btn-ink btn-sm" onclick={copy}>
        <Icon name={copied ? "check" : "copy"} size={14} />{copied ? id.minutes.copiedShort : t.copy}
      </button>
      <button type="button" class="btn btn-line btn-sm" onclick={openMail}><Icon name="send" size={14} />{t.openMail}</button>
      <button type="button" class="btn btn-quiet btn-sm ml-auto" onclick={() => generate(true)}>
        <Icon name="refresh" size={14} />{t.regenerate}
      </button>
    </div>
  {/if}
</section>
