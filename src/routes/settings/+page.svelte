<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { api } from "$lib/api";
  import ApiKeySection from "$lib/components/ApiKeySection.svelte";
  import ShortcutInput from "$lib/components/ShortcutInput.svelte";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import type { AppError, QuotaToday, Settings, UpdateInfo } from "$lib/types";

  const t = id.settings;

  let form = $state<Settings | null>(null);
  let saving = $state(false);
  let version = $state("");
  let quota = $state<QuotaToday | null>(null);

  // ±1 jam meeting ≈ 1 jam audio terkirim (dua channel, hanya bagian bersuara).
  const quotaHours = $derived(
    quota ? Math.max(0, Math.floor((quota.sttAudioSecLimit - quota.sttAudioSecUsed) / 3600)) : 0,
  );

  async function saveReport() {
    try {
      if (await api.saveProblemReport()) showToast(t.reportSaved, "success");
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }
  let update = $state<UpdateInfo | null>(null);
  let updateMsg = $state<string | null>(null);
  let updateBusy = $state(false);

  async function checkUpdate() {
    updateBusy = true;
    updateMsg = null;
    try {
      update = await api.checkUpdate();
      if (!update) updateMsg = t.upToDate;
    } catch (e) {
      updateMsg = (e as AppError).message;
    } finally {
      updateBusy = false;
    }
  }

  async function installUpdate() {
    updateBusy = true;
    updateMsg = t.installingUpdate;
    try {
      await api.installUpdate();
    } catch (e) {
      updateMsg = (e as AppError).message;
      updateBusy = false;
    }
  }

  onMount(async () => {
    try {
      form = await api.getSettings();
      version = await getVersion();
      quota = await api.getQuotaToday().catch(() => null);
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

      <fieldset class="flex flex-col gap-1">
        <legend class="mb-1 text-sm font-medium">{t.retention}</legend>
        <label class="flex items-center gap-2">
          <input type="radio" name="retention" value="after_transcript" bind:group={form.audioRetention} />
          {t.retentionAfter}
        </label>
        <label class="flex items-center gap-2">
          <input type="radio" name="retention" value="days7" bind:group={form.audioRetention} />
          {t.retentionDays7}
        </label>
        <label class="flex items-center gap-2">
          <input type="radio" name="retention" value="forever" bind:group={form.audioRetention} />
          {t.retentionForever}
        </label>
        <span class="text-sm text-gray-500">{t.retentionNote}</span>
      </fieldset>

      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.minimizeToTray} />
        <span class="flex flex-col">
          <span>{t.minimizeToTray}</span>
          <span class="text-sm text-gray-500">{t.minimizeToTrayNote}</span>
        </span>
      </label>

      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.autostart} />
        <span class="flex flex-col">
          <span>{t.autostart}</span>
          <span class="text-sm text-gray-500">{t.autostartNote}</span>
        </span>
      </label>

      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.meetingDetection} />
        <span class="flex flex-col">
          <span>{t.meetingDetection}</span>
          <span class="text-sm text-gray-500">{t.meetingDetectionNote}</span>
        </span>
      </label>

      <div class="flex flex-col gap-1">
        <span class="text-sm font-medium">{t.shortcut}</span>
        <ShortcutInput bind:value={form.globalShortcut} />
        <span class="text-sm text-gray-500">{t.shortcutHint}</span>
      </div>

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

  {#if quota}
    <section class="flex flex-col gap-1 rounded-xl border border-gray-200 bg-white p-5">
      <h2 class="mb-1 font-semibold">{t.quota}</h2>
      <span class="text-gray-700">
        {t.quotaAudio(Math.round(quota.sttAudioSecUsed / 60), Math.round(quota.sttAudioSecLimit / 60))}
      </span>
      <div class="h-2 overflow-hidden rounded bg-gray-200">
        <div
          class="h-full bg-indigo-600"
          style:width={`${Math.min(100, (quota.sttAudioSecUsed / Math.max(1, quota.sttAudioSecLimit)) * 100)}%`}
        ></div>
      </div>
      <span class="text-gray-700">{t.quotaTokens(quota.llmTokensUsed, Math.round(quota.llmTokensLimit))}</span>
      <span class="text-sm text-gray-700">{t.quotaEstimate(quotaHours)}</span>
      <span class="text-sm text-gray-500">{t.quotaNote}</span>
    </section>
  {/if}

  <section class="flex flex-col gap-2 rounded-xl border border-gray-200 bg-white p-5">
    <button type="button" class="self-start rounded-lg border border-gray-300 px-4 py-2 hover:bg-gray-50" onclick={saveReport}>
      {t.report}
    </button>
    <span class="text-sm text-gray-500">{t.reportNote}</span>
  </section>

  <section class="flex flex-wrap items-center gap-3 rounded-xl border border-gray-200 bg-white p-5">
    <h2 class="font-semibold">{t.about}</h2>
    <span class="text-gray-600">{t.version(version)}</span>
    <div class="ml-auto flex flex-wrap gap-2">
      {#if update}
        <button
          type="button"
          class="rounded-lg bg-indigo-600 px-4 py-2 font-medium text-white hover:bg-indigo-700 disabled:opacity-50"
          disabled={updateBusy}
          onclick={installUpdate}
        >
          {t.installUpdate}
        </button>
      {:else}
        <button
          type="button"
          class="rounded-lg border border-gray-300 px-4 py-2 hover:bg-gray-50 disabled:opacity-50"
          disabled={updateBusy}
          onclick={checkUpdate}
        >
          {updateBusy ? t.checkingUpdate : t.checkUpdate}
        </button>
      {/if}
      <button
        type="button"
        class="rounded-lg border border-gray-300 px-4 py-2 hover:bg-gray-50"
        onclick={() => api.openLogFolder().catch((e: AppError) => showToast(e.message, "error"))}
      >
        {t.openLogs}
      </button>
    </div>
    {#if update}
      <p class="w-full text-sm text-indigo-800">{t.updateAvailable(update.version)}</p>
      {#if update.notes}<p class="w-full text-sm whitespace-pre-line text-gray-600">{update.notes}</p>{/if}
    {/if}
    {#if updateMsg}
      <p class="w-full text-sm text-gray-600" role="status">{updateMsg}</p>
    {/if}
  </section>
</main>
