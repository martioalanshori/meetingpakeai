<script lang="ts">
  // Jendela "Catatan saya" selama merekam (langkah 49): dibuka dari tombol pensil di widget.
  import { page } from "$app/state";
  import NotesEditor from "$lib/components/NotesEditor.svelte";
  import { id } from "$lib/i18n/id";

  const meetingId = page.url.searchParams.get("id") ?? "";
</script>

<main class="on-dark flex h-screen flex-col gap-2 bg-ink p-3 text-white">
  <p class="text-xs text-white/70">{id.notes.windowHint}</p>
  {#if meetingId}
    <div class="min-h-0 flex-1 [&_textarea]:h-[calc(100vh-5.5rem)]">
      <NotesEditor {meetingId} rows={8} autofocus dark />
    </div>
  {/if}
</main>
