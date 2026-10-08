<script lang="ts">
  import "../app.css";
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { api, events } from "$lib/api";
  import ConsentDialog from "$lib/components/ConsentDialog.svelte";
  import Toaster from "$lib/components/Toaster.svelte";
  import { initRecording, openConsent, rec } from "$lib/recording.svelte";

  let { children } = $props();

  // Jendela widget rekaman memakai halaman /recorder tanpa elemen jendela main.
  const isRecorderWindow = $derived(page.url.pathname.startsWith("/recorder"));

  /** Notifikasi diklik / jendela dibuka: buka meeting yang baru selesai, atau popup consent dari tawaran rekam. */
  async function openPending() {
    const meetingId = await api.takePendingMeeting().catch(() => null);
    if (meetingId) await goto(`/meeting/${meetingId}`);
    const consent = await api.takePendingConsent().catch(() => null);
    if (consent?.open) openConsent(consent.sourceApp);
  }

  const unlisten: UnlistenFn[] = [];
  onDestroy(() => unlisten.forEach((u) => u()));

  onMount(async () => {
    if (page.url.pathname.startsWith("/recorder")) return;
    await initRecording();
    unlisten.push(
      await getCurrentWindow().onFocusChanged(({ payload: focused }) => {
        if (focused) openPending();
      }),
      await events.appPending(() => openPending()),
    );
    try {
      const s = await api.getOnboardingStatus();
      if (!s.completed && !page.url.pathname.startsWith("/onboarding")) await goto("/onboarding");
      // Jendela dibuat dari menu tray / shortcut / notifikasi → popup consent atau detail meeting.
      else await openPending();
    } catch {
      /* tetap di halaman sekarang */
    }
  });
</script>

{@render children()}

{#if !isRecorderWindow}
  {#if rec.consentOpen}
    <ConsentDialog />
  {/if}
  <Toaster />
{/if}
