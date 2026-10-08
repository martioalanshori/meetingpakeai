<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { api } from "$lib/api";
  import RecordButton from "$lib/components/RecordButton.svelte";
  import Wordmark from "$lib/components/Wordmark.svelte";
  import { id } from "$lib/i18n/id";

  let shortcut = $state("");
  onMount(async () => {
    shortcut = await api.getSettings().then((s) => s.globalShortcut, () => "");
  });

  const nav = [
    { href: "/", text: id.nav.meetings, match: (p: string) => p === "/" || p.startsWith("/meeting") },
    { href: "/tasks", text: id.nav.tasks, match: (p: string) => p.startsWith("/tasks") },
    { href: "/settings", text: id.nav.settings, match: (p: string) => p.startsWith("/settings") },
  ];
</script>

<aside class="flex h-full w-56 shrink-0 flex-col gap-6 border-r border-line px-4 py-5">
  <div class="px-1"><Wordmark /></div>

  <RecordButton />

  <nav aria-label={id.nav.label} class="flex flex-col gap-0.5">
    {#each nav as n (n.href)}
      {@const active = n.match(page.url.pathname)}
      <a
        href={n.href}
        aria-current={active ? "page" : undefined}
        class={[
          "rounded-lg px-3 py-2 text-[0.9375rem]",
          active ? "bg-sheet font-semibold text-ink shadow-[inset_0_0_0_1px_var(--color-line)]" : "text-ink-soft hover:bg-wash hover:text-ink",
        ]}
      >
        {n.text}
      </a>
    {/each}
  </nav>

  {#if shortcut}
    <p class="mt-auto px-1 text-xs leading-relaxed text-ink-faint">{id.rail.shortcutHint(shortcut)}</p>
  {/if}
</aside>
