<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import AiProviderSection from "$lib/components/AiProviderSection.svelte";
  import Wordmark from "$lib/components/Wordmark.svelte";
  import { id } from "$lib/i18n/id";
  import type { AppError, AudioTestResult, MicPermission } from "$lib/types";

  const t = id.onboarding;

  let step = $state(1);
  let autostart = $state(true);
  let shortcut = $state("");

  // Langkah 2: layanan AI (siap jika semua key yang dibutuhkan tersimpan)
  let keyOk = $state(false);

  // Langkah 3: izin mic & tes
  let permission = $state<MicPermission>("unknown");
  let testing = $state(false);
  let result = $state<AudioTestResult | null>(null);
  let testError = $state<string | null>(null);
  let micDb = $state(-90);
  let sysDb = $state(-90);
  let unlisten: UnlistenFn | null = null;

  onMount(async () => {
    try {
      const s = await api.getOnboardingStatus();
      keyOk = s.apiKeySet;
      permission = s.micPermission;
    } catch {
      /* abaikan */
    }
    shortcut = await api.getSettings().then((s) => s.globalShortcut, () => "");
    unlisten = await events.recordingLevel((l) => {
      micDb = l.micDbfs;
      sysDb = l.systemDbfs;
    });
  });

  onDestroy(() => unlisten?.());

  async function recheckPermission() {
    try {
      permission = await api.checkMicPermission();
    } catch {
      /* abaikan */
    }
  }

  async function runTest() {
    testing = true;
    testError = null;
    result = null;
    try {
      result = await api.runAudioTest();
      await recheckPermission();
    } catch (e) {
      testError = (e as AppError).message;
    } finally {
      testing = false;
    }
  }

  async function finish() {
    try {
      await api.updateSettings({ autostart }).catch(() => {});
      await api.completeOnboarding();
      await goto("/");
    } catch (e) {
      testError = (e as AppError).message;
    }
  }

  function meterWidth(db: number) {
    return `${Math.max(0, Math.min(100, ((db + 60) / 60) * 100))}%`;
  }

  const primary = "btn btn-ink px-5 py-2.5";
  const secondary = "btn btn-line";
</script>

<main class="mx-auto flex min-h-full w-full max-w-xl flex-col justify-center gap-8 px-8 py-10">
  <div class="flex items-center justify-between gap-4">
    <Wordmark />
    <span class="tabular text-sm text-ink-soft">{id.onboarding.stepOf(step, 4)}</span>
  </div>
  <ol class="flex gap-1.5" aria-hidden="true">
    {#each [1, 2, 3, 4] as n (n)}
      <li class={["h-1 flex-1 rounded-full", n <= step ? "bg-ink" : "bg-line"]}></li>
    {/each}
  </ol>

  {#if step === 1}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.welcomeTitle}</h1>
      <p class="text-[1.0625rem] leading-[1.75]">{t.privacy}</p>
      <button type="button" class={[primary, "self-start"]} onclick={() => (step = 2)}>{t.understand}</button>
    </section>
  {:else if step === 2}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.apiKeyTitle}</h1>
      <p class="leading-relaxed">{t.aiIntro}</p>
      <h2 class="font-bold">{t.groqQuick}</h2>
      <ol class="flex flex-col gap-3">
        {#each t.apiKeySteps as s, i (s)}
          <li class="grid grid-cols-[1.75rem_1fr] items-baseline leading-relaxed">
            <span class="tabular font-bold text-ink-faint">{i + 1}</span>{s}
          </li>
        {/each}
      </ol>
      <button type="button" class={[secondary, "self-start"]} onclick={() => openUrl("https://console.groq.com/keys")}>
        {t.openConsole}
      </button>
      <div class="border-t border-line pt-5">
        <AiProviderSection showHeading={false} onchange={(ready) => (keyOk = ready)} />
      </div>
      <button type="button" class={[primary, "self-start"]} disabled={!keyOk} onclick={() => (step = 3)}>
        {t.next}
      </button>
    </section>
  {:else if step === 3}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.micTitle}</h1>
      <div class="flex flex-wrap items-center gap-3">
        <p
          class={[
            permission === "allowed" && "text-ok",
            permission === "denied" && "text-bad",
            permission === "unknown" && "text-ink-soft",
          ]}
        >
          {permission === "allowed" ? t.permAllowed : permission === "denied" ? t.permDenied : t.permUnknown}
        </p>
        {#if permission !== "allowed"}
          <button type="button" class={secondary} onclick={() => api.openMicSettings()}>{t.openPrivacy}</button>
          <button type="button" class={secondary} onclick={recheckPermission}>{t.recheck}</button>
        {/if}
      </div>

      <div class="flex flex-col gap-3 panel p-5">
        <button type="button" class={[primary, "self-start"]} disabled={testing} onclick={runTest}>
          {t.startTest}
        </button>
        {#if testing}
          <p class="text-ink-soft">{t.testing}</p>
        {/if}
        <div class="grid grid-cols-[7.5rem_1fr] items-center gap-x-3 gap-y-2 text-sm" aria-hidden="true">
          <span class="font-semibold text-mic">{id.recorder.mic}</span>
          <div class="h-1.5 overflow-hidden rounded-full bg-line-soft">
            <div class="h-full rounded-full bg-mic transition-[width] duration-100" style:width={meterWidth(micDb)}></div>
          </div>
          <span class="font-semibold text-system">{id.recorder.system}</span>
          <div class="h-1.5 overflow-hidden rounded-full bg-line-soft">
            <div class="h-full rounded-full bg-system transition-[width] duration-100" style:width={meterWidth(sysDb)}></div>
          </div>
        </div>
        {#if result}
          <p class={result.micOk ? "text-ok" : "text-bad"}>{result.micOk ? t.micOk : t.micFail}</p>
          <p class={result.systemOk ? "text-ok" : "text-warn"}>
            {result.systemOk ? t.systemOk : t.systemFail}
          </p>
        {/if}
        {#if testError}
          <p class="text-bad">{testError}</p>
        {/if}
      </div>

      <button type="button" class={[primary, "self-start"]} disabled={!result?.micOk} onclick={() => (step = 4)}>
        {t.next}
      </button>
    </section>
  {:else}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.doneTitle}</h1>
      <p class="leading-relaxed">{t.tip}</p>
      {#if shortcut}
        <p class="leading-relaxed">{t.shortcutTip(shortcut)}</p>
      {/if}
      <label class="flex items-center gap-2">
        <input type="checkbox" class="h-4 w-4" bind:checked={autostart} />
        <span>{t.autostart}</span>
      </label>
      <button type="button" class={[primary, "self-start"]} onclick={finish}>{t.start}</button>
      {#if testError}
        <p class="text-bad">{testError}</p>
      {/if}
    </section>
  {/if}
</main>
