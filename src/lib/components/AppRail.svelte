<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { api } from "$lib/api";
  import Icon, { type IconName } from "$lib/components/Icon.svelte";
  import RecordButton from "$lib/components/RecordButton.svelte";
  import { id } from "$lib/i18n/id";
  import { startTour } from "$lib/tour.svelte";
  import { railCollapsed, toggleRail } from "$lib/ui.svelte";

  let shortcut = $state("");
  onMount(async () => {
    shortcut = await api.getSettings().then((s) => s.globalShortcut, () => "");
  });

  const collapsed = $derived(railCollapsed());
  const toggleLabel = $derived(collapsed ? id.rail.show : id.rail.hide);

  const nav: { href: string; text: string; icon: IconName; match: (p: string) => boolean }[] = [
    { href: "/", text: id.nav.home, icon: "home", match: (p) => p === "/" },
    { href: "/meetings", text: id.nav.meetings, icon: "list", match: (p) => p.startsWith("/meeting") },
    { href: "/tasks", text: id.nav.tasks, icon: "tasks", match: (p) => p.startsWith("/tasks") },
    { href: "/settings", text: id.nav.settings, icon: "settings", match: (p) => p.startsWith("/settings") },
  ];
</script>

<!-- Rel ringkas (ikon, label jadi tooltip) atau penuh; default ikut lebar jendela (≥ 1280 px penuh). -->
<aside
  data-rail={collapsed ? "icons" : "full"}
  class="flex h-full w-[4.25rem] shrink-0 flex-col gap-6 px-2.5 py-5 full:w-56 full:px-4"
>
  <div data-tour="record"><RecordButton /></div>

  <nav aria-label={id.nav.label} class="flex flex-col gap-0.5">
    {#each nav as n (n.href)}
      {@const active = n.match(page.url.pathname)}
      <a
        href={n.href}
        data-tour={`nav-${n.href === "/" ? "home" : n.href.slice(1)}`}
        aria-current={active ? "page" : undefined}
        title={n.text}
        class={[
          "flex items-center justify-center gap-3 rounded-lg px-3 py-2.5 text-base full:justify-start full:py-2",
          active ? "bg-sheet font-semibold text-ink shadow-[0_1px_2px_rgb(28_31_38/0.08),inset_0_0_0_1px_var(--color-line)]" : "text-ink-soft hover:bg-wash hover:text-ink",
        ]}
      >
        <Icon name={n.icon} size={18} class="shrink-0" />
        <span class="sr-only full:not-sr-only">{n.text}</span>
      </a>
    {/each}
  </nav>

  <div class="mt-auto flex flex-col gap-3">
    {#if shortcut}
      <p class="hidden px-1 text-sm leading-relaxed text-ink-faint full:block">{id.rail.shortcutHint(shortcut)}</p>
    {/if}
    <button
      type="button"
      class="flex items-center justify-center gap-3 rounded-lg px-3 py-2.5 text-sm text-ink-soft hover:bg-wash hover:text-ink full:justify-start full:py-2"
      title={id.guide.openLong}
      data-tour="guide"
      onclick={() => startTour()}
    >
      <Icon name="help" size={18} class="shrink-0" />
      <span class="sr-only whitespace-nowrap full:not-sr-only">{id.guide.open}</span>
    </button>
    <button
      type="button"
      class="flex items-center justify-center gap-3 rounded-lg px-3 py-2.5 text-sm text-ink-soft hover:bg-wash hover:text-ink full:justify-start full:py-2"
      title={toggleLabel}
      aria-label={toggleLabel}
      aria-expanded={!collapsed}
      onclick={() => toggleRail()}
    >
      <Icon name="panel-left" size={18} class="shrink-0" />
      <span class="sr-only whitespace-nowrap full:not-sr-only">{id.rail.hideShort}</span>
    </button>
  </div>
</aside>
