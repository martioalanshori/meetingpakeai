<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import ClipboardKeyHint from "$lib/components/ClipboardKeyHint.svelte";
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

<div class="flex flex-col gap-4">
  <div class="flex flex-wrap items-baseline gap-x-4 gap-y-1">
    <h2 class="text-lg font-bold">{t.heading}</h2>
    <span class={["flex items-center gap-1.5 text-sm", apiKeySet ? "text-ok" : "text-ink-soft"]}>
      <span class={["h-2 w-2 rounded-full", apiKeySet ? "bg-ok" : "bg-ink-faint"]} aria-hidden="true"></span>
      {apiKeySet ? t.saved : t.notSaved}
    </span>
  </div>

  {#if editing}
    {#if keyInput.trim() === ""}
      <ClipboardKeyHint onuse={(k) => (keyInput = k)} />
    {/if}
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
          class="field font-mono"
          placeholder={t.inputPlaceholder}
          bind:value={keyInput}
        />
      </label>
      <button
        type="submit"
        class="btn btn-ink"
        disabled={busy || keyInput.trim() === ""}
      >
        {busy ? t.testing : t.testAndSave}
      </button>
      {#if apiKeySet}
        <button
          type="button"
          class="btn btn-quiet"
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
        class="btn btn-line"
        onclick={() => (editing = true)}
      >
        {t.change}
      </button>
      <button
        type="button"
        class="btn btn-line"
        disabled={busy}
        onclick={testSaved}
      >
        {busy ? t.testing : t.test}
      </button>
      <button
        type="button"
        class="btn btn-danger"
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
        "text-ok": message.kind === "ok",
        "text-warn": message.kind === "warn",
        "text-bad": message.kind === "error",
      }}
    >
      {message.text}
    </p>
  {/if}
</div>
