<script lang="ts">
  // Logo aplikasi (Logo.png: empat kotak hitam + kotak tengah oranye), digambar ulang sebagai SVG agar tajam di semua ukuran.
  let { size = 22, showName = true }: { size?: number; showName?: boolean } = $props();
</script>

<span class="inline-flex items-center gap-2.5">
  <svg width={size} height={size} viewBox="0 0 1080 1080" aria-hidden="true">
    <g fill="#0a0a0a">
      <rect x="0" y="0" width="330" height="322" rx="55" />
      <rect x="759" y="0" width="320" height="322" rx="55" />
      <rect x="0" y="750" width="330" height="329" rx="55" />
      <rect x="759" y="750" width="320" height="329" rx="55" />
    </g>
    <rect x="380" y="371" width="320" height="329" rx="55" fill="#ff4a1c" />
  </svg>
  {#if showName}
    <span class="text-[1.0625rem] leading-none font-bold tracking-[-0.01em] text-ink">Meeting Pake AI</span>
  {/if}
</span>
