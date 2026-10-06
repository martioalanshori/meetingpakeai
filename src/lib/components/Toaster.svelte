<script lang="ts">
  import { dismissToast, toasts } from "$lib/toast.svelte";
  import { id } from "$lib/i18n/id";
</script>

<div class="pointer-events-none fixed inset-x-0 bottom-4 z-50 flex flex-col items-center gap-2 px-4" aria-live="polite">
  {#each toasts as t (t.id)}
    <div
      role="status"
      class={[
        "pointer-events-auto flex max-w-lg items-center gap-3 rounded-lg px-4 py-2.5 text-sm text-white shadow-lg",
        t.kind === "error" ? "bg-red-700" : t.kind === "success" ? "bg-emerald-700" : "bg-gray-800",
      ]}
    >
      <span>{t.text}</span>
      <button
        type="button"
        class="rounded px-1 text-white/80 hover:text-white"
        aria-label={id.common.close}
        onclick={() => dismissToast(t.id)}>✕</button
      >
    </div>
  {/each}
</div>
