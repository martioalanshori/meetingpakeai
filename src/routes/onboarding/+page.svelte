<script lang="ts">
  import { onDestroy, onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { cubicOut } from "svelte/easing";
  import { goto } from "$app/navigation";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { api, events } from "$lib/api";
  import { startTour } from "$lib/tour.svelte";
  import AiProviderSection from "$lib/components/AiProviderSection.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import Wordmark from "$lib/components/Wordmark.svelte";
  import { id } from "$lib/i18n/id";
  import type { AppError, AudioTestResult, MicPermission } from "$lib/types";

  const t = id.onboarding;

  let step = $state(1);
  /** Arah animasi pindah langkah: 1 = maju (masuk dari kanan), -1 = kembali (dari kiri). */
  let dir = $state(1);
  let prevStep = 1;
  $effect.pre(() => {
    dir = step >= prevStep ? 1 : -1;
    prevStep = step;
  });
  const reducedMotion = typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  /** Isi langkah baru masuk bergeser + memudar; yang lama langsung hilang agar ukuran tidak bertumpuk. */
  const stepIn = () => ({ x: 28 * dir, duration: reducedMotion ? 0 : 280, easing: cubicOut });
  let autostart = $state(true);
  let shortcut = $state("");
  let name = $state("");

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
    const settings = await api.getSettings().catch(() => null);
    shortcut = settings?.globalShortcut ?? "";
    // Nama bawaan ("Saya") tidak diisikan agar kolom tampil kosong dengan placeholder.
    name = settings && settings.userDisplayName !== "Saya" ? settings.userDisplayName : "";
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

  /** Langkah 2 → 3: simpan nama (kosong = kembali ke bawaan "Saya"). */
  /** Nama wajib diisi sebelum lanjut. */
  const nameOk = $derived(name.trim() !== "");

  async function saveName() {
    if (!nameOk) return;
    await api.updateSettings({ userDisplayName: name.trim() }).catch(() => {});
    step = 3;
  }

  async function finish(tour: boolean) {
    try {
      await api.updateSettings({ autostart }).catch(() => {});
      await api.completeOnboarding();
      await goto("/");
      // Pengguna baru langsung diajak tur: tiap fitur disorot di aplikasi dan bisa dicoba.
      if (tour) startTour();
    } catch (e) {
      testError = (e as AppError).message;
    }
  }

  function meterWidth(db: number) {
    return `${Math.max(0, Math.min(100, ((db + 60) / 60) * 100))}%`;
  }

  const primary = "btn btn-ink px-5 py-2.5";
  const secondary = "btn btn-line";
  /** Baris tombol: selalu di dasar kartu, tetap terlihat saat isi langkah bergulir. */
  const footer = "sticky bottom-0 flex items-center gap-2 bg-sheet pt-3";
  /** Isi langkah: di tengah ruang di atas tombol (langkah pendek tidak menyisakan ruang kosong di bawah). */
  const body = "my-auto flex flex-col gap-5 py-2";
</script>

<main class="flex min-h-full w-full items-center justify-center px-6 py-10">
<!-- Ukuran kartu tetap di semua langkah (tidak melompat); isi panjang bergulir, tombol selalu di bawah. -->
<div class="flex h-[min(36rem,calc(100vh-5rem))] w-full max-w-xl flex-col gap-7 rounded-[var(--radius-box)] border border-line bg-sheet p-8 shadow-[0_24px_60px_-28px_rgb(28_31_38/0.35)]">
  <div class="flex items-center justify-between gap-4">
    <Wordmark size={28} />
    <span class="tabular text-sm text-ink-soft">{id.onboarding.stepOf(step, 5)}</span>
  </div>
  <ol class="flex gap-1.5" aria-hidden="true">
    {#each [1, 2, 3, 4, 5] as n (n)}
      <li class={["h-1 flex-1 rounded-full transition-colors duration-300 motion-reduce:transition-none", n <= step ? "bg-ink" : "bg-line"]}></li>
    {/each}
  </ol>

  <div class="-mx-2 flex min-h-0 flex-1 flex-col overflow-x-hidden overflow-y-auto px-2">
  {#key step}
  <div class="flex flex-1 flex-col" in:fly={stepIn()}>
  {#if step === 1}
    <section class="flex flex-1 flex-col">
      <div class={body}>
        <div class="flex flex-col gap-2">
          <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.welcomeTitle}</h1>
          <p class="text-ink-soft">{t.welcomeLead}</p>
        </div>
      </div>
      <div class={footer}>
        <button type="button" class={primary} onclick={() => (step = 2)}>{t.next}</button>
      </div>
    </section>
  {:else if step === 2}
    <section class="flex flex-1 flex-col">
      <div class={body}>
        <div class="flex flex-col gap-2">
          <label class="text-2xl font-bold tracking-[-0.02em]" for="onboarding-name">{t.nameTitle}</label>
          <input
            id="onboarding-name"
            class="field max-w-sm"
            maxlength="50"
          required
          aria-required="true"
            placeholder={t.namePlaceholder}
            autocomplete="given-name"
            bind:value={name}
            onkeydown={(e) => {
              if (e.key === "Enter") saveName();
            }}
          />
          <span class="hint max-w-prose">{t.nameHint}</span>
        </div>
      </div>
      <div class={footer}>
        <button type="button" class="btn btn-quiet" onclick={() => (step = 1)}><Icon name="arrow-left" size={16} />{t.back}</button>
        <button type="button" class={primary} disabled={!nameOk} onclick={saveName}>{t.next}</button>
      </div>
    </section>
  {:else if step === 3}
    <section class="flex flex-1 flex-col">
      <div class={body}>
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
      </div>
      <div class={footer}>
        <button type="button" class="btn btn-quiet" onclick={() => (step = 2)}><Icon name="arrow-left" size={16} />{t.back}</button>
        <button type="button" class={primary} disabled={!keyOk} onclick={() => (step = 4)}>{t.next}</button>
      </div>
    </section>
  {:else if step === 4}
    <section class="flex flex-1 flex-col">
      <div class={body}>
        <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.micTitle}</h1>
        <!-- Izin mikrofon: lencana kecil; tombol perbaikan hanya bila belum diizinkan. -->
        <div class="flex flex-wrap items-center gap-2">
          <span
            class={[
              "inline-flex items-center gap-1.5 rounded-full px-3 py-1 text-sm font-medium",
              permission === "allowed" ? "bg-ok/10 text-ok" : permission === "denied" ? "bg-bad/10 text-bad" : "bg-line-soft text-ink-soft",
            ]}
          >
            <Icon name={permission === "allowed" ? "check-circle" : "alert-circle"} size={15} />
            {permission === "allowed" ? t.permAllowed : permission === "denied" ? t.permDenied : t.permUnknown}
          </span>
          {#if permission !== "allowed"}
            <button type="button" class="btn btn-line btn-sm" onclick={() => api.openMicSettings()}>{t.openPrivacy}</button>
            <button type="button" class="btn btn-quiet btn-sm" onclick={recheckPermission}>{t.recheck}</button>
          {/if}
        </div>

        <!-- Tes: dua baris sumber suara (ikon, nama, status, meter); penjelasan panjang hanya saat gagal. -->
        <div class="flex flex-col gap-4 rounded-[var(--radius-box)] border border-line p-5">
          <p class={["text-sm leading-relaxed", testing ? "font-semibold text-ink" : "text-ink-soft"]}>{t.testing}</p>

          <ul class="flex flex-col gap-3">
            {#each [
              { key: "mic", label: id.recorder.mic, icon: "mic", db: micDb, ok: result?.micOk, fail: t.micFail, warnOnly: false },
              { key: "system", label: id.recorder.system, icon: "speaker", db: sysDb, ok: result?.systemOk, fail: t.systemFail, warnOnly: true },
            ] as ch (ch.key)}
              {@const done = result !== null && result !== undefined}
              <li class="flex flex-col gap-2">
                <div class="flex items-center gap-3">
                  <span
                    class={[
                      "flex h-9 w-9 shrink-0 items-center justify-center rounded-full transition-colors duration-300",
                      ch.key === "mic" ? "bg-mic/10 text-mic" : "bg-system/10 text-system",
                    ]}
                  >
                    <Icon name={ch.icon as "mic" | "speaker"} size={17} />
                  </span>
                  <div class="flex min-w-0 flex-1 flex-col gap-1.5">
                    <div class="flex items-center justify-between gap-2">
                      <span class="text-sm font-semibold">{ch.label}</span>
                      {#if testing}
                        <span class="text-sm text-ink-soft motion-safe:animate-pulse">{t.statusListening}</span>
                      {:else if done && ch.ok}
                        <span class="inline-flex items-center gap-1 text-sm font-semibold text-ok"><Icon name="check-circle" size={14} />{t.statusOk}</span>
                      {:else if done}
                        <span class={["inline-flex items-center gap-1 text-sm font-semibold", ch.warnOnly ? "text-warn" : "text-bad"]}
                          ><Icon name="alert-circle" size={14} />{t.statusFail}</span
                        >
                      {:else}
                        <span class="text-sm text-ink-faint">{t.statusIdle}</span>
                      {/if}
                    </div>
                    <div class="h-1.5 overflow-hidden rounded-full bg-line-soft" aria-hidden="true">
                      <div
                        class={["h-full rounded-full transition-[width] duration-100", ch.key === "mic" ? "bg-mic" : "bg-system"]}
                        style:width={meterWidth(ch.db)}
                      ></div>
                    </div>
                  </div>
                </div>
                {#if done && !ch.ok}
                  <p class={["pl-12 text-sm leading-relaxed", ch.warnOnly ? "text-warn" : "text-bad"]}>{ch.fail}</p>
                {/if}
              </li>
            {/each}
          </ul>

          <button type="button" class={["btn self-start", result?.micOk ? "btn-line" : "btn-ink"]} disabled={testing} onclick={runTest}>
            <Icon name={testing ? "refresh" : "play"} size={14} class={testing ? "motion-safe:animate-spin" : ""} />
            {testing ? t.testRunning : result ? t.retest : t.startTest}
          </button>
          {#if testError}
            <p class="text-sm text-bad">{testError}</p>
          {/if}
        </div>

      </div>
      <div class={footer}>
        <button type="button" class="btn btn-quiet" onclick={() => (step = 3)}><Icon name="arrow-left" size={16} />{t.back}</button>
        <button type="button" class={primary} disabled={!result?.micOk} onclick={() => (step = 5)}>{t.next}</button>
      </div>
    </section>
  {:else}
    <section class="flex flex-1 flex-col">
      <div class={body}>
        <h1 class="text-2xl font-bold tracking-[-0.02em]">{t.doneTitle}</h1>
        <p class="leading-relaxed">{t.tip}</p>
        <p class="leading-relaxed">{t.guideNext}</p>
        {#if shortcut}
          <p class="leading-relaxed">{t.shortcutTip(shortcut)}</p>
        {/if}
        <label class="flex items-center gap-2">
          <input type="checkbox" class="h-4 w-4" bind:checked={autostart} />
          <span>{t.autostart}</span>
        </label>
      </div>
      <div class={footer}>
        <button type="button" class="btn btn-quiet" onclick={() => (step = 4)}><Icon name="arrow-left" size={16} />{t.back}</button>
        <button type="button" class={primary} onclick={() => finish(true)}>{t.startTour}</button>
        <button type="button" class="btn btn-quiet" onclick={() => finish(false)}>{t.skipTour}</button>
      </div>
      {#if testError}
        <p class="text-bad">{testError}</p>
      {/if}
    </section>
  {/if}
  </div>
  {/key}
  </div>
</div>
</main>
