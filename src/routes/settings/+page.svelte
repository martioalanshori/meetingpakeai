<script lang="ts">
  import { onMount } from "svelte";
  import { getVersion } from "@tauri-apps/api/app";
  import { api } from "$lib/api";
  import { page } from "$app/state";
  import AiProviderSection from "$lib/components/AiProviderSection.svelte";
  import ChoiceCards from "$lib/components/ChoiceCards.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Switch from "$lib/components/Switch.svelte";
  import ShortcutInput from "$lib/components/ShortcutInput.svelte";
  import { id } from "$lib/i18n/id";
  import { showToast } from "$lib/toast.svelte";
  import { confirmDialog } from "$lib/confirm.svelte";
  import { setWindowTitle } from "$lib/viewport.svelte";
  import type { AppError, Settings, UpdateInfo } from "$lib/types";

  const t = id.settings;

  let form = $state<Settings | null>(null);
  let loadError = $state<string | null>(null);
  let version = $state("");
  let storage = $state<{ recordingsBytes: number; clearableMeetings: number } | null>(null);

  const formatBytes = (b: number) =>
    b >= 1024 ** 3 ? `${(b / 1024 ** 3).toFixed(1).replace(".", ",")} GB` : `${Math.max(0, Math.round(b / 1024 ** 2))} MB`;

  async function clearAudio() {
    if (!storage || storage.clearableMeetings === 0) return;
    const ok = await confirmDialog({
      title: t.storageClearTitle,
      message: t.storageClearMessage(storage.clearableMeetings),
      confirmText: t.storageClear,
      danger: true,
    });
    if (!ok) return;
    try {
      const n = await api.clearOldAudio();
      showToast(t.storageCleared(n), "success");
      storage = await api.getStorageUsage();
    } catch (e) {
      showToast((e as AppError).message, "error");
    }
  }


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

  type Section = "recording" | "ai" | "app" | "help";
  const sections: { key: Section; label: string }[] = [
    { key: "recording", label: t.sectionRecording },
    { key: "ai", label: t.sectionAi },
    { key: "app", label: t.sectionApp },
    { key: "help", label: t.sectionHelp },
  ];
  // `?tab=ai` dari banner "antrean dijeda" langsung membuka Layanan AI.
  const initial = page.url.searchParams.get("tab");
  let section = $state<Section>(sections.some((x) => x.key === initial) ? (initial as Section) : "recording");

  let nameDraft = $state("");
  let glossaryDraft = $state("");
  const GLOSSARY_MAX = 800;

  onMount(async () => {
    setWindowTitle(t.title);
    try {
      form = await api.getSettings();
      nameDraft = form.userDisplayName;
      glossaryDraft = form.sttGlossary;
    } catch (e) {
      loadError = (e as AppError).message;
    }
    version = await getVersion().catch(() => "");
    storage = await api.getStorageUsage().catch(() => null);
  });

  /** Simpan satu perubahan langsung (tanpa tombol Simpan); gagal → nilai dikembalikan. */
  async function patch(change: Partial<Settings>) {
    if (!form) return;
    const before = $state.snapshot(form);
    form = { ...form, ...change };
    try {
      form = await api.updateSettings(change);
      showToast(id.toast.settingsSaved, "success", 2000);
    } catch (e) {
      form = before;
      showToast((e as AppError).message, "error");
    }
  }

  async function saveGlossary() {
    if (!form || glossaryDraft === form.sttGlossary) return;
    await patch({ sttGlossary: glossaryDraft });
    glossaryDraft = form?.sttGlossary ?? glossaryDraft;
  }

  async function saveName() {
    if (!form) return;
    const v = nameDraft.trim();
    if (v === form.userDisplayName) return;
    await patch({ userDisplayName: v });
    nameDraft = form?.userDisplayName ?? v;
  }
</script>

{#snippet toggleRow(label: string, hint: string, checked: boolean, onchange: (v: boolean) => void)}
  <div class="flex items-start justify-between gap-6 py-4">
    <div class="flex flex-col gap-0.5">
      <span class="label">{label}</span>
      <span class="hint max-w-prose">{hint}</span>
    </div>
    <Switch {checked} {label} {onchange} />
  </div>
{/snippet}

<main class="mx-auto flex w-full max-w-3xl flex-col px-6 pt-7 pb-16 xl:px-10">
  <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.title}</h1>

  <nav class="mt-5 flex gap-1 overflow-x-auto border-b border-line" aria-label={t.title}>
    {#each sections as sec (sec.key)}
      <button
        type="button"
        class={[
          "-mb-px border-b-2 px-3 pt-1 pb-2.5 text-base whitespace-nowrap",
          section === sec.key ? "border-ink font-semibold text-ink" : "border-transparent text-ink-soft hover:text-ink",
        ]}
        aria-current={section === sec.key ? "page" : undefined}
        onclick={() => (section = sec.key)}
      >
        {sec.label}
      </button>
    {/each}
  </nav>

  {#if loadError}
    <div role="alert" class="mt-6 flex flex-wrap items-center gap-3 rounded-xl bg-bad-wash px-4 py-3 text-bad">
      <span class="flex-1 text-sm font-medium">{loadError}</span>
      <button type="button" class="btn btn-ink btn-sm" onclick={() => location.reload()}>{id.common.retry}</button>
    </div>
  {:else if !form && (section === "recording" || section === "app")}
    <div class="mt-6 flex flex-col gap-4 motion-safe:animate-pulse" aria-hidden="true">
      <div class="h-5 w-1/3 rounded bg-line-soft"></div>
      <div class="h-16 rounded-lg bg-line-soft"></div>
      <div class="h-16 rounded-lg bg-line-soft"></div>
    </div>
  {:else if section === "recording" && form}
    <section class="flex flex-col divide-y divide-line-soft">
      <div class="flex flex-col gap-2.5 py-5">
        <span class="label">{t.language}</span>
        <ChoiceCards
          name="lang"
          value={form.sttLanguage}
          options={[
            { value: "id", label: t.langId, hint: t.langIdHint },
            { value: "auto", label: t.langAuto, hint: t.langAutoNote },
          ]}
          onchange={(v) => patch({ sttLanguage: v })}
        />
      </div>
      <div class="flex flex-col gap-2.5 py-5">
        <span class="label">{t.notesLanguage}</span>
        <ChoiceCards
          name="notes-lang"
          value={form.notesLanguage}
          options={[
            { value: "id", label: t.notesLangId, hint: t.notesLangIdHint },
            { value: "en", label: t.notesLangEn, hint: t.notesLangEnHint },
            { value: "auto", label: t.notesLangAuto, hint: t.notesLangAutoHint },
          ]}
          onchange={(v) => patch({ notesLanguage: v })}
        />
      </div>
      <div class="flex flex-col gap-2.5 py-5">
        <span class="label">{t.retention}</span>
        <ChoiceCards
          name="retention"
          value={form.audioRetention}
          options={[
            { value: "after_transcript", label: t.retentionAfter, hint: t.retentionAfterHint },
            { value: "days7", label: t.retentionDays7, hint: t.retentionDays7Hint },
            { value: "forever", label: t.retentionForever, hint: t.retentionForeverHint },
          ]}
          onchange={(v) => patch({ audioRetention: v })}
        />
      </div>
      <div class="flex flex-col gap-2 py-5">
        <div class="flex items-baseline justify-between gap-4">
          <label class="label" for="glossary">{t.glossary}</label>
          <span class="hint tabular">{t.glossaryCount(glossaryDraft.length, GLOSSARY_MAX)}</span>
        </div>
        <textarea
          id="glossary"
          class="field min-h-28 resize-y leading-relaxed"
          rows="5"
          maxlength={GLOSSARY_MAX}
          placeholder={t.glossaryPlaceholder}
          bind:value={glossaryDraft}
          onblur={saveGlossary}
        ></textarea>
        <span class="hint max-w-prose">{t.glossaryHint}</span>
      </div>
      {@render toggleRow(t.meetingDetection, t.meetingDetectionNote, form.meetingDetection, (v) =>
        patch({ meetingDetection: v }),
      )}
      {#if form.meetingDetection}
        {@render toggleRow(t.autoRecord, t.autoRecordNote, form.autoRecord, (v) => patch({ autoRecord: v }))}
      {/if}
      {#if !form.meetingDetection || !form.autostart}
        <!-- Status deteksi (feedback3 A0.5): jelaskan kapan meeting bisa terlewat. -->
        <div class="flex flex-wrap items-center gap-3 py-4">
          <Icon name="alert-circle" size={18} class="shrink-0 text-warn" />
          <span class="hint flex-1 text-warn">{form.meetingDetection ? t.detectionNoAutostart : t.detectionOff}</span>
          {#if form.meetingDetection}
            <button type="button" class="btn btn-line btn-sm" onclick={() => patch({ autostart: true })}>{t.enableAutostart}</button>
          {/if}
        </div>
      {/if}
      <div class="flex flex-col gap-2 py-5">
        <label class="label" for="shortcut-input">{t.shortcut}</label>
        <ShortcutInput inputId="shortcut-input" value={form.globalShortcut} onchange={(v) => patch({ globalShortcut: v })} />
        <span class="hint max-w-prose">{t.shortcutHint}</span>
      </div>
      <div class="flex flex-col gap-2 py-5">
        <label class="label" for="bookmark-shortcut-input">{t.bookmarkShortcut}</label>
        <ShortcutInput
          inputId="bookmark-shortcut-input"
          value={form.bookmarkShortcut}
          onchange={(v) => patch({ bookmarkShortcut: v })}
        />
        <span class="hint max-w-prose">{t.bookmarkShortcutHint}</span>
      </div>
      {#if storage}
        <div class="flex flex-wrap items-center justify-between gap-4 py-5">
          <div class="flex flex-col gap-0.5">
            <span class="label">{t.storage}</span>
            <span class="hint">{t.storageUsed(formatBytes(storage.recordingsBytes))}</span>
          </div>
          <button type="button" class="btn btn-line btn-sm" disabled={storage.clearableMeetings === 0} onclick={clearAudio}>
            <Icon name="trash" size={14} />{t.storageClear}
          </button>
        </div>
      {/if}
    </section>
  {:else if section === "ai"}
    <section class="py-6">
      <AiProviderSection />
    </section>
  {:else if section === "app" && form}
    <section class="flex flex-col divide-y divide-line-soft">
      <div class="flex flex-col gap-2 py-5">
        <label class="label" for="display-name">{t.displayName}</label>
        <input
          id="display-name"
          class="field max-w-sm"
          maxlength="50"
          bind:value={nameDraft}
          onblur={saveName}
          onkeydown={(e) => {
            if (e.key === "Enter") (e.currentTarget as HTMLInputElement).blur();
          }}
        />
        <span class="hint max-w-prose">{t.displayNameHint}</span>
      </div>
      {@render toggleRow(t.autostart, t.autostartNote, form.autostart, (v) => patch({ autostart: v }))}
      {@render toggleRow(t.minimizeToTray, t.minimizeToTrayNote, form.minimizeToTray, (v) => patch({ minimizeToTray: v }))}
    </section>
  {:else if section === "help"}
    <section class="flex flex-col divide-y divide-line-soft">
      <div class="flex flex-wrap items-center justify-between gap-4 py-5">
        <div class="flex flex-col gap-0.5">
          <span class="label">{t.reportTitle}</span>
          <span class="hint max-w-prose">{t.reportNote}</span>
        </div>
        <button type="button" class="btn btn-line btn-sm" onclick={saveReport}>{t.report}</button>
      </div>
      <div class="flex flex-wrap items-center justify-between gap-4 py-5">
        <div class="flex flex-col gap-0.5">
          <span class="label">{t.logsTitle}</span>
          <span class="hint">{t.logsHint}</span>
        </div>
        <button
          type="button"
          class="btn btn-line btn-sm"
          onclick={() => api.openLogFolder().catch((e: AppError) => showToast(e.message, "error"))}
        >
          {t.openLogs}
        </button>
      </div>
      <div class="flex flex-col gap-2 py-5">
        <div class="flex flex-wrap items-center justify-between gap-4">
          <div class="flex flex-col gap-0.5">
            <span class="label">{t.updatesTitle}</span>
            {#if version}<span class="hint tabular">{t.version(version)}</span>{/if}
          </div>
          {#if update}
            <button type="button" class="btn btn-ink btn-sm" disabled={updateBusy} onclick={installUpdate}>{t.installUpdate}</button>
          {:else}
            <button type="button" class="btn btn-line btn-sm" disabled={updateBusy} onclick={checkUpdate}>
              {updateBusy ? t.checkingUpdate : t.checkUpdate}
            </button>
          {/if}
        </div>
        {#if update}
          <p class="text-sm">{t.updateAvailable(update.version)}</p>
          {#if update.notes}<p class="hint max-w-prose whitespace-pre-line">{update.notes}</p>{/if}
        {/if}
        {#if updateMsg}<p class="hint" role="status">{updateMsg}</p>{/if}
      </div>
    </section>
  {/if}
</main>
