<script lang="ts">
  // "Catatan saya" (langkah 49, feedback3 B2): tersimpan otomatis; dipakai di jendela catatan saat merekam
  // dan di detail meeting. Catatan ikut dikirim ke AI saat notulen dibuat.
  import { onDestroy, onMount } from "svelte";
  import { api } from "$lib/api";
  import { id } from "$lib/i18n/id";
  import type { AppError } from "$lib/types";

  let {
    meetingId,
    rows = 6,
    autofocus = false,
    dark = false,
  }: { meetingId: string; rows?: number; autofocus?: boolean; dark?: boolean } = $props();

  const t = id.notes;
  const MAX = 5000;
  let text = $state("");
  let saved = $state("");
  let saveState = $state<"idle" | "saving" | "saved" | "error">("idle");
  let loaded = $state(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  let el = $state<HTMLTextAreaElement | null>(null);

  async function save() {
    clearTimeout(timer);
    if (!loaded || text === saved) return;
    const value = text;
    saveState = "saving";
    try {
      await api.saveNotes(meetingId, value);
      saved = value;
      saveState = "saved";
    } catch (e) {
      saveState = "error";
      console.warn((e as AppError).message);
    }
  }

  function oninput() {
    clearTimeout(timer);
    timer = setTimeout(save, 800);
  }

  onMount(async () => {
    text = saved = await api.getNotes(meetingId).catch(() => "");
    loaded = true;
    if (autofocus) el?.focus();
  });
  onDestroy(() => {
    // Simpan sisa ketikan saat panel/jendela ditutup.
    if (loaded && text !== saved) api.saveNotes(meetingId, text).catch(() => {});
  });
</script>

<div class="flex flex-col gap-1.5">
  <textarea
    bind:this={el}
    bind:value={text}
    {oninput}
    onblur={save}
    {rows}
    maxlength={MAX}
    disabled={!loaded}
    placeholder={t.placeholder}
    aria-label={t.title}
    class={[
      "w-full resize-y rounded-[var(--radius-ctl)] border px-3 py-2 leading-relaxed outline-none",
      dark
        ? "border-white/15 bg-white/8 text-white placeholder:text-white/40 focus:border-white/40"
        : "field",
    ]}
  ></textarea>
  <span class={["text-xs", dark ? "text-white/55" : "text-ink-faint"]} role="status">
    {saveState === "saving" ? t.saving : saveState === "saved" ? t.saved : saveState === "error" ? t.error : t.hint}
  </span>
</div>
