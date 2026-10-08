<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import { id } from "$lib/i18n/id";
  import type { AppError, AudioTestResult, MicPermission } from "$lib/types";

  const t = id.onboarding;

  let step = $state(1);
  let autostart = $state(true);
  let shortcut = $state("");

  // Langkah 2: API key
  let keyInput = $state("");
  let keyOk = $state(false);
  let keyBusy = $state(false);
  let keyMessage = $state<{ kind: "ok" | "warn" | "error"; text: string } | null>(null);

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

  async function testKey() {
    keyBusy = true;
    keyMessage = null;
    try {
      const res = await api.testApiKey(keyInput);
      await api.saveApiKey(keyInput);
      keyOk = true;
      keyInput = "";
      keyMessage =
        res.missingModels.length > 0
          ? { kind: "warn", text: id.settings.apiKey.missingModels(res.missingModels) }
          : { kind: "ok", text: id.settings.apiKey.saveOk };
    } catch (e) {
      keyMessage = { kind: "error", text: (e as AppError).message };
    } finally {
      keyBusy = false;
    }
  }

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

  const primary =
    "rounded-lg bg-indigo-600 px-5 py-2.5 font-medium text-white hover:bg-indigo-700 disabled:cursor-not-allowed disabled:opacity-50";
  const secondary = "rounded-lg border border-gray-300 px-4 py-2 hover:bg-gray-50 disabled:opacity-50";
</script>

<main class="mx-auto flex min-h-full max-w-2xl flex-col justify-center gap-6 p-8">
  <ol class="flex gap-2" aria-label="Langkah">
    {#each [1, 2, 3, 4] as n (n)}
      <li class={["h-1.5 flex-1 rounded", n <= step ? "bg-indigo-600" : "bg-gray-200"]}></li>
    {/each}
  </ol>

  {#if step === 1}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-semibold">{t.welcomeTitle}</h1>
      <p class="leading-relaxed text-gray-700">{t.privacy}</p>
      <button type="button" class={[primary, "self-start"]} onclick={() => (step = 2)}>{t.understand}</button>
    </section>
  {:else if step === 2}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-semibold">{t.apiKeyTitle}</h1>
      <ol class="list-decimal space-y-1 pl-6 text-gray-700">
        {#each t.apiKeySteps as s (s)}
          <li>{s}</li>
        {/each}
      </ol>
      <button type="button" class={[secondary, "self-start"]} onclick={() => openUrl("https://console.groq.com/keys")}>
        {t.openConsole}
      </button>
      <form
        class="flex flex-wrap items-end gap-2"
        onsubmit={(e) => {
          e.preventDefault();
          testKey();
        }}
      >
        <label class="flex min-w-64 flex-1 flex-col gap-1 text-sm">
          {id.settings.apiKey.inputLabel}
          <input
            type="password"
            autocomplete="off"
            spellcheck="false"
            class="rounded-lg border border-gray-300 px-3 py-2 font-mono"
            placeholder={id.settings.apiKey.inputPlaceholder}
            bind:value={keyInput}
          />
        </label>
        <button type="submit" class={secondary} disabled={keyBusy || keyInput.trim() === ""}>
          {keyBusy ? id.settings.apiKey.testing : id.settings.apiKey.test}
        </button>
      </form>
      {#if keyMessage}
        <p
          role="status"
          class={[
            "text-sm",
            keyMessage.kind === "ok" && "text-emerald-700",
            keyMessage.kind === "warn" && "text-amber-700",
            keyMessage.kind === "error" && "text-red-700",
          ]}
        >
          {keyMessage.text}
        </p>
      {:else if keyOk}
        <p class="text-sm text-emerald-700">{id.settings.apiKey.saved}</p>
      {/if}
      <button type="button" class={[primary, "self-start"]} disabled={!keyOk} onclick={() => (step = 3)}>
        {t.next}
      </button>
    </section>
  {:else if step === 3}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-semibold">{t.micTitle}</h1>
      <div class="flex flex-wrap items-center gap-3">
        <p
          class={[
            permission === "allowed" && "text-emerald-700",
            permission === "denied" && "text-red-700",
            permission === "unknown" && "text-gray-700",
          ]}
        >
          {permission === "allowed" ? t.permAllowed : permission === "denied" ? t.permDenied : t.permUnknown}
        </p>
        {#if permission !== "allowed"}
          <button type="button" class={secondary} onclick={() => api.openMicSettings()}>{t.openPrivacy}</button>
          <button type="button" class={secondary} onclick={recheckPermission}>{t.recheck}</button>
        {/if}
      </div>

      <div class="flex flex-col gap-3 rounded-xl border border-gray-200 bg-white p-5">
        <button type="button" class={[primary, "self-start"]} disabled={testing} onclick={runTest}>
          {t.startTest}
        </button>
        {#if testing}
          <p class="text-gray-700">{t.testing}</p>
        {/if}
        <div class="grid grid-cols-[4rem_1fr] items-center gap-2 text-sm" aria-hidden="true">
          <span>{id.recorder.mic}</span>
          <div class="h-2 overflow-hidden rounded bg-gray-200">
            <div class="h-full bg-sky-500 transition-[width] duration-100" style:width={meterWidth(micDb)}></div>
          </div>
          <span>{id.recorder.system}</span>
          <div class="h-2 overflow-hidden rounded bg-gray-200">
            <div class="h-full bg-emerald-500 transition-[width] duration-100" style:width={meterWidth(sysDb)}></div>
          </div>
        </div>
        {#if result}
          <p class={result.micOk ? "text-emerald-700" : "text-red-700"}>{result.micOk ? t.micOk : t.micFail}</p>
          <p class={result.systemOk ? "text-emerald-700" : "text-amber-700"}>
            {result.systemOk ? t.systemOk : t.systemFail}
          </p>
        {/if}
        {#if testError}
          <p class="text-red-700">{testError}</p>
        {/if}
      </div>

      <button type="button" class={[primary, "self-start"]} disabled={!result?.micOk} onclick={() => (step = 4)}>
        {t.next}
      </button>
    </section>
  {:else}
    <section class="flex flex-col gap-5">
      <h1 class="text-2xl font-semibold">{t.doneTitle}</h1>
      <p class="text-gray-700">🎧 {t.tip}</p>
      {#if shortcut}
        <p class="text-gray-700">⌨ {t.shortcutTip(shortcut)}</p>
      {/if}
      <label class="flex items-center gap-2">
        <input type="checkbox" class="h-4 w-4" bind:checked={autostart} />
        <span>{t.autostart}</span>
      </label>
      <button type="button" class={[primary, "self-start"]} onclick={finish}>{t.start}</button>
      {#if testError}
        <p class="text-red-700">{testError}</p>
      {/if}
    </section>
  {/if}
</main>
