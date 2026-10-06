<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { api } from "$lib/api";
  import ApiKeySection from "$lib/components/ApiKeySection.svelte";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, Settings } from "$lib/types";

  const t = id.settings;

  let form = $state<Settings | null>(null);
  let saving = $state(false);
  let version = $state("");

  onMount(async () => {
    try {
      form = await api.getSettings();
      version = await getVersion();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  });

  async function save(e: SubmitEvent) {
    e.preventDefault();
    if (!form) return;
    saving = true;
    try {
      form = await api.updateSettings({ ...form });
      showToast(id.toast.settingsSaved, "success");
    } catch (err) {
      showToast((err as AppError).message, "error");
    } finally {
      saving = false;
    }
  }
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-4 p-6">
  <a href="/" class="text-sm text-indigo-700 hover:underline">{id.common.back}</a>
  <h1 class="text-xl font-semibold">{t.title}</h1>

  <ApiKeySection />

  {#if form}
    <form class="flex flex-col gap-5 rounded-xl border border-gray-200 bg-white p-5" onsubmit={save}>
      <h2 class="font-semibold">{t.general}</h2>

      <label class="flex flex-col gap-1">
        <span class="text-sm font-medium">{t.displayName}</span>
        <input class="max-w-sm rounded-lg border border-gray-300 px-3 py-2" maxlength="50" bind:value={form.userDisplayName} />
        <span class="text-sm text-gray-500">{t.displayNameHint}</span>
      </label>

      <fieldset class="flex flex-col gap-1">
        <legend class="mb-1 text-sm font-medium">{t.language}</legend>
        <label class="flex items-center gap-2">
          <input type="radio" name="lang" value="id" bind:group={form.sttLanguage} />
          {t.langId}
        </label>
        <label class="flex items-center gap-2">
          <input type="radio" name="lang" value="auto" bind:group={form.sttLanguage} />
          {t.langAuto}
        </label>
        <span class="text-sm text-gray-500">{t.langAutoNote}</span>
      </fieldset>

      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.deleteAudioAfterTranscript} />
        <span class="flex flex-col">
          <span>{t.deleteAudio}</span>
          <span class="text-sm text-gray-500">{t.deleteAudioNote}</span>
        </span>
      </label>

      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.minimizeToTray} />
        <span class="flex flex-col">
          <span>{t.minimizeToTray}</span>
          <span class="text-sm text-gray-500">{t.minimizeToTrayNote}</span>
        </span>
      </label>

      <label class="flex flex-col gap-1">
        <span class="text-sm font-medium">{t.consentMessage}</span>
        <textarea class="rounded-lg border border-gray-300 p-3" rows="4" bind:value={form.consentMessage}></textarea>
      </label>

      <button
        type="submit"
        class="self-start rounded-lg bg-indigo-600 px-5 py-2 font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
        disabled={saving}
      >
        {t.save}
      </button>
    </form>
  {/if}

  <section class="flex flex-wrap items-center gap-3 rounded-xl border border-gray-200 bg-white p-5">
    <h2 class="font-semibold">{t.about}</h2>
    <span class="text-gray-600">{t.version(version)}</span>
    <button
      type="button"
      class="ml-auto rounded-lg border border-gray-300 px-4 py-2 hover:bg-gray-50"
      onclick={() => api.openLogFolder().catch((e: AppError) => showToast(e.message, "error"))}
    >
      {t.openLogs}
    </button>
  </section>
</main>
