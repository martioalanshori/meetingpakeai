<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import type { MinutesStyle } from "$lib/minutes";
  import { showToast } from "$lib/toast.svelte";

  // Tombol salin isi satu tab. `whatsapp` → tombol kedua dengan format *tebal* dan bullet.
  let {
    text,
    whatsapp = false,
    okText = id.minutes.copied,
  }: { text: (style: MinutesStyle) => string; whatsapp?: boolean; okText?: string } = $props();

  let copied = $state<MinutesStyle | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(style: MinutesStyle) {
    try {
      await navigator.clipboard.writeText(text(style));
      copied = style;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = null), 2000);
      showToast(okText, "success", 2000);
    } catch {
      showToast(id.minutes.copyFailed, "error");
    }
  }

  const btn = "flex items-center gap-1.5 rounded-lg border border-gray-300 bg-white px-3 py-1 text-sm hover:bg-gray-100";
</script>

<div class="flex items-center gap-1.5">
  <button type="button" class={btn} onclick={() => copy("text")}>
    <Icon name={copied === "text" ? "check" : "copy"} size={15} />
    {copied === "text" ? id.minutes.copiedShort : id.minutes.copyTab}
  </button>
  {#if whatsapp}
    <button type="button" class={btn} title={id.minutes.copyWhatsapp} onclick={() => copy("whatsapp")}>
      <Icon name={copied === "whatsapp" ? "check" : "copy"} size={15} />
      {copied === "whatsapp" ? id.minutes.copiedShort : id.minutes.copyWhatsappShort}
    </button>
  {/if}
</div>
