<script lang="ts">
  // Saklar on/off (pengganti checkbox untuk pengaturan yang langsung tersimpan).
  let {
    checked,
    label,
    disabled = false,
    onchange,
  }: { checked: boolean; label: string; disabled?: boolean; onchange: (value: boolean) => void } = $props();
</script>

<button
  type="button"
  role="switch"
  aria-checked={checked}
  aria-label={label}
  {disabled}
  class={[
    "relative inline-flex h-6 w-10 shrink-0 items-center rounded-full transition-colors disabled:opacity-50",
    checked ? "bg-ink-strong" : "bg-line",
  ]}
  onclick={() => onchange(!checked)}
>
  <span
    class={[
      "absolute left-0.5 h-5 w-5 rounded-full bg-white shadow-[0_1px_2px_rgb(28_31_38/0.25)] transition-transform",
      checked && "translate-x-4",
    ]}
  ></span>
</button>
