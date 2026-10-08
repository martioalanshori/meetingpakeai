<script lang="ts">
  import { id } from "$lib/i18n/id";

  // Perekam kombinasi tombol: format sama dengan parser global-hotkey ("Ctrl+Alt+R").
  let { value = $bindable("") }: { value: string } = $props();

  const MODIFIER_KEYS = ["Control", "Alt", "Shift", "Meta", "AltGraph"];

  /** KeyboardEvent.code → nama tombol ("KeyR" → "R", "Digit1" → "1", "F5" → "F5"). */
  function keyName(code: string): string | null {
    if (/^Key[A-Z]$/.test(code)) return code.slice(3);
    if (/^Digit[0-9]$/.test(code)) return code.slice(5);
    if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
    if (["Space", "Insert", "Home", "End", "PageUp", "PageDown", "Pause"].includes(code)) return code;
    return null;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Tab") return;
    e.preventDefault();
    if (e.key === "Backspace" || e.key === "Delete") {
      value = "";
      return;
    }
    if (MODIFIER_KEYS.includes(e.key)) return;
    const key = keyName(e.code);
    // Wajib Ctrl atau Alt agar tidak bentrok dengan ketikan biasa.
    if (!key || !(e.ctrlKey || e.altKey)) return;
    const parts = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super", key];
    value = parts.filter(Boolean).join("+");
  }
</script>

<div class="flex items-center gap-2">
  <input
    readonly
    class="w-48 rounded-lg border border-gray-300 px-3 py-2 font-mono text-sm focus:border-indigo-500"
    {value}
    placeholder="—"
    onkeydown={onKeydown}
  />
  {#if value}
    <button type="button" class="rounded-lg px-3 py-2 text-sm hover:bg-gray-100" onclick={() => (value = "")}>
      {id.settings.shortcutClear}
    </button>
  {/if}
</div>
