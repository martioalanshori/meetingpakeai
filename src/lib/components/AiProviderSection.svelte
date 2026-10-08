<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "$lib/api";
  import AiRoleEditor from "$lib/components/AiRoleEditor.svelte";
  import { id } from "$lib/i18n/id";
  import type { AiConfig, AppError } from "$lib/types";

  // Layanan AI: Transkrip dan Ringkasan bisa memakai penyedia berbeda. `onchange(ready)` = kedua peran siap.
  let { onchange, showHeading = true }: { onchange?: (ready: boolean) => void; showHeading?: boolean } = $props();

  let config = $state<AiConfig | null>(null);
  let error = $state<string | null>(null);

  async function reload() {
    try {
      config = await api.getAiConfig();
      const ready = (["stt", "llm"] as const).every((r) => {
        const c = config![r];
        const preset = config!.presets.find((p) => p.id === c.provider);
        return !preset?.keyRequired || config!.keysSet.includes(c.provider);
      });
      onchange?.(ready);
    } catch (e) {
      error = (e as AppError).message;
    }
  }

  onMount(reload);
</script>

<div class="flex flex-col gap-6">
  {#if showHeading}
    <div class="flex flex-col gap-1">
      <h2 class="text-lg font-bold">{id.ai.heading}</h2>
      <p class="max-w-prose text-sm text-ink-soft">{id.ai.intro}</p>
    </div>
  {/if}
  {#if error}
    <p class="text-sm text-bad">{error}</p>
  {:else if config}
    <AiRoleEditor role="stt" {config} onsaved={reload} />
    <div class="border-t border-line-soft"></div>
    <AiRoleEditor role="llm" {config} onsaved={reload} />
  {/if}
</div>
