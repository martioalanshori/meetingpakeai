<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { api } from "$lib/api";
  import Icon, { type IconName } from "$lib/components/Icon.svelte";
  import RecordButton from "$lib/components/RecordButton.svelte";
  import { id } from "$lib/i18n/id";
  import { toggleRail } from "$lib/ui.svelte";

  let shortcut = $state("");
  onMount(async () => {
    shortcut = await api.getSettings().then((s) => s.globalShortcut, () => "");
  });

  const nav: { href: string; text: string; icon: IconName; match: (p: string) => boolean }[] = [
    { href: "/", text: id.nav.home, icon: "home", match: (p) => p === "/" },
    { href: "/meetings", text: id.nav.meetings, icon: "list", match: (p) => p.startsWith("/meeting") },
    { href: "/tasks", text: id.nav.tasks, icon: "tasks", match: (p) => p.startsWith("/tasks") },
    { href: "/settings", text: id.nav.settings, icon: "settings", match: (p) => p.startsWith("/settings") },
  ];
</script>

<!-- < 1280 px: rel ikon (label jadi tooltip) agar panel detail cukup lebar; ≥ 1280 px: rel penuh. -->
<aside class="flex h-full w-[4.25rem] shrink-0 flex-col gap-6 px-2.5 py-5 xl:w-56 xl:px-4">
  <RecordButton />

  <nav aria-label={id.nav.label} class="flex flex-col gap-0.5">
    {#each nav as n (n.href)}
      {@const active = n.match(page.url.pathname)}
      <a
        href={n.href}
        aria-current={active ? "page" : undefined}
        title={n.text}
        class={[
          "flex items-center justify-center gap-3 rounded-lg px-3 py-2.5 text-base xl:justify-start xl:py-2",
          active ? "bg-sheet font-semibold text-ink shadow-[0_1px_2px_rgb(28_31_38/0.08),inset_0_0_0_1px_var(--color-line)]" : "text-ink-soft hover:bg-wash hover:text-ink",
        ]}
      >
        <Icon name={n.icon} size={18} class="shrink-0" />
        <span class="sr-only xl:not-sr-only">{n.text}</span>
      </a>
    {/each}
  </nav>

  <div class="mt-auto flex flex-col gap-3">
    {#if shortcut}
      <p class="hidden px-1 text-xs leading-relaxed text-ink-faint xl:block">{id.rail.shortcutHint(shortcut)}</p>
    {/if}
    <button
      type="button"
      class="flex items-center justify-center gap-3 rounded-lg px-3 py-2.5 text-sm text-ink-soft hover:bg-wash hover:text-ink xl:justify-start xl:py-2"
      title={id.rail.hide}
      aria-label={id.rail.hide}
      onclick={() => toggleRail(true)}
    >
      <Icon name="panel-left" size={18} class="shrink-0" />
      <span class="sr-only xl:not-sr-only">{id.rail.hide.replace(" (Ctrl+B)", "")}</span>
    </button>
  </div>
</aside>
