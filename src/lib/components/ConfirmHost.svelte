<script lang="ts">
  import { tick } from "svelte";
  import { confirmState, settleConfirm } from "$lib/confirm.svelte";
  import { id } from "$lib/i18n/id";

  let dialog = $state<HTMLDialogElement | null>(null);
  let confirmBtn = $state<HTMLButtonElement | null>(null);

  $effect(() => {
    if (confirmState.opts && dialog && !dialog.open) {
      dialog.showModal();
      // Aksi merusak: fokus awal di Batal agar Enter tidak langsung menghapus.
      if (!confirmState.opts.danger) tick().then(() => confirmBtn?.focus());
    } else if (!confirmState.opts && dialog?.open) {
      dialog.close();
    }
  });
</script>

<dialog
  bind:this={dialog}
  class="sheet-dialog"
  aria-labelledby="confirm-title"
  onclose={() => {
    if (confirmState.opts) settleConfirm(false);
  }}
>
  {#if confirmState.opts}
    {@const o = confirmState.opts}
    <div class="flex flex-col gap-3 p-6">
      <h2 id="confirm-title" class="section-title">{o.title}</h2>
      {#if o.message}<p class="leading-relaxed text-ink-soft">{o.message}</p>{/if}
      <div class="mt-2 flex justify-end gap-2">
        <button type="button" class="btn btn-quiet" onclick={() => settleConfirm(false)}>
          {o.cancelText ?? id.common.cancel}
        </button>
        <button
          type="button"
          class={["btn", o.danger ? "btn-danger-solid" : "btn-ink"]}
          bind:this={confirmBtn}
          onclick={() => settleConfirm(true)}
        >
          {o.confirmText}
        </button>
      </div>
    </div>
  {/if}
</dialog>
