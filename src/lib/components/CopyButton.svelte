<script lang="ts">
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import { markdownToHtml, type MinutesStyle } from "$lib/minutes";
  import { showToast } from "$lib/toast.svelte";

  // Tombol salin isi satu tab: teks biasa + HTML (judul tebal, daftar berpoin saat ditempel ke email/Docs/Word).
  let { text, okText = id.minutes.copied }: { text: (style: MinutesStyle) => string; okText?: string } = $props();

  let copied = $state<MinutesStyle | null>(null);
  let timer: ReturnType<typeof setTimeout> | undefined;

  async function copy(style: MinutesStyle) {
    try {
      const plain = text(style);
      try {
        const html = markdownToHtml(text("markdown"));
        await navigator.clipboard.write([
          new ClipboardItem({
            "text/plain": new Blob([plain], { type: "text/plain" }),
            "text/html": new Blob([html], { type: "text/html" }),
          }),
        ]);
      } catch {
        await navigator.clipboard.writeText(plain);
      }
      copied = style;
      clearTimeout(timer);
      timer = setTimeout(() => (copied = null), 2000);
      showToast(okText, "success", 2000);
    } catch {
      showToast(id.minutes.copyFailed, "error");
    }
  }

  const btn = "btn btn-line btn-sm";
</script>

<div class="flex items-center gap-1.5">
  <button type="button" class={btn} onclick={() => copy("text")}>
    <Icon name={copied === "text" ? "check" : "copy"} size={15} />
    {copied === "text" ? id.minutes.copiedShort : id.minutes.copyTab}
  </button>
</div>
