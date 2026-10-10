<script lang="ts">
  // Widget tawaran rekam (langkah 46): muncul di atas semua jendela saat meeting terdeteksi,
  // tetap terlihat walau Windows Do Not Disturb menahan notifikasi. `auto=1` → hitung mundur lalu merekam.
  import { onDestroy, onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { page } from "$app/state";
  import { api } from "$lib/api";
  import Icon from "$lib/components/Icon.svelte";
  import { id } from "$lib/i18n/id";
  import type { AppError } from "$lib/types";

  const t = id.offer;
  const AUTO_SECONDS = 10;
  /** Tawaran tanpa respons hilang sendiri (pengingat ulang diatur Rust setelah 3 menit). */
  const EXPIRE_MS = 120_000;

  const kind = page.url.searchParams.get("kind") ?? "browser";
  const auto = page.url.searchParams.get("auto") === "1";
  const app = t.apps[kind] ?? kind;

  let left = $state(AUTO_SECONDS);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let timer: ReturnType<typeof setInterval> | undefined;
  let expire: ReturnType<typeof setTimeout> | undefined;

  function close() {
    getCurrentWindow().close().catch(() => {});
  }

  async function record() {
    if (busy) return;
    busy = true;
    clearInterval(timer);
    try {
      await api.startRecording(kind);
      // Rust menutup widget ini saat widget rekaman dibuka; tutup juga di sini untuk jaga-jaga.
      close();
    } catch (e) {
      error = (e as AppError).message;
      busy = false;
    }
  }

  async function dismiss() {
    clearInterval(timer);
    await api.dismissMeetingOffer().catch(() => {});
    close();
  }

  onMount(() => {
    if (auto) {
      timer = setInterval(() => {
        left -= 1;
        if (left <= 0) record();
      }, 1000);
    } else {
      expire = setTimeout(close, EXPIRE_MS);
    }
  });
  onDestroy(() => {
    clearInterval(timer);
    clearTimeout(expire);
  });
</script>

<div class="on-dark flex h-screen select-none flex-col justify-center gap-2 bg-ink px-3 text-white" data-tauri-drag-region>
  <div class="flex items-center gap-2.5" data-tauri-drag-region>
    <span class="relative flex h-2.5 w-2.5 shrink-0" aria-hidden="true">
      <span class="absolute inline-flex h-full w-full rounded-full bg-rec opacity-60 motion-safe:animate-ping"></span>
      <span class="relative inline-flex h-2.5 w-2.5 rounded-full bg-rec"></span>
    </span>
    <div class="flex min-w-0 flex-1 flex-col" data-tauri-drag-region>
      <span class="truncate text-sm font-semibold" data-tauri-drag-region>{t.detected(app)}</span>
      <span class="truncate text-sm text-white/70" role="status" data-tauri-drag-region>
        {error ?? (auto ? t.autoIn(left) : t.question)}
      </span>
    </div>
    <button
      type="button"
      class="rounded p-1 text-white/70 hover:bg-white/15 hover:text-white"
      aria-label={auto ? t.cancel : t.dismiss}
      title={auto ? t.cancel : t.dismiss}
      onclick={dismiss}
    >
      <Icon name="x" size={16} />
    </button>
  </div>
  <div class="flex gap-2">
    <button
      type="button"
      class="flex flex-1 items-center justify-center gap-1.5 rounded-md bg-rec px-3 py-1.5 text-sm font-semibold hover:bg-rec-hover disabled:opacity-60"
      disabled={busy}
      onclick={record}
    >
      <span class="h-2 w-2 rounded-full bg-white" aria-hidden="true"></span>{auto ? `${t.record} sekarang` : t.record}
    </button>
    <button
      type="button"
      class="rounded-md bg-white/12 px-3 py-1.5 text-sm font-semibold hover:bg-white/20"
      onclick={dismiss}
    >
      {auto ? t.cancel : t.dismiss}
    </button>
  </div>
</div>
