<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import { id } from "$lib/i18n/id";
  import type { AppError } from "$lib/types";

  const t = id.settings.apiKey;

  let apiKeySet = $state(false);
  let editing = $state(false);
  let keyInput = $state("");
  let busy = $state(false);
  let message = $state<{ kind: "ok" | "warn" | "error"; text: string } | null>(null);

  onMount(refresh);

  async function refresh() {
    try {
      apiKeySet = (await api.getOnboardingStatus()).apiKeySet;
      editing = !apiKeySet;
    } catch (e) {
      message = { kind: "error", text: (e as AppError).message };
    }
  }

  function showResult(missingModels: string[], okText: string) {
    message =
      missingModels.length > 0
        ? { kind: "warn", text: t.missingModels(missingModels) }
        : { kind: "ok", text: okText };
  }

  /** Uji key yang diketik; simpan hanya jika lolos. */
  async function testAndSave() {
    busy = true;
    message = null;
    try {
      const res = await api.testApiKey(keyInput);
      await api.saveApiKey(keyInput);
      keyInput = "";
      apiKeySet = true;
      editing = false;
      showResult(res.missingModels, t.saveOk);
    } catch (e) {
      message = { kind: "error", text: (e as AppError).message };
    } finally {
      busy = false;
    }
  }

  async function testSaved() {
    busy = true;
    message = null;
    try {
      const res = await api.testApiKey();
      showResult(res.missingModels, t.ok);
    } catch (e) {
      message = { kind: "error", text: (e as AppError).message };
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (!confirm(t.removeConfirm)) return;
    try {
      await api.deleteApiKey();
      apiKeySet = false;
      editing = true;
      message = { kind: "ok", text: t.removed };
    } catch (e) {
      message = { kind: "error", text: (e as AppError).message };
    }
  }
</script>

<section class="flex flex-col gap-3 rounded-xl border border-gray-200 bg-white p-5">
  <div class="flex items-center justify-between">
    <h2 class="font-semibold">{t.heading}</h2>
    <span class={apiKeySet ? "text-sm text-emerald-700" : "text-sm text-gray-500"}>
      {apiKeySet ? t.saved : t.notSaved}
    </span>
  </div>

  {#if editing}
    <form
      class="flex flex-wrap items-end gap-2"
      onsubmit={(e) => {
        e.preventDefault();
        testAndSave();
      }}
    >
      <label class="flex min-w-64 flex-1 flex-col gap-1 text-sm">
        {t.inputLabel}
        <input
          type="password"
          autocomplete="off"
          spellcheck="false"
          class="rounded-lg border border-gray-300 px-3 py-2 font-mono"
          placeholder={t.inputPlaceholder}
          bind:value={keyInput}
        />
      </label>
      <button
        type="submit"
        class="rounded-lg bg-indigo-600 px-4 py-2 font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
        disabled={busy || keyInput.trim() === ""}
      >
        {busy ? t.testing : t.testAndSave}
      </button>
      {#if apiKeySet}
        <button
          type="button"
          class="rounded-lg px-4 py-2 text-gray-700 hover:bg-gray-100"
          onclick={() => {
            editing = false;
            keyInput = "";
          }}
        >
          {t.cancel}
        </button>
      {/if}
    </form>
  {:else}
    <div class="flex flex-wrap gap-2">
      <button
        type="button"
        class="rounded-lg border border-gray-300 px-4 py-2 hover:bg-gray-50"
        onclick={() => (editing = true)}
      >
        {t.change}
      </button>
      <button
        type="button"
        class="rounded-lg border border-gray-300 px-4 py-2 hover:bg-gray-50 disabled:opacity-50"
        disabled={busy}
        onclick={testSaved}
      >
        {busy ? t.testing : t.test}
      </button>
      <button
        type="button"
        class="rounded-lg px-4 py-2 text-red-700 hover:bg-red-50"
        onclick={remove}
      >
        {t.remove}
      </button>
    </div>
  {/if}

  {#if message}
    <p
      role="status"
      class={{
        "text-sm": true,
        "text-emerald-700": message.kind === "ok",
        "text-amber-700": message.kind === "warn",
        "text-red-700": message.kind === "error",
      }}
    >
      {message.text}
    </p>
  {/if}
</section>
