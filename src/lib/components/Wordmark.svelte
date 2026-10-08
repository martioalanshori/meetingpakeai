<script lang="ts">
  // Tanda aplikasi: dua batang = dua suara yang direkam (Saya / Peserta lain).
  let { size = 22 }: { size?: number } = $props();
</script>

<span class="inline-flex items-center gap-2.5">
  <svg width={size} height={size} viewBox="0 0 22 22" aria-hidden="true">
    <rect x="4" y="3" width="5" height="16" rx="2.5" fill="var(--color-mic)" />
    <rect x="13" y="7" width="5" height="12" rx="2.5" fill="var(--color-system)" />
  </svg>
  <span class="text-[1.0625rem] leading-none font-bold tracking-[-0.01em] text-ink">Meeting Pake AI</span>
</span>
