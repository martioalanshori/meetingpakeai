<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { id } from "$lib/i18n/id";
  import { rec, startRecording } from "$lib/recording.svelte";
  import { showToast } from "$lib/toast.svelte";

  let message = $state("");
  let confirmed = $state(false);
  let dialog: HTMLDialogElement;

  onMount(async () => {
    dialog.showModal();
    try {
      message = (await api.getSettings()).consentMessage;
    } catch {
      message = "";
    }
  });

  function close() {
    rec.consentOpen = false;
  }

  async function copy() {
    try {
      await navigator.clipboard.writeText(message);
      showToast(id.toast.copied, "success", 2500);
    } catch {
      showToast(id.errors.INTERNAL, "error");
    }
  }
</script>

<dialog
  bind:this={dialog}
  class="m-auto w-full max-w-lg rounded-xl p-0 shadow-2xl backdrop:bg-black/40"
  aria-labelledby="consent-title"
  oncancel={(e) => {
    e.preventDefault();
    close();
  }}
>
  <div class="flex flex-col gap-4 p-6">
    <h2 id="consent-title" class="text-lg font-semibold">{id.consent.title}</h2>
    <p class="text-gray-700">{id.consent.body}</p>

    <label class="flex flex-col gap-1 text-sm text-gray-600">
      {id.consent.messageLabel}
      <textarea readonly rows="4" class="resize-none rounded-lg border border-gray-300 bg-gray-50 p-3 text-gray-900">{message}</textarea>
    </label>
    <button
      type="button"
      class="self-start rounded-lg border border-gray-300 px-4 py-2 text-sm hover:bg-gray-50"
      onclick={copy}
    >
      {id.consent.copy}
    </button>

    <label class="flex items-center gap-2">
      <input type="checkbox" class="h-4 w-4" bind:checked={confirmed} />
      <span>{id.consent.checkbox}</span>
    </label>

    <div class="flex justify-end gap-2">
      <button type="button" class="rounded-lg px-4 py-2 hover:bg-gray-100" onclick={close}>
        {id.common.cancel}
      </button>
      <button
        type="button"
        class="rounded-lg bg-red-600 px-4 py-2 font-medium text-white hover:bg-red-700 disabled:cursor-not-allowed disabled:opacity-50"
        disabled={!confirmed || rec.busy}
        onclick={startRecording}
      >
        {rec.busy ? id.home.starting : id.consent.start}
      </button>
    </div>
  </div>
</dialog>
