<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import type { MinutesStyle } from "$lib/minutes";
  import { showToast } from "$lib/toast.svelte";

  // Tombol salin isi satu tab (teks biasa).
  let { text, okText = id.minutes.copied }: { text: (style: MinutesStyle) => string; okText?: string } = $props();

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

  const btn = "btn btn-line py-1.5";
</script>

<div class="flex items-center gap-1.5">
  <button type="button" class={btn} onclick={() => copy("text")}>
    <Icon name={copied === "text" ? "check" : "copy"} size={15} />
    {copied === "text" ? id.minutes.copiedShort : id.minutes.copyTab}
  </button>
</div>
