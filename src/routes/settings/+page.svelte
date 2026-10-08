<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { api } from "$lib/api";
  import AiProviderSection from "$lib/components/AiProviderSection.svelte";
  import ShortcutInput from "$lib/components/ShortcutInput.svelte";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import { setWindowTitle } from "$lib/viewport.svelte";
  import type { AppError, Settings, UpdateInfo } from "$lib/types";

  const t = id.settings;

  let form = $state<Settings | null>(null);
  let saving = $state(false);
  let version = $state("");


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
    setWindowTitle(t.title);
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

<main class="mx-auto flex w-full max-w-3xl flex-col px-6 pt-7 pb-16 xl:px-10">
  <h1 class="mb-2 text-2xl font-bold tracking-[-0.02em]">{t.title}</h1>

  {#if form}
    <form onsubmit={save}>
      <section class="flex flex-col gap-5 border-b border-line py-7">
        <h2 class="text-lg font-bold">{t.sectionRecording}</h2>

        <fieldset class="flex flex-col gap-1.5">
          <legend class="mb-1 font-medium">{t.language}</legend>
          <label class="flex items-center gap-2">
            <input type="radio" name="lang" value="id" bind:group={form.sttLanguage} />
            {t.langId}
          </label>
          <label class="flex items-center gap-2">
            <input type="radio" name="lang" value="auto" bind:group={form.sttLanguage} />
            {t.langAuto}
          </label>
          <span class="text-sm text-ink-soft">{t.langAutoNote}</span>
        </fieldset>

        <fieldset class="flex flex-col gap-1.5">
          <legend class="mb-1 font-medium">{t.retention}</legend>
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
          <span class="text-sm text-ink-soft">{t.retentionNote}</span>
        </fieldset>

      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.meetingDetection} />
        <span class="flex flex-col gap-0.5">
          <span class="font-medium">{t.meetingDetection}</span>
          <span class="text-sm text-ink-soft">{t.meetingDetectionNote}</span>
        </span>
      </label>

        <div class="flex flex-col gap-1.5">
          <span class="font-medium">{t.shortcut}</span>
          <ShortcutInput bind:value={form.globalShortcut} />
          <span class="max-w-prose text-sm text-ink-soft">{t.shortcutHint}</span>
        </div>
      </section>

      <section class="flex flex-col gap-5 border-b border-line py-7">
        <h2 class="text-lg font-bold">{t.sectionApp}</h2>

        <label class="flex flex-col gap-1.5">
          <span class="font-medium">{t.displayName}</span>
          <input class="field max-w-sm" maxlength="50" bind:value={form.userDisplayName} />
          <span class="text-sm text-ink-soft">{t.displayNameHint}</span>
        </label>

      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.autostart} />
        <span class="flex flex-col gap-0.5">
          <span class="font-medium">{t.autostart}</span>
          <span class="text-sm text-ink-soft">{t.autostartNote}</span>
        </span>
      </label>
      <label class="flex items-start gap-3">
        <input type="checkbox" class="mt-1 h-4 w-4" bind:checked={form.minimizeToTray} />
        <span class="flex flex-col gap-0.5">
          <span class="font-medium">{t.minimizeToTray}</span>
          <span class="text-sm text-ink-soft">{t.minimizeToTrayNote}</span>
        </span>
      </label>

        <button type="submit" class="btn btn-ink self-start" disabled={saving}>{t.save}</button>
      </section>
    </form>
  {/if}

  <section class="border-b border-line py-7">
    <AiProviderSection />
  </section>

  <section class="flex flex-col gap-4 py-7">
    <h2 class="text-lg font-bold">{t.sectionHelp}</h2>
    <div class="flex flex-col items-start gap-1.5">
      <button type="button" class="btn btn-line" onclick={saveReport}>{t.report}</button>
      <span class="max-w-prose text-sm text-ink-soft">{t.reportNote}</span>
    </div>
    <div class="flex flex-wrap items-center gap-2">
      {#if version}<span class="tabular mr-2 text-ink-soft">{t.version(version)}</span>{/if}
      {#if update}
        <button type="button" class="btn btn-ink" disabled={updateBusy} onclick={installUpdate}>{t.installUpdate}</button>
      {:else}
        <button type="button" class="btn btn-line" disabled={updateBusy} onclick={checkUpdate}>
          {updateBusy ? t.checkingUpdate : t.checkUpdate}
        </button>
      {/if}
      <button
        type="button"
        class="btn btn-quiet"
        onclick={() => api.openLogFolder().catch((e: AppError) => showToast(e.message, "error"))}
      >
        {t.openLogs}
      </button>
    </div>
    {#if update}
      <p class="text-sm">{t.updateAvailable(update.version)}</p>
      {#if update.notes}<p class="max-w-prose text-sm whitespace-pre-line text-ink-soft">{update.notes}</p>{/if}
    {/if}
    {#if updateMsg}
      <p class="text-sm text-ink-soft" role="status">{updateMsg}</p>
    {/if}
  </section>
</main>
