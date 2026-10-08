<script lang="ts" module>
  import type { IconName } from "$lib/components/Icon.svelte";

  export type MenuEntry =
    | {
        label: string;
        onselect: () => void;
        icon?: IconName;
        disabled?: boolean;
        danger?: boolean;
        /** Teks kecil di bawah label (mis. alasan item nonaktif). */
        hint?: string;
      }
    | { separator: true };
</script>

<script lang="ts">
  import { tick, type Snippet } from "svelte";
  import Icon from "$lib/components/Icon.svelte";

  // Menu tarik-turun bersama: tertutup saat klik di luar / Escape / Tab, navigasi ↑ ↓ Home End,
  // fokus masuk ke item pertama saat dibuka dan kembali ke tombol saat ditutup.
  let {
    label,
    items,
    trigger,
    triggerClass = "btn btn-quiet btn-icon",
    align = "right",
  }: {
    label: string;
    items: MenuEntry[];
    trigger: Snippet;
    triggerClass?: string;
    align?: "left" | "right";
  } = $props();

  let open = $state(false);
  let root = $state<HTMLDivElement | null>(null);
  let button = $state<HTMLButtonElement | null>(null);
  let list = $state<HTMLDivElement | null>(null);

  function enabledItems(): HTMLButtonElement[] {
    return list ? [...list.querySelectorAll<HTMLButtonElement>('[role="menuitem"]:not(:disabled)')] : [];
  }

  function focusAt(i: number) {
    const els = enabledItems();
    if (els.length > 0) els[((i % els.length) + els.length) % els.length].focus();
  }

  async function show() {
    open = true;
    await tick();
    focusAt(0);
  }

  function hide(refocus = true) {
    open = false;
    if (refocus) button?.focus();
  }

  function onKeydown(e: KeyboardEvent) {
    const els = enabledItems();
    const idx = els.indexOf(document.activeElement as HTMLButtonElement);
    if (e.key === "ArrowDown") {
      e.preventDefault();
      focusAt(idx + 1);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      focusAt(idx - 1);
    } else if (e.key === "Home") {
      e.preventDefault();
      focusAt(0);
    } else if (e.key === "End") {
      e.preventDefault();
      focusAt(els.length - 1);
    } else if (e.key === "Escape") {
      e.preventDefault();
      hide();
    } else if (e.key === "Tab") {
      hide(false);
    }
  }

  function choose(entry: Extract<MenuEntry, { label: string }>) {
    hide();
    entry.onselect();
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (open && root && !root.contains(e.target as Node)) hide(false);
  }}
/>

<div class="relative" bind:this={root}>
  <button
    type="button"
    class={triggerClass}
    aria-label={label}
    title={label}
    aria-haspopup="menu"
    aria-expanded={open}
    bind:this={button}
    onclick={() => (open ? hide() : show())}
    onkeydown={(e) => {
      if (e.key === "ArrowDown" && !open) {
        e.preventDefault();
        show();
      }
    }}
  >
    {@render trigger()}
  </button>
  {#if open}
    <div
      role="menu"
      tabindex="-1"
      aria-label={label}
      class={["menu", align === "left" ? "right-auto left-0" : ""]}
      bind:this={list}
      onkeydown={onKeydown}
    >
      {#each items as entry, i (i)}
        {#if "separator" in entry}
          <div class="menu-sep" role="separator"></div>
        {:else}
          <button
            type="button"
            role="menuitem"
            class={["menu-item", entry.danger && "text-bad"]}
            disabled={entry.disabled}
            onclick={() => choose(entry)}
          >
            {#if entry.icon}<Icon name={entry.icon} size={16} class="shrink-0 opacity-80" />{/if}
            <span class="flex flex-col">
              <span>{entry.label}</span>
              {#if entry.hint}<span class="text-xs text-ink-faint">{entry.hint}</span>{/if}
            </span>
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</div>
