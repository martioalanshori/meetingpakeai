<script lang="ts">
  import "../app.css";
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { api, events } from "$lib/api";
  import AppRail from "$lib/components/AppRail.svelte";
  import ConfirmHost from "$lib/components/ConfirmHost.svelte";
  import MeetingOfferBanner from "$lib/components/MeetingOfferBanner.svelte";
  import Toaster from "$lib/components/Toaster.svelte";
  import { initRecording, rec } from "$lib/recording.svelte";
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import { toggleRail, ui } from "$lib/ui.svelte";

  let { children } = $props();

  // Jendela widget rekaman memakai halaman /recorder tanpa elemen jendela main.
  const isRecorderWindow = $derived(page.url.pathname.startsWith("/recorder"));
  const isOnboarding = $derived(page.url.pathname.startsWith("/onboarding"));

  /** Notifikasi diklik / jendela dibuka: buka meeting yang baru selesai, atau banner tawaran rekam. */
  async function openPending() {
    const meetingId = await api.takePendingMeeting().catch(() => null);
    if (meetingId) await goto(`/meeting/${meetingId}`);
    const nav = await api.takePendingNav().catch(() => null);
    if (nav) await goto(nav);
    const offer = await api.takePendingOffer().catch(() => null);
    if (offer && rec.state.status === "idle") rec.offer = offer;
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
      // Jendela dibuat dari notifikasi → banner tawaran rekam atau detail meeting.
      else await openPending();
    } catch {
      /* tetap di halaman sekarang */
    }
  });
</script>

<svelte:window
  onkeydown={(e) => {
    // Ctrl+B: sembunyikan / tampilkan sidebar (hanya jendela main).
    if (isRecorderWindow || isOnboarding) return;
    if (e.ctrlKey && !e.altKey && !e.shiftKey && e.key.toLowerCase() === "b") {
      e.preventDefault();
      toggleRail();
    }
  }}
/>

{#if isRecorderWindow || isOnboarding}
  {@render children()}
{:else}
  <div class="flex h-full print:block">
    {#if ui.railHidden}
      <!-- Sidebar disembunyikan: sisakan strip tipis berisi tombol tampilkan (isi tidak tertutup). -->
      <div class="flex w-11 shrink-0 justify-center pt-5 print:hidden">
        <button
          type="button"
          class="h-fit rounded-lg p-1.5 text-ink-soft hover:bg-wash hover:text-ink"
          title={id.rail.show}
          aria-label={id.rail.show}
          onclick={() => toggleRail(false)}
        >
          <Icon name="panel-left" size={18} />
        </button>
      </div>
    {:else}
      <div class="contents print:hidden"><AppRail /></div>
    {/if}
    <div class="min-w-0 flex-1 overflow-y-auto [scrollbar-gutter:stable] border-l border-line bg-sheet print:overflow-visible print:border-0">
      {@render children()}
    </div>
  </div>
{/if}

{#if !isRecorderWindow}
  <MeetingOfferBanner />
  <ConfirmHost />
  <Toaster />
{/if}
