<script lang="ts" generics="T extends string">
  // Pilihan tunggal sebagai kartu (pengganti radio bawaan): judul + penjelas, kartu terpilih bertepi tinta.
  let {
    name,
    value,
    options,
    onchange,
  }: {
    name: string;
    value: T;
    options: { value: T; label: string; hint?: string }[];
    onchange: (value: T) => void;
  } = $props();
</script>

<div class="grid gap-2 sm:grid-cols-[repeat(auto-fit,minmax(12rem,1fr))]" role="radiogroup">
  {#each options as o (o.value)}
    <label
      class={[
        "flex cursor-pointer items-start gap-3 rounded-[var(--radius-btn)] border px-3.5 py-3 transition-colors",
        value === o.value ? "border-ink bg-sheet shadow-[inset_0_0_0_1px_var(--color-ink)]" : "border-line hover:border-ink-faint",
      ]}
    >
      <input
        type="radio"
        class="sr-only"
        {name}
        value={o.value}
        checked={value === o.value}
        onchange={() => onchange(o.value)}
      />
      <span
        class={[
          "mt-0.5 flex h-4 w-4 shrink-0 items-center justify-center rounded-full border-2",
          value === o.value ? "border-ink-strong" : "border-line",
        ]}
        aria-hidden="true"
      >
        {#if value === o.value}<span class="h-2 w-2 rounded-full bg-ink-strong"></span>{/if}
      </span>
      <span class="flex flex-col gap-0.5">
        <span class="text-sm font-semibold">{o.label}</span>
        {#if o.hint}<span class="text-sm leading-snug text-ink-soft">{o.hint}</span>{/if}
      </span>
    </label>
  {/each}
</div>

<style>
  label:has(input:focus-visible) {
    outline: 2px solid var(--color-ink);
    outline-offset: 2px;
  }
</style>
