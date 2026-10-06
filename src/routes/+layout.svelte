<script lang="ts">
  import "../app.css";
  import { onMount } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/state";
  import { api } from "$lib/api";
  import ConsentDialog from "$lib/components/ConsentDialog.svelte";
  import Toaster from "$lib/components/Toaster.svelte";
  import { initRecording, openConsent, rec } from "$lib/recording.svelte";

  let { children } = $props();

  // Jendela widget rekaman memakai halaman /recorder tanpa elemen jendela main.
  const isRecorderWindow = $derived(page.url.pathname.startsWith("/recorder"));

  onMount(async () => {
    if (page.url.pathname.startsWith("/recorder")) return;
    await initRecording();
    // Jendela dibuat ulang dari menu tray "Mulai rekam" → langsung buka popup consent.
    if (await api.takePendingConsent().catch(() => false)) openConsent();
    try {
      const s = await api.getOnboardingStatus();
      if (!s.completed && !page.url.pathname.startsWith("/onboarding")) await goto("/onboarding");
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
