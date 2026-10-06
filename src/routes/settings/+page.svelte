<script lang="ts">
  import ApiKeySection from "$lib/components/ApiKeySection.svelte";
  import { id } from "$lib/i18n/id";
</script>

<main class="mx-auto flex max-w-3xl flex-col gap-4 p-6">
  <a href="/" class="text-sm text-indigo-700 hover:underline">{id.placeholder.back}</a>
  <h1 class="text-xl font-semibold">{id.settings.title}</h1>

  <ApiKeySection />

  <!-- Langkah 14: nama "Saya", bahasa, hapus audio, tray, pesan consent, versi & folder log. -->
</main>
