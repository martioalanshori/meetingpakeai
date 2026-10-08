<script lang="ts">
  import { dismissToast, toasts } from "$lib/toast.svelte";
  import { id } from "$lib/i18n/id";
</script>

<div class="pointer-events-none fixed right-5 bottom-5 z-50 flex flex-col items-end gap-2" aria-live="polite">
  {#each toasts as t (t.id)}
    <div
      role="status"
      class="pointer-events-auto flex max-w-sm items-start gap-3 rounded-lg bg-ink py-2.5 pr-2 pl-3.5 text-sm text-white shadow-[0_12px_32px_-12px_rgb(30_36_51/0.5)]"
    >
      <span
        class={["mt-1.5 h-2 w-2 shrink-0 rounded-full", t.kind === "error" ? "bg-[#ff8a7a]" : t.kind === "success" ? "bg-[#7fd1b9]" : "bg-white/50"]}
        aria-hidden="true"
      ></span>
      <span class="flex-1 leading-snug">{t.text}</span>
      <button
        type="button"
        class="rounded px-1.5 text-white/60 hover:text-white"
        aria-label={id.common.close}
        onclick={() => dismissToast(t.id)}>✕</button
      >
    </div>
  {/each}
</div>
